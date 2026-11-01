//! Compile the SentencePiece vocabulary named by `SMS_VOCABULARY` into `vocabulary.rs`: the
//! pieces a merge can produce, normal and user-defined, sorted by their bytes for a binary
//! search. Piece `i` is `TEXT[STARTS[i]..STARTS[i + 1]]` and scores `SCORES[i]`, so the table
//! holds no pointers.

use std::fmt::Write;

use sentencepiece_model::{SentencePieceModel, Type};

fn main() {
    println!("cargo::rerun-if-env-changed=SMS_VOCABULARY");
    let path = std::env::var("SMS_VOCABULARY")
        .expect("SMS_VOCABULARY should name a SentencePiece .model file, such as TAIDE-LX-7B's tokenizer.model");
    println!("cargo::rerun-if-changed={path}");
    let model = SentencePieceModel::from_file(&path)
        .unwrap_or_else(|error| panic!("{path} should be a SentencePiece model: {error}"));
    let mut pieces: Vec<(&str, f32)> = model
        .pieces()
        .iter()
        .filter(|piece| matches!(piece.r#type(), Type::Normal | Type::UserDefined))
        .map(|piece| (piece.piece(), piece.score()))
        .collect();
    pieces.sort_by(|left, right| left.0.cmp(right.0));
    let text: String = pieces.iter().map(|(piece, _)| *piece).collect();
    let mut starts = vec![0];
    starts.extend(pieces.iter().scan(0, |end, (piece, _)| {
        *end += piece.len();
        Some(*end)
    }));
    let mut source = format!("static TEXT: &str = {text:?};\n");
    writeln!(
        source,
        "static STARTS: [u32; {}] = {starts:?};",
        starts.len()
    )
    .expect("a String should take writes");
    let scores: Vec<i32> = pieces.iter().map(|(_, score)| *score as i32).collect();
    writeln!(
        source,
        "static SCORES: [i32; {}] = {scores:?};",
        scores.len()
    )
    .expect("a String should take writes");
    let out = std::env::var("OUT_DIR").expect("cargo should set OUT_DIR");
    std::fs::write(format!("{out}/vocabulary.rs"), source).expect("OUT_DIR should be writable");
}
