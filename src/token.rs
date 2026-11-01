//! token: the bag of a text, the tokens of both tokenizers concatenated. A token is its field,
//! the tokenizer that cut it, and its text, so the piece `下` and the character `下` are two
//! terms. A lone `▁`, the word start that merged with nothing, is left out: it is in most
//! lines, so it ranks nothing and every query would read its postings.

use crate::{bpe, unigram};

pub const BPE: i64 = 0;
pub const UNIGRAM: i64 = 1;

/// `(field, text)`.
pub type Token = (i64, String);

pub fn tokens(text: &str) -> Vec<Token> {
    let mut bag: Vec<Token> = bpe::tokens(text)
        .into_iter()
        .filter(|piece| piece != "▁")
        .map(|piece| (BPE, piece))
        .collect();
    bag.extend(unigram::tokens(text).map(|word| (UNIGRAM, word)));
    bag
}
