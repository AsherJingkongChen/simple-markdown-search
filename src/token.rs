//! token: the bag of a text, the tokens of both tokenizers concatenated. A token is its field,
//! the tokenizer that cut it, and its text, so the piece `下` and the character `下` are two
//! terms.

use crate::{bpe, unigram};

pub const BPE: i64 = 0;
pub const UNIGRAM: i64 = 1;

/// `(field, text)`.
pub type Token = (i64, String);

pub fn tokens(text: &str) -> Vec<Token> {
    let mut bag: Vec<Token> = bpe::tokens(text)
        .into_iter()
        .map(|piece| (BPE, piece))
        .collect();
    bag.extend(unigram::tokens(text).map(|word| (UNIGRAM, word)));
    bag
}
