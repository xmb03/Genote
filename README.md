# Genote

A CLI tool that generates IT notes using local LLMs via Ollama. Feed it a topic and a few example notes, and it writes a new one in your style.

## How it works

genote reads your existing .md notes from a directory, sends them as style examples to an Ollama model, and generates a new note on the topic you specify. The more example notes you have, the better it matches your writing style.

When `use_covered_topics = true` in the config, genote collects the filenames of your existing notes and tells the model to only use concepts from those covered topics. This prevents the model from introducing material you haven't studied yet — it stays within the boundaries of your known topics.

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
curl -L https://github.com/xmb03/Genote/releases/latest/download/genote-v0.1.0-linux-x86_64.tar.gz | tar xz
sudo mv genote /usr/local/bin/
```

## Setup

Copy the example config and adjust it to your environment:

```bash
cp config.toml.example config.toml
```

Edit `config.toml`:

| Field          | Description                                          |
|----------------|------------------------------------------------------|
| `model`        | The Ollama model to use (e.g. `gemma3`, `llama3`)    |
| `api_url`      | Your Ollama API endpoint                             |
| `notes_dir`    | Directory containing your existing .md notes         |
| `lang`         | Language for the generated note (`en`, `ru`, etc.)   |
| `note_size`    | `small` for concise notes or `big` for detailed ones |
| `notes_count`  | How many example notes to use (default 7)            |
| `use_covered_topics` | `true` — the model uses only concepts from existing note filenames (covered topics). Default `false` |

You need to have at least one `.md` file in your notes directory for genote to learn your writing style.

## Usage

```bash
genote "Rust ownership and borrowing"
```

The generated note appears as a new .md file in your notes directory.

## License

MIT
