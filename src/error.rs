//! error: everything sms can fail at, each saying what it failed on.

use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(
        "usage: sms set <root> [dir...]\n       sms get <root> <query...> [-n LIMIT] [-c COVERAGE]"
    )]
    Usage,
    #[error(transparent)]
    Argument(#[from] lexopt::Error),
    #[error("HOME, where ~/.sms lives: {0}")]
    Home(#[from] std::env::VarError),
    #[error("{}: {source}", path.display())]
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error(transparent)]
    Walk(#[from] walkdir::Error),
    #[error("{}: {source}", path.display())]
    Time {
        path: PathBuf,
        source: std::time::SystemTimeError,
    },
    #[error("~/.sms/index.db: {0}")]
    Index(#[from] rusqlite::Error),
    #[error("name the dirs to index: sms set {} <dir>...", .0.display())]
    NoDirs(PathBuf),
    #[error("no root named {0}; index it first: sms set <root> <dir>...")]
    NoRoot(String),
    #[error("{name} names several roots, so give the full path: {roots}")]
    Roots { name: String, roots: String },
    #[error("no line matches {0}")]
    NoMatch(String),
}

pub type Result<T> = std::result::Result<T, Error>;

/// Attach the path to an IO error.
pub fn at(path: impl Into<PathBuf>) -> impl FnOnce(std::io::Error) -> Error {
    let path = path.into();
    move |source| Error::Io { path, source }
}
