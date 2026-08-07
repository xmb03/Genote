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
  <img src="https://img.shields.io/badge/Rust-1.85+-orange?style=flat-square" alt="Rust">
  <a href="https://github.com/xmb03/Genote/actions"><img src="https://img.shields.io/github/actions/workflow/status/xmb03/Genote/release.yml?style=flat-square&label=build" alt="Build"></a>
</p>

<p align="center">
  Generate IT study notes using local LLMs and cloud APIs.<br>
  Feed it a topic and a few example notes — it writes a new one in your style.
</p>

---

## Features

- **Multi-provider** — Ollama (local), OpenAI-compatible (vLLM, DeepSeek, OpenRouter, GitHub Models), Anthropic Claude, Google Gemini, llama.cpp. Switch via `--provider` or `config.toml`
- **API key in config** — `api_key = "sk-..."` lives in `config.toml` (or per-profile). No keyring dependencies needed. Override per-run with `--api-key`
- **Cross‑platform** — runs on Linux, macOS, and Windows
- **Multi-topic batch** — generate several notes at once: `genote "Rust" "Go" "Python"`
- **Style learning** — reads your existing `.md` notes and mimics their structure (headings, lists, code blocks)
- **Skill files** — load explicit style rules (what to do / what NOT to do) from a file via `skill` in config or `--skill`. Injected both into the model's system prompt and as a STRICT preamble of the user prompt (weak-mode stage 2 keeps it in the system prompt only; llama.cpp has no system field, so it receives the preamble only)
- **Covered-topics context** — the model is told which topics you've already covered (`use_covered_topics = true`): it must NOT re-explain them, only build on them
- **Hints** — pass extra instructions per topic via `(hint)` syntax without affecting the filename: `genote "Closures (skip FnOnce)"`
- **Profiles** — define multiple environments (work/home/server) and switch via `--profile`
- **CLI overrides** — every config option can be overridden inline: `-s big -l ru`
- **Language control** — generate notes in any language (`en`, `ru`, etc.)
- **Size control** — `small` (exactly 25–30 lines, line count is verified with a warning), `mid` (45–68 lines, verified with a warning), or `big` (comprehensive, up to 8192 tokens; `small` caps at 4096, `mid` at 6144)
- **Weak-mode for small models** — two-stage generation (`--weak-mode`) for models under 15B parameters. First analyzes your style, then writes the note in the same chat session. Slower but produces accurate style cloning
- **Logging control** — configure output verbosity: `log = true` for full debugging, `[log] prompt = false` to hide prompts, or suppress all non-error output
- **Issue prompt** — on any error, optionally open a pre‑filled GitHub issue to report bugs (`GENOTE_NO_ISSUE=1` to suppress)
- **Progress & timing** — see `[2/3] Sending request…` + elapsed time and token count per note
- **Graceful error handling** — one topic failing doesn't stop the rest of the batch
- **Automatic retries** — transient failures (HTTP 429, 5xx) are retried up to 2 times (3 attempts total) with short backoff; each request has a hard 300 s timeout
- **`~` expansion** — use `~/notes` in paths, home dir is resolved automatically
- **Config lookup** — `config.toml` is looked up in a single user config directory (see [Config lookup order](#config-lookup-order))
- **Filename sanitization** — spaces, slashes, and `\ : * ? " < > |` in topic names become underscores
- **Smart example selection** — reads up to `notes_count` (default 7) example `.md` files, ignores non-`.md` files; `notes_include` in config restricts examples to a specific file list

## Demo

![Genote demo](assets/demo.gif)

## How it works

Genote reads your existing `.md` notes from a directory, sends them as style examples to an LLM, and generates a new note on the topic you specify. It supports Ollama, OpenAI-compatible APIs, Anthropic Claude, Google Gemini, and llama.cpp — local or cloud. The more example notes you have, the better it matches your writing style.

When `use_covered_topics = true`, genote collects the filenames of all `.md` notes in your notes directory (regardless of `notes_include`) and tells the model that these topics are already known to the reader: it must NOT re-explain, redefine, or recap them, but instead build on them as a foundation while explaining the new topic.

When `--weak-mode` is enabled, genote splits generation into two stages sent to the same chat session. First it asks the model to analyze the style of your example notes. Then it generates the new note based on that analysis. This helps smaller models (under 15B parameters) follow formatting rules more accurately, since they don't have to process examples and write the note at the same time.

Additional behavior details:

- For `small` notes, genote counts the non-empty lines of the generated text and prints a warning if the count falls outside 25–30; for `mid` notes the range is 45–68. The model is not re-run, it's a warning only.
- Writing over an existing note file prints `Warning: overwriting <path>`.
- Unknown keys in `config.toml` are not silently ignored — each prints `Warning: unknown config key(s) in ...`.

## Prerequisites

Depending on your provider:
- **Ollama** — [Ollama](https://ollama.ai) running locally with a model pulled (`gemma`, `llama3`, etc.)
- **OpenAI-compatible** — an endpoint (vLLM, DeepSeek, OpenRouter, GitHub Models) + `api_key` in `config.toml`
- **Anthropic Claude** — `api_key` in `config.toml`
- **Google Gemini** — `api_key` in `config.toml`
- **llama.cpp** — llama.cpp server running

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

Grab the latest binary for your platform from the [Releases page](https://github.com/xmb03/Genote/releases).

```bash
# Linux x86_64
curl -L https://github.com/xmb03/Genote/releases/latest/download/genote-x86_64-unknown-linux-gnu.tar.gz | tar xz
sudo mv genote /usr/local/bin/

# macOS (Apple Silicon)
curl -L https://github.com/xmb03/Genote/releases/latest/download/genote-aarch64-apple-darwin.tar.gz | tar xz
sudo mv genote /usr/local/bin/

# Windows (x86_64) — run in PowerShell
curl -LO https://github.com/xmb03/Genote/releases/latest/download/genote-x86_64-pc-windows-msvc.tar.gz
tar xzf genote-x86_64-pc-windows-msvc.tar.gz
# Move genote.exe to a folder in your PATH
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

### Home Manager (Flake)

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
home-manager switch --flake .#youruser
```

The `genote` binary will be in PATH. Config is generated at `~/.config/Genote/config.toml` (capital `G`, matching the binary's [config lookup order](#config-lookup-order)).

Example config: [`home-manager-example.nix`](home-manager-example.nix)

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
| `provider` | API backend: `ollama` (default), `openai`, `llamacpp`, `anthropic`, `gemini` |
| `model` | Model name (e.g. `gemma3`, `llama3`, `gpt-4o`, `claude-sonnet-4`) |
| `api_url` | Your API endpoint (e.g. `http://127.0.0.1:11434` for Ollama). Base URL only — known endpoint suffixes (`/api/generate`, `/v1/chat/completions`, `/completion`, etc.) are stripped automatically; an unrecognized trailing path segment is dropped too. A full Gemini `:generateContent` URL (even with `?key=...`) is normalized the same way. Because of this, don't put a custom path prefix here (e.g. `http://host/myproxy/`) — pass the full endpoint URL instead |
| `api_key` | API key for cloud providers (`openai`/`anthropic`/`gemini`). Plaintext in config; precedence: `--api-key` CLI > `GENOTE_API` env > config > interactive prompt. The interactive prompt appears only for auth-requiring providers; `ollama`/`llamacpp` get an empty key silently |
| `notes_dir` | Directory containing your existing `.md` notes |
| `lang` | Language for the generated note (`en`, `ru`, etc.) |
| `note_size` | `small` for 25–30 lines, `mid` for 45–68 lines, or `big` for comprehensive |
| `notes_count` | How many example notes to use (default 7) |
| `notes_include` | Optional list of filenames in `notes_dir` (`.md` optional). If set, ONLY these notes are used as style examples; missing files are skipped with a warning, duplicates are ignored. The covered-topics list is NOT restricted by it — it always includes all `.md` files |
| `use_covered_topics` | `true` — the model treats existing note filenames as already-known topics: it must not re-explain them, only build on them. Default `false` |
| `weak_mode` | `true` — two-stage generation for models under 15B. Slower but better style cloning. Default `false` |
| `temperature` | Optional sampling temperature (e.g. `0.7`). Unset = provider default |
| `skill` | Path to a style-rules file. Injected into the model's system prompt AND as a STRICT preamble of the user prompt (weak-mode stage 2: system prompt only) |
| `log` | Logging: `true` (all), `false` (errors only), or `{ prompt, response, status, timing }` for fine control |
| `default` | Profile used when `--profile` is not passed (only meaningful when `[profile]` sections exist) |

### Profiles (multiple environments)

Define multiple profiles in `config.toml` and switch between them with `--profile`. Global root fields serve as defaults for all profiles. Profile fields override them. CLI flags override everything. `default = "work"` selects the profile used when `--profile` is not passed; if profiles exist but no `default` is set and no `--profile` is passed, genote warns and falls back to root-level settings. Passing `--profile` when the config has no `[profile]` sections is an error.

```toml
default = "work"

# global defaults applied to every profile
model = "llama3"
api_url = "http://127.0.0.1:11434"
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

An inline table works too:

```toml
log = { prompt = false, response = false }   # only status + timing
```

### Config lookup order

`config.toml` is looked up in a single user config directory (no lookup next to the binary, no CWD):

1. **Linux**: `$XDG_CONFIG_HOME/Genote/config.toml` or `~/.config/Genote/config.toml` (capital `G`, always)
2. **macOS**: `~/Library/Application Support/Genote/config.toml`
3. **Windows**: `%APPDATA%\Genote\config.toml`

## Usage

```bash
# basic usage — all settings from config.toml
genote "Rust ownership and borrowing"

# multiple notes in one command
genote "Rust Ownership" "Borrow Checker" "Smart Pointers"

# override size and language inline
genote -s big -l en "Rust ownership"

# use an OpenAI-compatible provider (vLLM, DeepSeek, OpenRouter, etc.)
genote -p openai -m gpt-4o "Rust ownership"

# pass an API key for this run only (persist in config.toml instead)
genote --api-key "sk-..." "Rust ownership"

# pass extra instructions to the model — not included in filename
genote "Borrow checker (only &mut, skip &)"

# each topic independently supports hints
genote "Closures (skip FnOnce)" "Lifetimes (only elision)" "Generics (no traits)"

# don't treat existing notes as already-known topics for this run
genote --use-covered-topics=false "Async Rust"

# use fewer style examples
genote -n 3 "Pattern matching"

# weak-mode for models under 15B — two-stage generation, better style cloning
genote --weak-mode "Rust Ownership"

# use a skill file — style rules go to the system prompt, notes stay in the user prompt
genote --skill ~/skills/concise.md "Borrow Checker"

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

If one topic fails (network error, empty response, etc.), the rest continue unaffected. If any topic fails, the process exits with code `1` (for scripts/CI).

Validation errors also exit with code `1` before any network call: missing `config.toml` or unparseable config; unknown provider; `note_size` other than `small`/`mid`/`big`; missing `model`/`api_url`/`notes_dir`/`lang`; non-existent `notes_dir`; `notes_count = 0`; no `.md` files in `notes_dir` (or none of the `notes_include` files found); a topic that is empty after hint removal (e.g. `"(hint)"` with no topic text); `--profile` without `[profile]` sections; unknown profile name.

With cloud providers (`openai`/`anthropic`/`gemini`) and more than one topic, genote generates all notes concurrently; local providers run topics sequentially.

### All CLI flags

Every config option can be overridden via the command line:

| Flag | Overrides |
|---|---|
| `-m`, `--model <name>` | `model` |
| `--api-url <url>` | `api_url` |
| `-p`, `--provider <name>` | `provider` |
| `--api-key [key]` | `api_key` — override for this run; no value = interactive prompt (not saved to config). `GENOTE_API` env var works too |
| `-d`, `--notes-dir <dir>` | `notes_dir` |
| `-l`, `--lang <lang>` | `lang` |
| `-s`, `--note-size <size>` | `note_size` |
| `-n`, `--notes-count <n>` | `notes_count` |
| `--use-covered-topics [bool]` | `use_covered_topics` — bare flag = true, `--use-covered-topics=false` to disable |
| `--weak-mode [bool]` | `weak_mode` — bare flag = true, `--weak-mode=false` to disable |
| `--skill <path>` | `skill` |
| `--profile <name>` | profile selection |

```bash
genote --help
```

## Testing

```bash
./test/run.sh          # full offline suite — mock LLM API, no real server needed
./test/run.sh -f NAME  # run a single case
./test/run.sh --live   # include the real-ollama smoke test (auto-skipped when offline)
```

Bash + python3, no test framework. A new case is just a `test_*()` function added to `test/cases/*.sh` — it runs automatically. Full per-case logs (stdout, stderr, recorded API requests, config, generated notes) land in `test/logs/<timestamp>/<case>/`.

## License

MIT
