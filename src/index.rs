//! index: the SQLite file at `~/.sms/index.db`, shared by every root. `set` rebuilds a root
//! from scratch; `refresh`, which `get` runs first, reindexes only the files whose
//! `(path, mtime, bytes)` changed. A file is reindexed by deleting its row, which cascades to
//! its lines and postings, and inserting it again. The text itself is not stored.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use rusqlite::{Connection, OptionalExtension, Transaction, params};
use walkdir::WalkDir;

use crate::token::tokens;

const SCHEMA: &str = "
PRAGMA foreign_keys = ON;
CREATE TABLE IF NOT EXISTS root (id_root INTEGER PRIMARY KEY, path TEXT UNIQUE);
CREATE TABLE IF NOT EXISTS dir (
    id_root INTEGER REFERENCES root ON DELETE CASCADE, name TEXT,
    PRIMARY KEY (id_root, name));
CREATE TABLE IF NOT EXISTS file (
    id_file INTEGER PRIMARY KEY, id_root INTEGER REFERENCES root ON DELETE CASCADE,
    path TEXT, mtime INTEGER, bytes INTEGER, UNIQUE (id_root, path));
CREATE TABLE IF NOT EXISTS line (
    id_file INTEGER REFERENCES file ON DELETE CASCADE, at_line INTEGER, tokens INTEGER,
    PRIMARY KEY (id_file, at_line)) WITHOUT ROWID;
CREATE TABLE IF NOT EXISTS term (id_term INTEGER PRIMARY KEY, text TEXT UNIQUE);
CREATE TABLE IF NOT EXISTS post (
    id_term INTEGER REFERENCES term, id_file INTEGER REFERENCES file ON DELETE CASCADE,
    at_line INTEGER, hits INTEGER,
    PRIMARY KEY (id_term, id_file, at_line)) WITHOUT ROWID;
";

/// A root as stored: its id and its absolute path.
pub struct Root {
    pub id: i64,
    pub path: PathBuf,
}

/// A markdown file on disk, relative to its root.
struct Found {
    path: String,
    mtime: i64,
    bytes: i64,
}

pub fn open() -> Connection {
    let home = std::env::var("HOME").expect("HOME should be set");
    let folder = Path::new(&home).join(".sms");
    std::fs::create_dir_all(&folder).expect("~/.sms should be creatable");
    let connection =
        Connection::open(folder.join("index.db")).expect("~/.sms/index.db should open");
    connection
        .execute_batch(SCHEMA)
        .expect("the schema should apply to ~/.sms/index.db");
    connection
}

/// Register the root with these dirs, or with its stored dirs when none are given, and
/// rebuild it from scratch.
pub fn set(connection: &mut Connection, path: &Path, dirs: &[String]) -> Result<(), String> {
    let path = path
        .canonicalize()
        .map_err(|error| format!("{}: {error}", path.display()))?;
    let transaction = connection
        .transaction()
        .expect("a transaction should begin");
    transaction
        .execute(
            "INSERT OR IGNORE INTO root (path) VALUES (?1)",
            [path.to_string_lossy()],
        )
        .expect("the root should insert");
    let id = transaction
        .query_row(
            "SELECT id_root FROM root WHERE path = ?1",
            [path.to_string_lossy()],
            |row| row.get(0),
        )
        .expect("the root should exist");
    if !dirs.is_empty() {
        transaction
            .execute("DELETE FROM dir WHERE id_root = ?1", [id])
            .expect("the dirs should delete");
        for dir in dirs {
            transaction
                .execute(
                    "INSERT INTO dir (id_root, name) VALUES (?1, ?2)",
                    params![id, dir],
                )
                .expect("a dir should insert");
        }
    }
    let root = Root { id, path };
    let dirs = stored_dirs(&transaction, &root);
    if dirs.is_empty() {
        return Err(format!(
            "name the dirs to index: sms set {} <dir>...",
            root.path.display()
        ));
    }
    transaction
        .execute("DELETE FROM file WHERE id_root = ?1", [root.id])
        .expect("the root's files should delete");
    let mut terms = HashMap::new();
    for found in walk(&root, &dirs) {
        insert(&transaction, &root, &found, &mut terms);
    }
    transaction.commit().expect("the rebuild should commit");
    Ok(())
}

/// Reindex the files that were added, changed, or removed since the index last saw them.
pub fn refresh(connection: &mut Connection, root: &Root) {
    let transaction = connection
        .transaction()
        .expect("a transaction should begin");
    let mut stored: HashMap<String, (i64, i64, i64)> = transaction
        .prepare("SELECT path, id_file, mtime, bytes FROM file WHERE id_root = ?1")
        .expect("the file query should prepare")
        .query_map([root.id], |row| {
            Ok((row.get(0)?, (row.get(1)?, row.get(2)?, row.get(3)?)))
        })
        .expect("the files should read")
        .collect::<Result<_, _>>()
        .expect("each file row should read");
    let mut terms = HashMap::new();
    for found in walk(root, &stored_dirs(&transaction, root)) {
        match stored.remove(&found.path) {
            Some((_, mtime, bytes)) if mtime == found.mtime && bytes == found.bytes => {}
            Some((id_file, _, _)) => {
                delete(&transaction, id_file);
                insert(&transaction, root, &found, &mut terms);
            }
            None => insert(&transaction, root, &found, &mut terms),
        }
    }
    for (id_file, _, _) in stored.into_values() {
        delete(&transaction, id_file);
    }
    transaction.commit().expect("the refresh should commit");
}

/// The stored root that this path or basename names.
pub fn find(connection: &Connection, name: &str) -> Result<Root, String> {
    let roots: Vec<Root> = connection
        .prepare("SELECT id_root, path FROM root")
        .expect("the root query should prepare")
        .query_map([], |row| {
            Ok(Root {
                id: row.get(0)?,
                path: PathBuf::from(row.get::<_, String>(1)?),
            })
        })
        .expect("the roots should read")
        .collect::<Result<_, _>>()
        .expect("each root row should read");
    let exact = Path::new(name).canonicalize().ok();
    let mut matches: Vec<Root> = roots
        .into_iter()
        .filter(|root| match &exact {
            Some(path) => &root.path == path,
            None => root.path.file_name().is_some_and(|base| base == name),
        })
        .collect();
    match matches.len() {
        0 => Err(format!(
            "no root named {name}; index it first: sms set <root> <dir>..."
        )),
        1 => Ok(matches.remove(0)),
        _ => Err(format!(
            "{name} names several roots, so give the full path: {}",
            matches
                .iter()
                .map(|root| root.path.display().to_string())
                .collect::<Vec<_>>()
                .join(", ")
        )),
    }
}

fn stored_dirs(transaction: &Transaction, root: &Root) -> Vec<String> {
    transaction
        .prepare("SELECT name FROM dir WHERE id_root = ?1 ORDER BY name")
        .expect("the dir query should prepare")
        .query_map([root.id], |row| row.get(0))
        .expect("the dirs should read")
        .collect::<Result<_, _>>()
        .expect("each dir row should read")
}

/// Every markdown file under the root's dirs.
fn walk(root: &Root, dirs: &[String]) -> Vec<Found> {
    dirs.iter()
        .flat_map(|dir| WalkDir::new(root.path.join(dir)).follow_links(true))
        .filter_map(Result::ok)
        .filter(|entry| {
            entry.file_type().is_file()
                && entry
                    .path()
                    .extension()
                    .is_some_and(|extension| extension == "md")
        })
        .filter_map(|entry| {
            let metadata = entry.metadata().ok()?;
            let mtime = metadata
                .modified()
                .ok()?
                .duration_since(UNIX_EPOCH)
                .ok()?
                .as_nanos();
            Some(Found {
                path: entry
                    .path()
                    .strip_prefix(&root.path)
                    .ok()?
                    .to_string_lossy()
                    .into_owned(),
                mtime: mtime as i64,
                bytes: metadata.len() as i64,
            })
        })
        .collect()
}

fn delete(transaction: &Transaction, id_file: i64) {
    transaction
        .execute("DELETE FROM file WHERE id_file = ?1", [id_file])
        .expect("a file should delete");
}

/// Insert a file with its lines and postings. `terms` caches term ids across the files of
/// one transaction.
fn insert(transaction: &Transaction, root: &Root, found: &Found, terms: &mut HashMap<String, i64>) {
    let text = String::from_utf8_lossy(
        &std::fs::read(root.path.join(&found.path)).expect("a walked file should be readable"),
    )
    .into_owned();
    transaction
        .execute(
            "INSERT INTO file (id_root, path, mtime, bytes) VALUES (?1, ?2, ?3, ?4)",
            params![root.id, found.path, found.mtime, found.bytes],
        )
        .expect("a file should insert");
    let id_file = transaction.last_insert_rowid();
    let mut line_statement = transaction
        .prepare_cached("INSERT INTO line (id_file, at_line, tokens) VALUES (?1, ?2, ?3)")
        .expect("the line insert should prepare");
    let mut post_statement = transaction
        .prepare_cached(
            "INSERT INTO post (id_term, id_file, at_line, hits) VALUES (?1, ?2, ?3, ?4)",
        )
        .expect("the post insert should prepare");
    for (index, line) in text.lines().enumerate() {
        let bag = tokens(line);
        if bag.is_empty() {
            continue;
        }
        let at_line = index as i64 + 1;
        line_statement
            .execute(params![id_file, at_line, bag.len() as i64])
            .expect("a line should insert");
        let mut hits: HashMap<&str, i64> = HashMap::new();
        for token in &bag {
            *hits.entry(token).or_default() += 1;
        }
        for (token, count) in hits {
            let id_term = term(transaction, token, terms);
            post_statement
                .execute(params![id_term, id_file, at_line, count])
                .expect("a post should insert");
        }
    }
}

/// The id of a term, inserted on first sight.
fn term(transaction: &Transaction, text: &str, terms: &mut HashMap<String, i64>) -> i64 {
    if let Some(&id) = terms.get(text) {
        return id;
    }
    let found: Option<i64> = transaction
        .prepare_cached("SELECT id_term FROM term WHERE text = ?1")
        .expect("the term query should prepare")
        .query_row([text], |row| row.get(0))
        .optional()
        .expect("the term should read");
    let id = found.unwrap_or_else(|| {
        transaction
            .prepare_cached("INSERT INTO term (text) VALUES (?1)")
            .expect("the term insert should prepare")
            .execute([text])
            .expect("a term should insert");
        transaction.last_insert_rowid()
    });
    terms.insert(text.to_owned(), id);
    id
}
