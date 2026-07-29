# Genote — agent guide

Minimal Rust CLI that generates IT study notes via local LLMs. Single binary, two source files, no tests.

## Build & run

```bash
cargo build --release          # binary at target/release/genote
cp config.toml.example config.toml   # then edit (config.toml is gitignored)
cargo run --release -- "Your topic"
```

No test, lint, format, or typecheck commands exist — `cargo build` is the only check.

## Config quirks

- `config.toml` is looked up **next to the binary first**, then CWD.
- Flat mode: all fields at root. Profile mode: `[profile.xxx]` sections + global root defaults.
- Merge order: global defaults → profile → CLI flags (each overrides the previous).
- `provider` field: `ollama` (default), `openai` (OpenAI, vLLM, DeepSeek, OpenRouter, GitHub Models, etc.), `llamacpp`, `anthropic` (Claude), or `gemini` (Google).
- `api_key` stored in system keyring. Set via `--api-key sk-...`, reset via `--api-key` (no value), or prompted on first launch.

## CLI quirks

- `(hint)` in topic passes extra instructions to the model without affecting the output filename:
  ```
  genote "Borrow checker (only &mut, skip &)"
  ```
- `use_covered_topics`: reads `.md` filenames in `notes_dir`, strips extension, replaces `_` with spaces → passed as topic restrictions.
- `note_size` must be exactly `small` (15–30 lines) or `big` (comprehensive).
- `weak_mode` enables two-stage generation via chat endpoint (style analysis → note generation). Uses `weak_mode` CLI flag or `weak_mode = true` in config.
- `--provider` / `-p` to set provider from CLI.

## Architecture

- `src/main.rs` — CLI, config resolution, topic loop, note assembly.
- `src/provider.rs` — `Provider` enum with URL/body/response logic for Ollama, OpenAI-compatible (vLLM, LM Studio), and llama.cpp.
- Unsorted `.md` files in `notes_dir` used as style examples (up to `notes_count`, default 7).
- Output saved to `notes_dir/{topic_slug}.md` (spaces/slashes → underscores).
- Weak mode uses chat endpoint; normal mode uses generate/completion endpoint (varies by provider).

## CI / release

GitHub Actions: `cargo build --release` on `v*` tag pushes → `.tar.gz` attached to release.

## Skills (`.opencode/skills/`)

- `karpathy-guidelines` — behavioral caution for LLM coding
- `ponytail` — minimal/simple solution bias
