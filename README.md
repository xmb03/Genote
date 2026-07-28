<p align="center">
  <img src="assets/logo.png" alt="Genote" width="600"/>
</p>

<h1 align="center">Genote</h1>

<p align="center">
  <a href="https://github.com/xmb03/Genote/releases"><img src="https://img.shields.io/github/v/release/xmb03/Genote?style=flat-square&label=release" alt="Release"></a>
  <a href="https://github.com/xmb03/Genote/commits"><img src="https://img.shields.io/github/last-commit/xmb03/Genote?style=flat-square&label=updated" alt="Last commit"></a>
  <a href="https://github.com/xmb03/Genote"><img src="https://img.shields.io/github/languages/top/xmb03/Genote?style=flat-square" alt="Language"></a>
  <a href="https://github.com/xmb03/Genote"><img src="https://img.shields.io/github/languages/code-size/xmb03/Genote?style=flat-square" alt="Code size"></a>
  <br>
  <a href="LICENSE"><img src="https://img.shields.io/badge/license-MIT-blue?style=flat-square" alt="License"></a>
  <img src="https://img.shields.io/badge/Rust-1.84+-orange?style=flat-square" alt="Rust">
  <a href="https://github.com/xmb03/Genote/actions"><img src="https://img.shields.io/github/actions/workflow/status/xmb03/Genote/release.yml?style=flat-square&label=build" alt="Build"></a>
</p>

<p align="center">
  Generate IT study notes using local LLMs via Ollama.<br>
  Feed it a topic and a few example notes — it writes a new one in your style.
</p>

---

## Features

- **Multi-topic batch** — generate several notes at once: `genote "Rust" "Go" "Python"`
- **Style learning** — reads your existing `.md` notes and mimics their structure (headings, lists, code blocks)
- **Covered-topics restriction** — limit the model to concepts you've already studied (`use_covered_topics = true`)
- **Hints** — pass extra instructions per topic via `(hint)` syntax without affecting the filename: `genote "Closures (skip FnOnce)"`
- **Profiles** — define multiple environments (work/home/server) and switch via `--profile`
- **CLI overrides** — every config option can be overridden inline: `-s big -l ru`
- **Language control** — generate notes in any language (`en`, `ru`, etc.)
- **Size control** — `small` (15–30 lines) or `big` (comprehensive)
- **Weak-mode for small models** — two-stage generation (`--weak-mode`) for models under 15B parameters. First analyzes your style, then writes the note in the same chat session. Slower but produces accurate style cloning
- **Logging control** — configure output verbosity: `log = true` for full debugging, `[log] prompt = false` to hide prompts, or suppress all non-error output
- **Issue prompt** — on any error, optionally open a pre‑filled GitHub issue to report bugs (`GENOTE_NO_ISSUE=1` to suppress)
- **Progress & timing** — see `[2/3] Sending request…` + elapsed time and token count per note
- **Graceful error handling** — one topic failing doesn't stop the rest of the batch
- **`~` expansion** — use `~/notes` in paths, home dir is resolved automatically
- **Config lookup** — `config.toml` is searched next to the binary first, then in CWD
- **Filename sanitization** — spaces and slashes in topic names become underscores
- **Smart example selection** — reads up to `notes_count` (default 7) example `.md` files, ignores non-`.md` files

## Demo

![Genote demo](assets/demo.gif)

## How it works

Genote reads your existing `.md` notes from a directory, sends them as style examples to an Ollama model, and generates a new note on the topic you specify. The more example notes you have, the better it matches your writing style.

When `use_covered_topics = true`, genote collects the filenames of your existing notes and tells the model to only use concepts from those covered topics. This prevents the model from introducing material you haven't studied yet.

When `--weak-mode` is enabled, genote splits generation into two stages sent to the same chat session. First it asks the model to analyze the style of your example notes. Then it generates the new note based on that analysis. This helps smaller models (under 15B parameters) follow formatting rules more accurately, since they don't have to process examples and write the note at the same time.

## Prerequisites

- [Ollama](https://ollama.ai) running locally (or remotely)
- A model pulled in Ollama (e.g. `gemma`, `llama3`, `mistral`)

## Installation

### From crates.io

Requires [Rust](https://rustup.rs).

```bash
cargo install genote
```

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

### Arch Linux (AUR)

```bash
yay -S genote        # from source
yay -S genote-bin    # pre-compiled binary
```

Also available in [Paru](https://github.com/Morganamilo/paru):

```bash
paru -S genote
```

### NixOS / Home Manager (Flake)

Add to your flake inputs:

```nix
{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    genote.url = "github:xmb03/Genote";  # or path: /path/to/Genote for local
  };

  outputs = { self, nixpkgs, genote, ... }:
    let
      system = "x86_64-linux";
      pkgs = nixpkgs.legacyPackages.${system};
    in {
      nixosConfigurations.your-host = nixpkgs.lib.nixosSystem {
        system = system;
        modules = [
          genote.nixosModules.default
          ({ inputs, ... }: {
            programs.genote = {
              enable = true;
              settings = {
                model = "llama3";
                notes_dir = "/home/youruser/notes";
                lang = "ru";
                note_size = "small";
                use_covered_topics = true;
              };
            };
          })
        ];
      };

      # Or for Home Manager (standalone or within NixOS)
      homeConfigurations."youruser" = pkgs.home-manager.lib.homeManagerConfiguration {
        pkgs = pkgs;
        modules = [
          genote.homeManagerModules.default
          ({ inputs, ... }: {
            programs.genote = {
              enable = true;
              settings = {
                model = "llama3";
                notes_dir = "~/notes";
                lang = "ru";
                note_size = "small";
                use_covered_topics = true;
              };
            };
          })
        ];
      };
    };
}
```

Then rebuild:

```bash
# NixOS
sudo nixos-rebuild switch --flake .#your-host

# Home Manager (standalone)
home-manager switch --flake .#youruser
```

The `genote` binary will be in PATH. Config is generated at:
- **Home Manager**: `~/.config/genote/config.toml` (XDG)
- **NixOS**: `/etc/genote/config.toml` (system-wide)

Example configs: [`home-manager-example.nix`](home-manager-example.nix) / [`nixos-example.nix`](nixos-example.nix)

## Setup

Copy the example config and adjust it to your environment:

```bash
cp config.toml.example config.toml
```

Edit `config.toml`.

### Flat config (simple)

All fields at the root level:

| Field | Description |
|---|---|
| `model` | The Ollama model to use (e.g. `gemma3`, `llama3`) |
| `api_url` | Your Ollama API endpoint |
| `notes_dir` | Directory containing your existing `.md` notes |
| `lang` | Language for the generated note (`en`, `ru`, etc.) |
| `note_size` | `small` for 15–30 lines or `big` for comprehensive |
| `notes_count` | How many example notes to use (default 7) |
| `use_covered_topics` | `true` — the model uses only concepts from existing note filenames. Default `false` |
| `weak_mode` | `true` — two-stage generation for models under 15B. Slower but better style cloning. Default `false` |
| `log` | Logging: `true` (all), `false` (errors only), or `{ prompt, response, status, timing }` for fine control |

### Profiles (multiple environments)

Define multiple profiles in `config.toml` and switch between them with `--profile`. Global root fields serve as defaults for all profiles. Profile fields override them. CLI flags override everything.

```toml
default = "work"

# global defaults applied to every profile
model = "llama3"
api_url = "http://127.0.0.1:11434/api/generate"
lang = "en"

[profile.work]
notes_dir = "~/work-notes"
note_size = "big"
notes_count = 10
use_covered_topics = true

[profile.home]
notes_dir = "~/personal-notes"
note_size = "small"
notes_count = 5
model = "mistral"
use_covered_topics = false
```

```bash
# uses default profile ("work")
genote "Rust ownership"

# switch to home profile
genote --profile home "Async Rust"
```

You need at least one `.md` file in your notes directory for genote to learn your writing style.

### Logging

Control what genote prints to stdout:

| Config | Effect |
|--------|--------|
| _(no `[log]` section)_ | Status + timing shown, prompts/responses hidden |
| `log = true` | Everything (prompts, responses, status, timing) |
| `log = false` | Only errors |
| `[log] prompt = false` | Everything except prompts |

Categories:

| Field | Description |
|-------|-------------|
| `prompt` | The full instruction sent to the model |
| `response` | The model's generated note text |
| `status` | Progress messages (`Sending request…`, `Saved: …`) |
| `timing` | Elapsed time and token count per note |

Works in both flat and profile mode:

```toml
# flat
[log]
prompt = false
response = true
status = true
timing = true

# or per-profile
[profile.work]
note_size = "big"
[profile.work.log]
prompt = false
```

### Config lookup order

1. Next to the binary (`target/release/config.toml`)
2. Current working directory (`./config.toml`)

## Usage

```bash
# basic usage — all settings from config.toml
genote "Rust ownership and borrowing"

# multiple notes in one command
genote "Rust Ownership" "Borrow Checker" "Smart Pointers"

# override size and language inline
genote -s big -l en "Rust ownership"

# pass extra instructions to the model — not included in filename
genote "Borrow checker (only &mut, skip &)"

# each topic independently supports hints
genote "Closures (skip FnOnce)" "Lifetimes (only elision)" "Generics (no traits)"

# disable covered-topics restriction for this run
genote --use-covered-topics=false "Async Rust"

# use fewer style examples
genote -n 3 "Pattern matching"

# weak-mode for models under 15B — two-stage generation, better style cloning
genote --weak-mode "Rust Ownership"

# weak-mode with all other options
genote --weak-mode -s big -l ru "Borrow Checker"
```

The generated note appears as a new `.md` file in your notes directory. Spaces and slashes in the topic name are replaced with underscores (e.g. `Rust Ownership` → `Rust_Ownership.md`).

During generation you'll see progress indicators and timing:

```
[1/3] Sending request (Model: llama3, Topic: "Borrow Checker", style examples: 5)…
  Took 4231 ms, generated tokens: 342
[2/3] Sending request (Model: llama3, Topic: "Smart Pointers", style examples: 5)…
  Took 5210 ms, generated tokens: 487
```

If one topic fails (network error, empty response, etc.), the rest continue unaffected.

### All CLI flags

Every config option can be overridden via the command line:

| Flag | Overrides |
|---|---|
| `-m`, `--model <name>` | `model` |
| `--api-url <url>` | `api_url` |
| `-d`, `--notes-dir <dir>` | `notes_dir` |
| `-l`, `--lang <lang>` | `lang` |
| `-s`, `--note-size <size>` | `note_size` |
| `-n`, `--notes-count <n>` | `notes_count` |
| `--use-covered-topics <bool>` | `use_covered_topics` |
| `--weak-mode` | `weak_mode` (flag, no value needed) |
| `--profile <name>` | profile selection |

```bash
genote --help
```

## License

MIT
