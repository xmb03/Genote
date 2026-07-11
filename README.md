# genote

A CLI tool that generates IT notes using local LLMs via Ollama. Feed it a topic and a few example notes, and it writes a new one in your style.

## How it works

genote reads your existing .md notes from a directory, sends them as style examples to an Ollama model, and generates a new note on the topic you specify. The more example notes you have, the better it matches your writing style.

## Prerequisites

- [Ollama](https://ollama.ai) running locally (or remotely)
- A model pulled in Ollama (e.g. `gemma`, `llama3`, `mistral`)

## Installation

```bash
git clone https://github.com/your-username/genote
cd genote
cargo build --release
```

The binary will be at `target/release/genote`.

## Setup

Copy the example config and adjust it:

```bash
cp config.toml.example config.toml
```

Edit `config.toml`:

| Field        | Description                                          |
|-------------|------------------------------------------------------|
| `model`     | The Ollama model to use (e.g. `gemma3`, `llama3`)    |
| `api_url`   | Your Ollama API endpoint                             |
| `notes_dir` | Directory containing your existing .md notes         |
| `lang`      | Language for the generated note (`en`, `ru`, etc.)   |
| `note_size` | `small` for concise notes or `big` for detailed ones |
| `notes_count` | How many example notes to use (default 7)          |

## Usage

```bash
genote "Rust ownership and borrowing"
```

The generated note appears as a new .md file in your notes directory.

## License

MIT
