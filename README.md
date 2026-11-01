# simple-markdown-search

> Rank the lines of markdown files against a query.

**Features:** BM25+ line ranking, Bilingual tokenization, no daemon, single SQLite index.

## Install

Download a SentencePiece `.model`, such as `tokenizer.model` from [taide/TAIDE-LX-7B](https://huggingface.co/taide/TAIDE-LX-7B), then:

```bash
SMS_VOCABULARY=/path/to/tokenizer.model cargo install --path .
```

## Usage

```bash
# index the dirs under a root
sms set ~/notes diary wiki

# search lines under a root by path or basename
sms get ~/notes 下載 TAIDE
sms get notes 下載 TAIDE -n 5 -c 1
```

A query is the few words the line itself would hold, not a list of alternatives:
each query word the line lacks lowers its score, so after a miss, drop words instead of adding synonyms.

See [DESIGN.md](DESIGN.md) for how it works.

## Uninstall

```bash
cargo uninstall simple-markdown-search
rm -r ~/.sms
```
