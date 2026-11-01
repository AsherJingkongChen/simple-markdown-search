//! index: the SQLite file at `~/.sms/index.db`, shared by every root. `set` rebuilds a root
//! from scratch; `refresh`, which `get` runs first, reindexes only the files whose
//! `(path, mtime, bytes)` changed. A file is reindexed by deleting its row, which cascades to
//! its lines and postings, and inserting it again. A posting carries its term, so nothing
//! outlives the file it came from. The text itself is not stored.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, UNIX_EPOCH};

use rusqlite::{Connection, Transaction, TransactionBehavior, params};
use walkdir::WalkDir;

use crate::error::{Error, Result, at};
use crate::token::{Token, tokens};

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
CREATE TABLE IF NOT EXISTS post (
    field INTEGER, text TEXT, id_file INTEGER REFERENCES file ON DELETE CASCADE,
    at_line INTEGER, hits INTEGER,
    PRIMARY KEY (field, text, id_file, at_line)) WITHOUT ROWID;
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

/// Open the index, waiting up to five seconds for another `sms` that holds its lock. Writes
/// begin IMMEDIATE, taking the write lock at once, so two writers queue on this wait instead
/// of failing with SQLITE_BUSY when a read lock would need upgrading.
pub fn open() -> Result<Connection> {
    let folder = Path::new(&std::env::var("HOME")?).join(".sms");
    std::fs::create_dir_all(&folder).map_err(at(&folder))?;
    let connection = Connection::open(folder.join("index.db"))?;
    connection.busy_timeout(Duration::from_secs(5))?;
    connection.execute_batch(SCHEMA)?;
    Ok(connection)
}

/// Register the root with these dirs, or with its stored dirs when none are given, and
/// rebuild it from scratch.
pub fn set(connection: &mut Connection, path: &Path, dirs: &[String]) -> Result<()> {
    let path = path.canonicalize().map_err(at(path))?;
    let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let text = path.to_string_lossy();
    transaction.execute("INSERT OR IGNORE INTO root (path) VALUES (?1)", [&text])?;
    let id = transaction.query_row("SELECT id_root FROM root WHERE path = ?1", [&text], |row| {
        row.get(0)
    })?;
    if !dirs.is_empty() {
        transaction.execute("DELETE FROM dir WHERE id_root = ?1", [id])?;
        for dir in dirs {
            transaction.execute(
                "INSERT INTO dir (id_root, name) VALUES (?1, ?2)",
                params![id, dir],
            )?;
        }
    }
    let root = Root { id, path };
    let dirs = stored_dirs(&transaction, &root)?;
    if dirs.is_empty() {
        return Err(Error::NoDirs(root.path));
    }
    transaction.execute("DELETE FROM file WHERE id_root = ?1", [root.id])?;
    for found in walk(&root, &dirs)? {
        insert(&transaction, &root, &found)?;
    }
    transaction.commit()?;
    Ok(())
}

/// Reindex the files that were added, changed, or removed since the index last saw them.
pub fn refresh(connection: &mut Connection, root: &Root) -> Result<()> {
    let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let mut stored: HashMap<String, (i64, i64, i64)> = transaction
        .prepare("SELECT path, id_file, mtime, bytes FROM file WHERE id_root = ?1")?
        .query_map([root.id], |row| {
            Ok((row.get(0)?, (row.get(1)?, row.get(2)?, row.get(3)?)))
        })?
        .collect::<rusqlite::Result<_>>()?;
    for found in walk(root, &stored_dirs(&transaction, root)?)? {
        match stored.remove(&found.path) {
            Some((_, mtime, bytes)) if mtime == found.mtime && bytes == found.bytes => {}
            Some((id_file, _, _)) => {
                delete(&transaction, id_file)?;
                insert(&transaction, root, &found)?;
            }
            None => insert(&transaction, root, &found)?,
        }
    }
    for (id_file, _, _) in stored.into_values() {
        delete(&transaction, id_file)?;
    }
    transaction.commit()?;
    Ok(())
}

/// The stored root that this path or basename names. A name that is no existing path is
/// matched against the basenames.
pub fn find(connection: &Connection, name: &str) -> Result<Root> {
    let roots: Vec<Root> = connection
        .prepare("SELECT id_root, path FROM root")?
        .query_map([], |row| {
            Ok(Root {
                id: row.get(0)?,
                path: PathBuf::from(row.get::<_, String>(1)?),
            })
        })?
        .collect::<rusqlite::Result<_>>()?;
    let exact = Path::new(name).canonicalize().ok();
    let mut matches: Vec<Root> = roots
        .into_iter()
        .filter(|root| match &exact {
            Some(path) => &root.path == path,
            None => root.path.file_name().is_some_and(|base| base == name),
        })
        .collect();
    match matches.len() {
        0 => Err(Error::NoRoot(name.to_owned())),
        1 => Ok(matches.remove(0)),
        _ => Err(Error::Roots {
            name: name.to_owned(),
            roots: matches
                .iter()
                .map(|root| root.path.display().to_string())
                .collect::<Vec<_>>()
                .join(", "),
        }),
    }
}

fn stored_dirs(transaction: &Transaction, root: &Root) -> Result<Vec<String>> {
    Ok(transaction
        .prepare("SELECT name FROM dir WHERE id_root = ?1 ORDER BY name")?
        .query_map([root.id], |row| row.get(0))?
        .collect::<rusqlite::Result<_>>()?)
}

/// Every markdown file under the root's dirs. A dir that is missing or unreadable, or a file
/// whose metadata cannot be read, fails the walk.
fn walk(root: &Root, dirs: &[String]) -> Result<Vec<Found>> {
    let mut found = Vec::new();
    for dir in dirs {
        for entry in WalkDir::new(root.path.join(dir)).follow_links(true) {
            let entry = entry?;
            let path = entry.path();
            if !entry.file_type().is_file()
                || path.extension().is_none_or(|extension| extension != "md")
            {
                continue;
            }
            let metadata = entry.metadata()?;
            let mtime = metadata
                .modified()
                .map_err(at(path))?
                .duration_since(UNIX_EPOCH)
                .map_err(|source| Error::Time {
                    path: path.to_owned(),
                    source,
                })?;
            found.push(Found {
                path: path
                    .strip_prefix(&root.path)
                    .expect("a walked path should lie under its root")
                    .to_string_lossy()
                    .into_owned(),
                mtime: mtime.as_nanos() as i64,
                bytes: metadata.len() as i64,
            });
        }
    }
    Ok(found)
}

fn delete(transaction: &Transaction, id_file: i64) -> Result<()> {
    transaction.execute("DELETE FROM file WHERE id_file = ?1", [id_file])?;
    Ok(())
}

/// Insert a file with its lines and postings.
fn insert(transaction: &Transaction, root: &Root, found: &Found) -> Result<()> {
    let path = root.path.join(&found.path);
    let bytes = std::fs::read(&path).map_err(at(&path))?;
    let text = String::from_utf8_lossy(&bytes);
    transaction.execute(
        "INSERT INTO file (id_root, path, mtime, bytes) VALUES (?1, ?2, ?3, ?4)",
        params![root.id, found.path, found.mtime, found.bytes],
    )?;
    let id_file = transaction.last_insert_rowid();
    let mut line_statement = transaction
        .prepare_cached("INSERT INTO line (id_file, at_line, tokens) VALUES (?1, ?2, ?3)")?;
    let mut post_statement = transaction.prepare_cached(
        "INSERT INTO post (field, text, id_file, at_line, hits) VALUES (?1, ?2, ?3, ?4, ?5)",
    )?;
    for (index, line) in text.lines().enumerate() {
        let bag = tokens(line);
        if bag.is_empty() {
            continue;
        }
        let at_line = index as i64 + 1;
        line_statement.execute(params![id_file, at_line, bag.len() as i64])?;
        let mut hits: HashMap<&Token, i64> = HashMap::new();
        for token in &bag {
            *hits.entry(token).or_default() += 1;
        }
        for ((field, text), count) in hits {
            post_statement.execute(params![field, text, id_file, at_line, count])?;
        }
    }
    Ok(())
}
