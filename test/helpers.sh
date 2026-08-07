#!/usr/bin/env bash
# Shared helpers for Genote test cases (sourced by run.sh and each case).

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(dirname "$SCRIPT_DIR")"

BIN=""
SANDBOX=""
MOCK_URL=""
MOCK_LOG=""
CODE=""
OUT=""
ERR=""

if [ -t 1 ] && [ -z "${NO_COLOR:-}" ]; then
  C_RED=$'\033[31m'; C_YELLOW=$'\033[33m'; C_RST=$'\033[0m'
else
  C_RED=""; C_YELLOW=""; C_RST=""
fi

TOOL_TIMEOUT=""
if command -v timeout >/dev/null 2>&1; then
  TOOL_TIMEOUT=timeout
elif command -v gtimeout >/dev/null 2>&1; then
  TOOL_TIMEOUT=gtimeout
else
  echo "warning: no timeout(1) found; a hung genote run will hang the suite" >&2
fi

require_python3() {
  command -v python3 >/dev/null 2>&1 || { echo "  skip: python3 required for mock server"; exit 2; }
}

mksandbox() {
  SANDBOX="$(mktemp -d)"
  echo "$SANDBOX" > "$RUNNER_TMP/sandbox"
  export HOME="$SANDBOX/home"
  export XDG_CONFIG_HOME="$SANDBOX/.config"
  export GENOTE_NO_ISSUE=1
  unset GENOTE_API
  mkdir -p "$HOME" "$XDG_CONFIG_HOME/Genote" "$SANDBOX/notes"
  cat > "$SANDBOX/notes/rust.md" <<'EOF'
# Rust basics
- owned values
- borrows with &
EOF
  cat > "$SANDBOX/notes/linux.md" <<'EOF'
# Linux basics
- processes
- files
EOF
}

mkconfig() { cat > "$XDG_CONFIG_HOME/Genote/config.toml"; }

base_config() {
  mkconfig <<EOF
model = "test-model"
api_url = "$MOCK_URL"
notes_dir = "$SANDBOX/notes"
lang = "ru"
note_size = "small"
notes_count = 3
EOF
  for line in "$@"; do
    echo "$line" >> "$XDG_CONFIG_HOME/Genote/config.toml"
  done
}

start_mock() {
  require_python3
  local args=()
  [ -n "${1:-}" ] && args=(--spec "$1")
  python3 "$SCRIPT_DIR/mock_server.py" --log "$SANDBOX/mock.log" "${args[@]}" \
    > "$SANDBOX/mock.port" 2> "$SANDBOX/mock.err" &
  echo "$!" > "$RUNNER_TMP/mockpid"
  for _ in $(seq 1 50); do
    port="$(cat "$SANDBOX/mock.port" 2>/dev/null)"
    [ -n "$port" ] && break
    sleep 0.1
  done
  if [ -z "${port:-}" ]; then
    echo "  FAIL: mock server did not start"
    cat "$SANDBOX/mock.err" 2>/dev/null
    exit 1
  fi
  export MOCK_URL="http://127.0.0.1:$port"
  export MOCK_LOG="$SANDBOX/mock.log"
}

genote_run() {
  local secs="$1"
  shift
  OUT="$SANDBOX/out"
  ERR="$SANDBOX/err"
  if [ -n "$TOOL_TIMEOUT" ]; then
    $TOOL_TIMEOUT "$secs" "$BIN" "$@" > "$OUT" 2> "$ERR"
  else
    "$BIN" "$@" > "$OUT" 2> "$ERR"
  fi
  CODE=$?
  return "$CODE"
}
run_genote() { genote_run 30 "$@"; }
run_genote_slow() { genote_run 300 "$@"; }

expect_fail() {
  "$@"
  local c=$?
  [ "$c" -ne 0 ] || { echo "  FAIL: expected non-zero exit, got 0"; exit 1; }
}

assert_eq() {
  [ "$1" = "$2" ] || { echo "  FAIL: expected '$1', got '$2'"; exit 1; }
}

assert_contains() {
  grep -qF -- "$2" "$1" || { echo "  FAIL: '$2' not found in $1"; exit 1; }
}

assert_not_contains() {
  if grep -qF -- "$2" "$1"; then
    echo "  FAIL: '$2' found in $1"
    exit 1
  fi
}

mock_count() { grep -cF -- "$1" "$MOCK_LOG"; }
