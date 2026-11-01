//! search: BM25+ over the lines of one root. Each distinct term of the query adds
//! `idf · (tf·(k1+1) / (tf + k1·(1 − b + b·length/average)) + δ)` to every line that holds it,
//! with `idf = ln(1 + (N − n + 0.5) / (n + 0.5))` over the N lines of the root.

use std::collections::{BTreeSet, HashMap};

use rusqlite::{Connection, params};

use crate::error::Result;
use crate::index::Root;
use crate::token::tokens;

/// k1: how fast repeats of a term stop adding.
const SATURATION: f64 = 1.2;
/// b: how much a long line is discounted.
const LENGTH_WEIGHT: f64 = 0.75;
/// δ: what any line holding the term earns at least.
const FLOOR: f64 = 1.0;

/// A scored line, `at_line` numbered from 1 in the file at `path`, relative to the root.
pub struct Hit {
    pub path: String,
    pub at_line: i64,
    pub score: f64,
}

/// The best `limit` lines for the query, best first.
pub fn search(connection: &Connection, root: &Root, query: &str, limit: usize) -> Result<Vec<Hit>> {
    let (total, average): (f64, f64) = connection.query_row(
        "SELECT COUNT(*), COALESCE(AVG(line.tokens), 0) FROM line JOIN file USING (id_file) WHERE id_root = ?1",
        [root.id],
        |row| Ok((row.get::<_, i64>(0)? as f64, row.get(1)?)),
    )?;
    let mut statement = connection.prepare(
        "SELECT post.id_file, post.at_line, post.hits, line.tokens
         FROM post JOIN file USING (id_file)
         JOIN line ON line.id_file = post.id_file AND line.at_line = post.at_line
         WHERE post.field = ?1 AND post.text = ?2 AND file.id_root = ?3",
    )?;
    let mut scores: HashMap<(i64, i64), f64> = HashMap::new();
    for (field, text) in tokens(query).into_iter().collect::<BTreeSet<_>>() {
        let postings: Vec<(i64, i64, f64, f64)> = statement
            .query_map(params![field, text, root.id], |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get::<_, i64>(2)? as f64,
                    row.get::<_, i64>(3)? as f64,
                ))
            })?
            .collect::<rusqlite::Result<_>>()?;
        let holding = postings.len() as f64;
        let idf = (1.0 + (total - holding + 0.5) / (holding + 0.5)).ln();
        for (id_file, at_line, hits, length) in postings {
            let saturation = hits * (SATURATION + 1.0)
                / (hits + SATURATION * (1.0 - LENGTH_WEIGHT + LENGTH_WEIGHT * length / average));
            *scores.entry((id_file, at_line)).or_default() += idf * (saturation + FLOOR);
        }
    }
    let mut ranked: Vec<((i64, i64), f64)> = scores.into_iter().collect();
    ranked.sort_by(|left, right| right.1.total_cmp(&left.1).then(left.0.cmp(&right.0)));
    ranked.truncate(limit);
    let mut paths = connection.prepare("SELECT path FROM file WHERE id_file = ?1")?;
    ranked
        .into_iter()
        .map(|((id_file, at_line), score)| {
            Ok(Hit {
                path: paths.query_row([id_file], |row| row.get(0))?,
                at_line,
                score,
            })
        })
        .collect()
}
