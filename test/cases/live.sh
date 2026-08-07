#!/usr/bin/env bash
# Optional live smoke test against a real ollama. SKIP unless reachable or --live.

test_live_ollama() {
  require_python3
  mksandbox
  if curl -sf -m 3 http://127.0.0.1:11434/api/tags > "$SANDBOX/tags.json" 2>/dev/null; then
    :
  elif [ -n "${LIVE:-}" ]; then
    echo "  skip: ollama unreachable at 127.0.0.1:11434"
    exit 2
  else
    echo "  skip: no local ollama (pass --live to force)"
    exit 2
  fi
  model="$(python3 -c "import json; print(json.load(open('$SANDBOX/tags.json'))['models'][0]['name'])" 2>/dev/null)"
  [ -n "$model" ] || { echo "  FAIL: no models served by ollama"; exit 1; }
  mkdir -p "$SANDBOX/live-notes"
  echo "# seed" > "$SANDBOX/live-notes/seed.md"
  mkconfig <<EOF
model = "$model"
api_url = "http://127.0.0.1:11434"
provider = "ollama"
notes_dir = "$SANDBOX/live-notes"
lang = "en"
note_size = "small"
notes_count = 1
EOF
  run_genote_slow "Smoke Test Topic"
  assert_eq "$CODE" 0
  [ -s "$SANDBOX/live-notes/Smoke_Test_Topic.md" ] || { echo "  FAIL: note missing or empty"; exit 1; }
}
