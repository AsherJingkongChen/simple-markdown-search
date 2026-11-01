//! bpe: SentencePiece BPE over the vocabulary that build.rs compiled in. It follows
//! SentencePiece's encoder: a `▁` is prepended and spaces become `▁`, the text splits into
//! characters, and the adjacent pair whose merge is a piece with the highest score merges
//! first, the leftmost on a tie, until no pair merges. A symbol that is no piece falls back to
//! its UTF-8 bytes as `<0xNN>` pieces.

use std::cmp::Ordering;
use std::collections::BinaryHeap;

include!(concat!(env!("OUT_DIR"), "/vocabulary.rs"));

/// The score of a piece, if the vocabulary has it, by a binary search over the sorted pieces.
fn score(piece: &str) -> Option<i32> {
    let (mut low, mut high) = (0, SCORES.len());
    while low < high {
        let middle = (low + high) / 2;
        let candidate = &TEXT[STARTS[middle] as usize..STARTS[middle + 1] as usize];
        match candidate.cmp(piece) {
            Ordering::Less => low = middle + 1,
            Ordering::Greater => high = middle,
            Ordering::Equal => return Some(SCORES[middle]),
        }
    }
    None
}

/// A run of the text, `start..end` in bytes, linked to its neighbours.
struct Symbol {
    start: usize,
    end: usize,
    previous: Option<usize>,
    next: Option<usize>,
}

/// A merge of two adjacent symbols, valid while the left still links to the right and the right
/// still ends where it ended when queued. A merged-away symbol links to nothing.
struct Pair {
    score: i32,
    left: usize,
    right: usize,
    end: usize,
}

impl Ord for Pair {
    fn cmp(&self, other: &Self) -> Ordering {
        self.score
            .cmp(&other.score)
            .then_with(|| other.left.cmp(&self.left))
    }
}

impl PartialOrd for Pair {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq for Pair {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}

impl Eq for Pair {}

pub fn tokens(text: &str) -> Vec<String> {
    if text.is_empty() {
        return Vec::new();
    }
    let text = format!("▁{}", text.replace(' ', "▁"));
    let bounds: Vec<usize> = text.char_indices().map(|(start, _)| start).collect();
    let mut symbols: Vec<Symbol> = bounds
        .iter()
        .enumerate()
        .map(|(index, &start)| Symbol {
            start,
            end: bounds.get(index + 1).copied().unwrap_or(text.len()),
            previous: index.checked_sub(1),
            next: (index + 1 < bounds.len()).then_some(index + 1),
        })
        .collect();
    let mut queue = BinaryHeap::new();
    let push = |queue: &mut BinaryHeap<Pair>, symbols: &[Symbol], left: usize, right: usize| {
        let (start, end) = (symbols[left].start, symbols[right].end);
        if let Some(score) = score(&text[start..end]) {
            queue.push(Pair {
                score,
                left,
                right,
                end,
            });
        }
    };
    for left in 0..symbols.len().saturating_sub(1) {
        push(&mut queue, &symbols, left, left + 1);
    }
    while let Some(pair) = queue.pop() {
        let (left, right) = (pair.left, pair.right);
        if symbols[left].next != Some(right) || symbols[right].end != pair.end {
            continue;
        }
        symbols[left].end = symbols[right].end;
        symbols[left].next = symbols[right].next.take();
        if let Some(next) = symbols[left].next {
            symbols[next].previous = Some(left);
        }
        if let Some(previous) = symbols[left].previous {
            push(&mut queue, &symbols, previous, left);
        }
        if let Some(next) = symbols[left].next {
            push(&mut queue, &symbols, left, next);
        }
    }
    let mut pieces = Vec::new();
    let mut at = (!symbols.is_empty()).then_some(0);
    while let Some(index) = at {
        let piece = &text[symbols[index].start..symbols[index].end];
        match score(piece) {
            Some(_) => pieces.push(piece.to_owned()),
            None => pieces.extend(piece.bytes().map(|byte| format!("<0x{byte:02X}>"))),
        }
        at = symbols[index].next;
    }
    pieces
}
