#!/usr/bin/env bash
set -u

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(dirname "$SCRIPT_DIR")"
RUNNER_TMP="$(mktemp -d)"
export RUNNER_TMP

usage() {
  cat <<EOF
Usage: ./test/run.sh [GROUP...] [OPTIONS]

Run Genote test cases. New case = a test_* function in test/cases/*.sh
(placed in the file matching its group); it runs automatically.

Groups: build config e2e live
Options:
  -f, --filter NAME   run only cases containing NAME
      --no-build      skip cargo build/test cases
      --release       build in release mode (default: debug)
      --live          force the live ollama smoke test
      --keep          keep sandbox dirs of failed cases
  -v, --verbose       print stdout/stderr of failed cases
  -h, --help          this help

Full per-case logs (stdout, stderr, mock requests, config, notes) are saved
to test/logs/<timestamp>/<case>/.
EOF
  exit 0
}

. "$SCRIPT_DIR/helpers.sh"

BUILD_FLAG=""; BUILD_MODE="debug"; NO_BUILD=""; LIVE=""; KEEP=""; VERBOSE=""; FILTER=""; GROUPSEL=()
while [ $# -gt 0 ]; do
  case "$1" in
    -f|--filter) FILTER="${2:-}"; shift 2;;
    --no-build) NO_BUILD=1; shift;;
    --release) BUILD_FLAG="--release"; BUILD_MODE="release"; shift;;
    --live) LIVE=1; shift;;
    --keep) KEEP=1; shift;;
    -v|--verbose) VERBOSE=1; shift;;
    -h|--help) usage;;
    -*) echo "unknown option: $1" >&2; exit 2;;
    *) GROUPSEL+=("$1"); shift;;
  esac
done
export LIVE NO_BUILD VERBOSE

BIN="$ROOT_DIR/target/$BUILD_MODE/genote"
if [ ! -x "$BIN" ] && [ -z "$NO_BUILD" ]; then
  echo "binary not built, running cargo build ..."
  cargo build $BUILD_FLAG --quiet
fi

for f in "$SCRIPT_DIR"/cases/*.sh; do . "$f"; done
CASES=($(declare -F | awk '$3 ~ /^test_/ {print $3}' | sort))

PASS=0; FAILN=0; SKIPN=0
LOG_DIR="$SCRIPT_DIR/logs/$(date +%Y%m%d-%H%M%S)"
mkdir -p "$LOG_DIR"

save_logs() {
  local sb="$1" name="$2"
  [ -n "$sb" ] || return 0
  local d="$LOG_DIR/$name"
  mkdir -p "$d"
  for f in out err mock.log mock.err; do
    [ -f "$sb/$f" ] && cp "$sb/$f" "$d/"
  done
  [ -f "$sb/.config/Genote/config.toml" ] && cp "$sb/.config/Genote/config.toml" "$d/config.toml"
  [ -d "$sb/notes" ] && cp -r "$sb/notes" "$d/notes"
}

cleanup_case() {
  local pid="$(cat "$RUNNER_TMP/mockpid" 2>/dev/null)"
  [ -n "$pid" ] && kill "$pid" 2>/dev/null
  rm -f "$RUNNER_TMP/mockpid"
  local sb="$(cat "$RUNNER_TMP/sandbox" 2>/dev/null)"
  [ -n "$sb" ] && [ -z "$KEEP" ] && rm -rf "$sb"
  rm -f "$RUNNER_TMP/sandbox"
}
trap 'cleanup_case; rm -rf "$RUNNER_TMP"' EXIT

for name in "${CASES[@]}"; do
  if [ "${#GROUPSEL[@]}" -gt 0 ]; then
    g="$(grep -l "^${name}()" "$SCRIPT_DIR"/cases/*.sh 2>/dev/null | xargs -n1 basename 2>/dev/null | sed 's/\.sh$//' | head -1)"
    m=0
    for grp in "${GROUPSEL[@]}"; do [ "$grp" = "$g" ] && m=1; done
    [ "$m" -eq 1 ] || continue
  fi
  [ -z "$FILTER" ] || case "$name" in *"$FILTER"*) ;; *) continue;; esac
  printf '%s ... ' "$name"
  ( "$name" )
  code=$?
  sb="$(cat "$RUNNER_TMP/sandbox" 2>/dev/null)"
  save_logs "$sb" "$name"
  case "$code" in
    0) echo "PASS"; PASS=$((PASS+1));;
    2) echo "SKIP"; SKIPN=$((SKIPN+1));;
    *)
      echo "${C_RED}FAIL${C_RST} (rerun: ./test/run.sh -f $name)"
      [ -n "$sb" ] && echo "  sandbox: $sb"
      if [ -n "$VERBOSE" ] && [ -n "$sb" ]; then
        for f in out err; do
          [ -f "$sb/$f" ] && { echo "  --- $f ---"; sed -n '1,30p' "$sb/$f"; }
        done
      fi
      FAILN=$((FAILN+1))
      ;;
  esac
  cleanup_case
done

echo
echo "$PASS passed, $FAILN failed, $SKIPN skipped"
echo "Logs: $LOG_DIR"
[ "$FAILN" -eq 0 ]
