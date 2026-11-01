# Design

What to know before reading the code of `sms`, the simple markdown search.

## Purpose

`sms` ranks the lines of markdown files against a query string.
It replaces grep for reading notes: a line may match part of the query, in any order, and the best match comes first.
It is a short-lived command-line program with no resident process.

## Commands

- `sms set <root> [dir...]` builds the index of a root.
  Given dirs, it replaces the root's list of dirs; without them, it reuses the stored list.
  Either way it rebuilds the root from scratch.
- `sms get <root> <query> [-n LIMIT] [-c COVERAGE]` searches a root.
  The root is a path or the basename of a stored root.
  Before searching, it reindexes the files that changed since the last call.
  It prints at most LIMIT hits (default 3), each with COVERAGE lines of context above and below (default 1).
  The output is grep's: a hit is `path:line:text`, a context line `path-line-text`, and each hit opens with a `-- score` line, the score a fraction of the top hit's.

## Units

- Query: one string.
- Token: a string cut from text by the tokenizers.
- Hit and scoring document: one line of a file, numbered from 1.
- Index: one root and its dirs; every `*.md` file under those dirs.
- Reindex: one file when `get` finds it changed; the whole root on `set`.

## Lifecycle

Two things change, and each has one trigger.

- Data: a markdown file is added, edited, or removed.
  `get` compares every file's `(path, mtime, bytes)` with the index and reindexes only the difference.
- Code: the binary or its vocabulary changes.
  The user runs `sms set <root>`, which rebuilds everything.

The index stores no version, since code never changes between two `set` calls.

## Tokenization

A line's tokens are the tokens of two tokenizers, concatenated into one bag; the query is cut the same way.
A token is its field, the tokenizer that cut it, and its text, so the piece `下` and the character `下` are two terms.

- `bpe`: SentencePiece BPE over a vocabulary compiled into the binary.
  It follows SentencePiece's encoder: a `▁` is prepended and spaces become `▁`; the text splits into characters; the adjacent pair whose merge is a piece with the highest score merges first, repeatedly; a symbol that is no piece falls back to its UTF-8 bytes as `<0xNN>` pieces.
- `unigram`: Unicode word boundaries (UAX #29), lowercased.
  A Han or hiragana character is a token by itself; a run of Latin letters, digits, or katakana is one token; punctuation, symbols, and spaces are dropped.

The bag rewards a line twice when a whole word of the query matches, once as a piece and once as its characters, and still finds a line that shares only some characters.

## Scoring

BM25+ with k1 = 1.2, b = 0.75, δ = 1, over the lines of the root:

```
score(q, line) = Σ over distinct terms t of q present in the line:
                 idf(t) · ( tf·(k1+1) / (tf + k1·(1 − b + b·length/average)) + δ )
idf(t) = ln(1 + (N − n_t + 0.5) / (n_t + 0.5))
```

N is the number of indexed lines in the root, n_t the number of those lines containing t, length a line's token count, average the mean length.
Printed scores are fractions of the top hit's score.

## Storage

One SQLite file at `~/.sms/index.db`, shared by every root.

```sql
root (id_root INTEGER PRIMARY KEY, path TEXT UNIQUE)
dir  (id_root REFERENCES root ON DELETE CASCADE, name TEXT, PRIMARY KEY (id_root, name))
file (id_file INTEGER PRIMARY KEY, id_root REFERENCES root ON DELETE CASCADE,
      path TEXT, mtime INTEGER, bytes INTEGER, UNIQUE (id_root, path))
line (id_file REFERENCES file ON DELETE CASCADE, at_line INTEGER, tokens INTEGER,
      PRIMARY KEY (id_file, at_line)) WITHOUT ROWID
term (id_term INTEGER PRIMARY KEY, field INTEGER, text TEXT, UNIQUE (field, text))
post (id_term REFERENCES term, id_file REFERENCES file ON DELETE CASCADE, at_line INTEGER, hits INTEGER,
      PRIMARY KEY (id_term, id_file, at_line)) WITHOUT ROWID
```

- `file.path` is relative to the root; `mtime` is in nanoseconds.
- `term.field` is 0 for bpe and 1 for unigram.
- `line.tokens` is the line's length for BM25+; a line with no tokens has no row.
- `post.hits` is the term's count in the line.
  Its primary key keeps one term's postings contiguous, so a query term is one range read.
- Deleting a file row deletes its lines and postings through the cascades, so reindexing a file is a delete and an insert.
- The text itself is not stored: hits are printed by reading the markdown files, the only source of truth.

## Build

The vocabulary is a SentencePiece `.model` the user downloads, named by `SMS_VOCABULARY` at build time.
`build.rs` reads it and generates a sorted static table of pieces and scores, so nothing is parsed at run time and the repository holds no model.
The table is one string of all pieces, a `u32` start per piece, and an `i32` score per piece: piece `i` is `TEXT[STARTS[i]..STARTS[i + 1]]`, and the table holds no pointers.

## Files

- `build.rs`: `.model` to the static piece table.
- `src/main.rs`: argument parsing and printing.
- `src/bpe.rs`: the BPE tokenizer over the static table.
- `src/unigram.rs`: the word-boundary tokenizer.
- `src/token.rs`: both tokenizers into one bag.
- `src/index.rs`: the schema, `set`, and the reindex that `get` runs first.
- `src/search.rs`: BM25+ over the index.

## Decisions

- One line is the hit and the scoring document: the result is a table, and a row per line is what a grep user reads and what an agent opens with `path:line`.
- BM25+ over BM25, TF-IDF, and query likelihood: on DRCD line retrieval (3,493 questions) hit@1 was .775, against .756, .669, and at best .770.
- Both tokenizers over either alone: hit@1 .790, against .775 for unigram and .767 for bpe.
- A field per tokenizer, as in Lucene's `(field, text)` terms: merging the two tokenizers' equal strings into one term dropped hit@1 from .791 to .774, since a single-character piece doubled that character's weight.
- No embedding model: embeddinggemma reached hit@1 .787, below BM25+ with both tokenizers, at the cost of loading 1.2 GB per call.
- No resident process: a query reads a few posting ranges in milliseconds, and the vocabulary is compiled in, so startup has nothing to load.
- SQLite without FTS5: FTS5 scores whole rows with its own BM25; the postings table gives line scores and BM25+ directly.
- No variant-character normalization: the notes are written in Traditional Chinese by their readers, so variants are rare.
- Crates: rusqlite (bundled SQLite), unicode-segmentation, lexopt, and walkdir at run time; sentencepiece-model at build time only.
  The BPE encoder and BM25+ are written here: no crate encodes from a compiled-in table, and the formula is a few lines.
