---
name: sms
description: Search markdown notes by their words, ranked, not by an exact string, with `sms get`. Use it instead of grep to find which line of a diary, wiki, or notes folder talks about something, when the exact wording is unknown.
---

# sms

`sms` ranks the lines of markdown files against a query and prints the best ones first.
A line may match part of the query, in any order; grep needs the exact string.

## Use

```
sms get <root> <query...> [-n LIMIT] [-c COVERAGE]
```

- `<root>` is a root's path or its basename, such as `anzu` for `/mnt/ark/family/anzu`.
- The query is the rest of the words; no quotes are needed.
- `-n` caps the hits (default 10); `-c` adds that many lines of context above and below each hit (default 0).

Each row is `score  path:line  text`.
The score is a fraction of the top hit's; a sharp drop marks where the relevant hits end.
The path is relative to the root, so `Read <root>/<path>` with an offset near the line opens the hit.

`get` reindexes changed files by itself; nothing needs running after editing notes.

## Index a root

```
sms set <root> <dir>...
```

Run it once per root with the dirs to index, and again after installing a new `sms` binary.
`sms set <root>` alone rebuilds with the dirs it already has.

## Write queries

- Use the words the note would use, in Chinese or English; mixing both is fine.
- Prefer a few distinctive words over a sentence: names, file names, numbers, coined terms.
- Several hits from one file at the top usually mean the whole file is the answer; read it.
