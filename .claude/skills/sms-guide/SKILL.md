---
name: sms-guide
description: How to search markdown notes with sms (simple-markdown-search). Read before running `sms get` or `sms set`, and before saying something is not in the notes.
allowed-tools: Bash(sms get *)
---

# sms

`sms get <root> <words...>` scores every line of the notes by how many of the query's words it holds, and prints the best three.
It is neither grep nor a web search, and the habits of both make it miss.

## Write the line, not the question

A query is the few words the line itself would hold, in the language the note is written in.
Every query word the line lacks lowers its score, so each extra word costs.

```bash
sms get notes 下載 TAIDE                          # the words on the line
sms get notes how to download the TAIDE tokenizer # a web search: most words are on no line
sms get notes "TAIDE|taide|Taide"                 # grep: no operators or quotes; the variants only lower the score
```

## A miss is not an absence

No match means those words are not on one line together, not that the note is missing.
Drop the word you are least sure of and search again; never add synonyms or variants, which only lower the score.
Say it is not in the notes only after one or two words still miss.

## Reading hits

- Each hit is `path:line:text` under a `-- score` line, with one line of context above and below as `path-line-text`.
- A score near 1 holds every query word; lines under 0.3 are never shown.
- To read around a hit, open the file at that line; `-n` and `-c` change the count of hits and context lines.

## Roots

- The root is a path or the basename of an indexed root. It is searched whole, with no filter by dir, so search only a root whose every dir you may read.
- Edits are reindexed on the next `get`. `sms set <root> [dir...]` rebuilds a root, and is needed only after a new binary or vocabulary.
