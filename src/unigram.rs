//! unigram: Unicode word boundaries (UAX #29), lowercased. A Han or hiragana character is a
//! token by itself; a run of Latin letters, digits, or katakana is one token; punctuation,
//! symbols, and spaces are dropped.

use unicode_segmentation::UnicodeSegmentation;

pub fn tokens(text: &str) -> impl Iterator<Item = String> + '_ {
    text.unicode_words().map(str::to_lowercase)
}
