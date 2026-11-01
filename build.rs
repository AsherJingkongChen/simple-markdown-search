//! Compile the SentencePiece vocabulary named by `SMS_VOCABULARY` into `vocabulary.rs`: the
//! pieces a merge can produce, normal and user-defined, sorted by their bytes for a binary
//! search, each with its score.

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
    let mut source = format!("static PIECES: [(&str, f32); {}] = [\n", pieces.len());
    for (piece, score) in pieces {
        writeln!(source, "    ({piece:?}, {score:?}),").expect("a String should take writes");
    }
    source.push_str("];\n");
    let out = std::env::var("OUT_DIR").expect("cargo should set OUT_DIR");
    std::fs::write(format!("{out}/vocabulary.rs"), source).expect("OUT_DIR should be writable");
}
