#!/usr/bin/env bash
# Config/CLI validation cases. No network, no mock server.

test_missing_config() {
  mksandbox
  expect_fail run_genote "any topic"
  assert_contains "$SANDBOX/err" "config.toml not found"
}

test_bad_toml() {
  mksandbox
  mkconfig <<'EOF'
model = "x"
[[[ broken
EOF
  expect_fail run_genote "any topic"
  assert_contains "$SANDBOX/err" "config parse error"
}

test_unknown_key_warning() {
  mksandbox
  base_config 'unknown_setting = 1'
  expect_fail run_genote "any topic"
  assert_contains "$SANDBOX/err" "unknown config key"
}

test_missing_model() {
  mksandbox
  mkconfig <<EOF
api_url = "http://127.0.0.1:1"
notes_dir = "$SANDBOX/notes"
lang = "ru"
note_size = "small"
EOF
  expect_fail run_genote "any topic"
  assert_contains "$SANDBOX/err" "model is not set"
}

test_bad_note_size() {
  mksandbox
  mkconfig <<EOF
model = "m"
api_url = "http://127.0.0.1:1"
notes_dir = "$SANDBOX/notes"
lang = "ru"
note_size = "huge"
EOF
  expect_fail run_genote "any topic"
  assert_contains "$SANDBOX/err" "note_size must be"
}

test_unknown_provider() {
  mksandbox
  mkconfig <<EOF
model = "m"
api_url = "http://127.0.0.1:1"
provider = "wat"
notes_dir = "$SANDBOX/notes"
lang = "ru"
note_size = "small"
EOF
  expect_fail run_genote "any topic"
  assert_contains "$SANDBOX/err" "unknown provider"
}

test_missing_notes_dir() {
  mksandbox
  mkconfig <<EOF
model = "m"
api_url = "http://127.0.0.1:1"
notes_dir = "$SANDBOX/nope"
lang = "ru"
note_size = "small"
EOF
  expect_fail run_genote "any topic"
  assert_contains "$SANDBOX/err" "Notes directory does not exist"
}

test_notes_count_zero() {
  mksandbox
  mkconfig <<EOF
model = "m"
api_url = "http://127.0.0.1:1"
notes_dir = "$SANDBOX/notes"
lang = "ru"
note_size = "small"
notes_count = 0
EOF
  expect_fail run_genote "any topic"
  assert_contains "$SANDBOX/err" "notes_count is set to 0"
}

test_empty_notes_dir() {
  mksandbox
  mkdir -p "$SANDBOX/empty"
  mkconfig <<EOF
model = "m"
api_url = "http://127.0.0.1:1"
notes_dir = "$SANDBOX/empty"
lang = "ru"
note_size = "small"
EOF
  expect_fail run_genote "any topic"
  assert_contains "$SANDBOX/err" "No .md files found"
}

test_notes_include_all_missing() {
  mksandbox
  mkconfig <<EOF
model = "m"
api_url = "http://127.0.0.1:1"
notes_dir = "$SANDBOX/notes"
lang = "ru"
note_size = "small"
notes_count = 3
notes_include = ["a.md", "b.md"]
EOF
  expect_fail run_genote "any topic"
  assert_contains "$SANDBOX/err" "None of the files listed in notes_include were found"
}

test_profile_flag_requires_profiles() {
  mksandbox
  mkconfig <<EOF
model = "m"
api_url = "http://127.0.0.1:1"
notes_dir = "$SANDBOX/notes"
lang = "ru"
note_size = "small"
EOF
  expect_fail run_genote --profile work "any topic"
  assert_contains "$SANDBOX/err" "--profile flag requires"
}

test_profile_not_found() {
  mksandbox
  mkconfig <<EOF
[profile.work]
model = "m"
EOF
  expect_fail run_genote --profile nope "any topic"
  assert_contains "$SANDBOX/err" "profile 'nope' not found"
}

test_no_default_profile_warning() {
  mksandbox
  mkconfig <<EOF
model = "m"
api_url = "http://127.0.0.1:1"
notes_dir = "$SANDBOX/notes"
lang = "ru"
note_size = "small"

[profile.work]
notes_dir = "$SANDBOX/work-notes"
EOF
  expect_fail run_genote "any topic"
  assert_contains "$SANDBOX/err" "no default profile set"
}

test_no_issue_hang() {
  mksandbox
  unset GENOTE_NO_ISSUE
  run_genote "any topic" < /dev/null
  assert_eq "$CODE" 1
  assert_contains "$SANDBOX/err" "config.toml not found"
}
