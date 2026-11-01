# simple-markdown-search

> Rank the lines of markdown files against a query.

**Features:** BM25+ line ranking, Chinese and English tokens, no daemon, one SQLite index.

## Install

Download a SentencePiece `.model`, such as `tokenizer.model` from [taide/TAIDE-LX-7B](https://huggingface.co/taide/TAIDE-LX-7B), then:

```bash
SMS_VOCABULARY=path/to/tokenizer.model cargo install --path .
```

## Usage

```bash
sms set ~/notes diary wiki     # index the dirs under a root
sms get notes 下載 TAIDE        # search it, by path or basename
sms get notes 下載 TAIDE -n 5 -c 1
```

See [DESIGN.md](DESIGN.md) for how it works.

## Uninstall

```bash
cargo uninstall simple-markdown-search
rm -r ~/.sms
```
