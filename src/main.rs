//! sms: rank the lines of markdown files against a query. See DESIGN.md.
//!
//! sms set <root> [dir...]
//! sms get <root> <query...> [-n LIMIT] [-c COVERAGE]

mod bpe;
mod error;
mod index;
mod search;
mod token;
mod unigram;

use std::path::PathBuf;
use std::process::ExitCode;

use lexopt::prelude::*;

use crate::error::{Error, Result, at};

enum Command {
    Set {
        root: PathBuf,
        dirs: Vec<String>,
    },
    Get {
        root: String,
        query: String,
        limit: usize,
        coverage: usize,
    },
}

fn parse() -> Result<Command> {
    let mut parser = lexopt::Parser::from_env();
    let mut words: Vec<String> = Vec::new();
    let mut limit = 3;
    let mut coverage = 1;
    while let Some(argument) = parser.next()? {
        match argument {
            Short('n') => limit = parser.value()?.parse()?,
            Short('c') => coverage = parser.value()?.parse()?,
            Value(word) => words.push(word.string()?),
            _ => return Err(argument.unexpected().into()),
        }
    }
    match words.as_slice() {
        [command, root, dirs @ ..] if command == "set" => Ok(Command::Set {
            root: root.into(),
            dirs: dirs.to_vec(),
        }),
        [command, root, query @ ..] if command == "get" && !query.is_empty() => Ok(Command::Get {
            root: root.clone(),
            query: query.join(" "),
            limit,
            coverage,
        }),
        _ => Err(Error::Usage),
    }
}

/// grep's format, with each hit's score on the `--` line before it: the hit is
/// `path:line:text` and its coverage `path-line-text`.
///
/// ```text
/// -- 0.62
/// notes/rust.md-15-## Borrowing
/// notes/rust.md:16:- A borrow cannot outlive its owner.
/// ```
fn print(root: &index::Root, hits: &[search::Hit], coverage: usize) -> Result<()> {
    for hit in hits {
        println!("-- {:.2}", hit.score);
        let path = root.path.join(&hit.path);
        let text = std::fs::read_to_string(&path).map_err(at(&path))?;
        let lines: Vec<&str> = text.lines().collect();
        let at = hit.at_line as usize;
        for number in at.saturating_sub(coverage).max(1)..=(at + coverage).min(lines.len()) {
            let mark = if number == at { ':' } else { '-' };
            println!("{}{mark}{number}{mark}{}", hit.path, lines[number - 1]);
        }
    }
    Ok(())
}

fn run() -> Result<()> {
    let command = parse()?;
    let mut connection = index::open()?;
    match command {
        Command::Set { root, dirs } => index::set(&mut connection, &root, &dirs),
        Command::Get {
            root,
            query,
            limit,
            coverage,
        } => {
            let root = index::find(&connection, &root)?;
            index::refresh(&mut connection, &root)?;
            let hits = search::search(&connection, &root, &query, limit)?;
            if hits.is_empty() {
                return Err(Error::NoMatch(query));
            }
            print(&root, &hits, coverage)
        }
    }
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("sms: {error}");
            ExitCode::FAILURE
        }
    }
}
