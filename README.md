# Genote

A CLI tool that generates IT notes using local LLMs via Ollama. Feed it a topic and a few example notes, and it writes a new one in your style.

## Demo

![Genote demo](demo.gif)

## How it works

genote reads your existing .md notes from a directory, sends them as style examples to an Ollama model, and generates a new note on the topic you specify. The more example notes you have, the better it matches your writing style.

When `use_covered_topics = true`, genote collects the filenames of your existing notes and tells the model to only use concepts from those covered topics. This prevents the model from introducing material you haven't studied yet.

## Prerequisites

- [Ollama](https://ollama.ai) running locally (or remotely)
- A model pulled in Ollama (e.g. `gemma`, `llama3`, `mistral`)

## Installation

### From source

Requires [Rust](https://rustup.rs).

```bash
git clone https://github.com/xmb03/Genote.git
cd Genote
cargo build --release
```

The binary will be at `target/release/genote`.

### Binary download

Grab the latest binary from the [Releases page](https://github.com/xmb03/Genote/releases).

```bash
curl -L https://github.com/xmb03/Genote/releases/latest/download/genote-linux-x86_64.tar.gz | tar xz
sudo mv genote /usr/local/bin/
```

## Setup

Copy the example config and adjust it to your environment:

```bash
cp config.toml.example config.toml
```

Edit `config.toml`.

### Flat config (simple)

All fields at the root level:

| Field               | Description                                          |
|---------------------|------------------------------------------------------|
| `model`             | The Ollama model to use (e.g. `gemma3`, `llama3`)    |
| `api_url`           | Your Ollama API endpoint                             |
| `notes_dir`         | Directory containing your existing .md notes         |
| `lang`              | Language for the generated note (`en`, `ru`, etc.)   |
| `note_size`         | `small` for concise notes or `big` for detailed ones |
| `notes_count`       | How many example notes to use (default 7)            |
| `use_covered_topics` | `true` — the model uses only concepts from existing note filenames. Default `false` |

### Profiles (multiple environments)

Define multiple profiles in `config.toml` and switch between them with `--profile`:

```toml
default = "work"

# global defaults applied to every profile
model = "llama3"
api_url = "http://127.0.0.1:11434/api/generate"
lang = "en"

[profile.work]
notes_dir = "~/work-notes"
note_size = "big"

[profile.home]
notes_dir = "~/personal-notes"
note_size = "small"
model = "mistral"
```

```bash
# uses default profile ("work")
genote "Rust ownership"

# switch to home profile
genote --profile home "Async Rust"
```

Global fields at root level serve as defaults for all profiles. Profile fields override them. CLI flags override everything.

You need at least one `.md` file in your notes directory for genote to learn your writing style.

## Usage

```bash
# basic usage — all settings from config.toml
genote "Rust ownership and borrowing"

# override size and language inline
genote -s big -l en "Rust ownership"

# pass extra instructions to the model — not included in filename
genote "Borrow checker (only &mut, skip &)"

# disable covered-topics restriction for this run
genote --use-covered-topics=false "Async Rust"

# use fewer style examples
genote -n 3 "Pattern matching"
```

The generated note appears as a new `.md` file in your notes directory.

### All CLI flags

Every config option can be overridden via the command line:

| Flag                           | Overrides            |
|--------------------------------|----------------------|
| `-m`, `--model <name>`        | `model`              |
| `--api-url <url>`             | `api_url`            |
| `-d`, `--notes-dir <dir>`     | `notes_dir`          |
| `-l`, `--lang <lang>`         | `lang`               |
| `-s`, `--note-size <size>`    | `note_size`          |
| `-n`, `--notes-count <n>`     | `notes_count`        |
| `--use-covered-topics <bool>` | `use_covered_topics` |
| `--profile <name>`           | profile selection    |

```bash
genote --help
```

## License

MIT
