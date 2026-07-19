# Genote — agent guide

Minimal Rust CLI that generates IT study notes via Ollama. Single binary, single source file, no tests.

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

## CLI quirks

- `(hint)` in topic passes extra instructions to the model without affecting the output filename:
  ```
  genote "Borrow checker (only &mut, skip &)"
  ```
- `use_covered_topics`: reads `.md` filenames in `notes_dir`, strips extension, replaces `_` with spaces → passed as topic restrictions.
- `note_size` must be exactly `small` (15–30 lines) or `big` (comprehensive).

## Architecture

- Single file `src/main.rs` (no lib.rs, no modules).
- Unsorted `.md` files in `notes_dir` used as style examples (up to `notes_count`, default 7).
- Output saved to `notes_dir/{topic_slug}.md` (spaces/slashes → underscores).
- Depends on **Ollama** running at `api_url` (default `http://127.0.0.1:11434/api/generate`).

## CI / release

GitHub Actions: `cargo build --release` on `v*` tag pushes → `.tar.gz` attached to release.

## Skills (`.opencode/skills/`)

- `karpathy-guidelines` — behavioral caution for LLM coding
- `ponytail` — minimal/simple solution bias
