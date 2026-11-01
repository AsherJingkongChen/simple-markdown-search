//! token: the bag of a text, the tokens of both tokenizers concatenated. The two vocabularies
//! never meet: a bpe token enters the bag behind a `▁`, which unigram never emits, so a piece
//! `下` and the character `下` stay two terms.

use crate::{bpe, unigram};

pub fn tokens(text: &str) -> Vec<String> {
    let mut bag: Vec<String> = bpe::tokens(text)
        .into_iter()
        .map(|piece| format!("▁{piece}"))
        .collect();
    bag.extend(unigram::tokens(text));
    bag
}
