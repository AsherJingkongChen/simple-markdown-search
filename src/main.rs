//! sms: rank the lines of markdown files against a query. See DESIGN.md.
//!
//! sms set <root> [dir...]
//! sms get <root> <query...> [-n LIMIT] [-c COVERAGE]

mod bpe;
mod index;
mod search;
mod token;
mod unigram;

use std::path::PathBuf;
use std::process::ExitCode;

use lexopt::prelude::*;

const USAGE: &str =
    "usage: sms set <root> [dir...]\n       sms get <root> <query...> [-n LIMIT] [-c COVERAGE]";

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

fn parse() -> Result<Command, lexopt::Error> {
    let mut parser = lexopt::Parser::from_env();
    let mut words: Vec<String> = Vec::new();
    let mut limit = 10;
    let mut coverage = 0;
    while let Some(argument) = parser.next()? {
        match argument {
            Short('n') => limit = parser.value()?.parse()?,
            Short('c') => coverage = parser.value()?.parse()?,
            Value(word) => words.push(word.string()?),
            _ => return Err(argument.unexpected()),
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
        _ => Err(USAGE.into()),
    }
}

/// One row per line, the hit with its score and its coverage without one:
/// `0.62  notes/rust.md:16  - A borrow cannot outlive its owner.`
fn print(root: &index::Root, hits: &[search::Hit], coverage: usize) {
    let top = hits.first().map_or(1.0, |hit| hit.score);
    for hit in hits {
        let text = std::fs::read_to_string(root.path.join(&hit.path)).unwrap_or_default();
        let lines: Vec<&str> = text.lines().collect();
        let at = hit.at_line as usize;
        for number in at.saturating_sub(coverage).max(1)..=(at + coverage).min(lines.len()) {
            let score = match number == at {
                true => format!("{:.2}", hit.score / top),
                false => String::new(),
            };
            println!("{score:>4}  {}:{number}  {}", hit.path, lines[number - 1]);
        }
    }
}

fn main() -> ExitCode {
    let command = match parse() {
        Ok(command) => command,
        Err(error) => {
            eprintln!("sms: {error}\n{USAGE}");
            return ExitCode::FAILURE;
        }
    };
    let mut connection = index::open();
    let outcome = match command {
        Command::Set { root, dirs } => index::set(&mut connection, &root, &dirs),
        Command::Get {
            root,
            query,
            limit,
            coverage,
        } => index::find(&connection, &root).map(|root| {
            index::refresh(&mut connection, &root);
            print(
                &root,
                &search::search(&connection, &root, &query, limit),
                coverage,
            );
        }),
    };
    match outcome {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("sms: {message}");
            ExitCode::FAILURE
        }
    }
}
