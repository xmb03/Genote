#!/usr/bin/env bash
# End-to-end cases against the mock LLM API (offline, hermetic).

test_ollama_single() {
  mksandbox
  start_mock
  base_config
  run_genote "Rust Ownership"
  assert_eq "$CODE" 0
  assert_contains "$SANDBOX/notes/Rust_Ownership.md" "# Test Note"
  assert_eq "$(mock_count '/api/generate')" 1
}

test_ollama_multi_topics() {
  mksandbox
  start_mock
  base_config
  run_genote "Topic One" "Topic Two"
  assert_eq "$CODE" 0
  assert_contains "$SANDBOX/notes/Topic_One.md" "# Test Note"
  assert_contains "$SANDBOX/notes/Topic_Two.md" "# Test Note"
  assert_eq "$(mock_count '/api/generate')" 2
}

test_weak_mode() {
  mksandbox
  start_mock
  base_config 'weak_mode = true'
  run_genote "Borrow Checker"
  assert_eq "$CODE" 0
  assert_contains "$SANDBOX/notes/Borrow_Checker.md" "# Test Note"
  assert_eq "$(mock_count '/api/chat')" 2
}

test_hint_topic() {
  mksandbox
  start_mock
  base_config
  run_genote "Closures (skip FnOnce)"
  assert_eq "$CODE" 0
  assert_contains "$SANDBOX/notes/Closures.md" "# Test Note"
  assert_contains "$MOCK_LOG" "skip FnOnce"
}

test_retry_on_500() {
  mksandbox
  cat > "$SANDBOX/spec.json" <<'EOF'
{"/api/generate": {"fail": 2}}
EOF
  start_mock "$SANDBOX/spec.json"
  base_config
  run_genote "Retry Topic"
  assert_eq "$CODE" 0
  assert_eq "$(mock_count '/api/generate')" 3
}

test_overwrite_warning() {
  mksandbox
  start_mock
  base_config
  run_genote "Same Topic"
  assert_eq "$CODE" 0
  run_genote "Same Topic"
  assert_eq "$CODE" 0
  assert_contains "$SANDBOX/err" "overwriting"
}

test_openai_provider() {
  mksandbox
  start_mock
  base_config 'provider = "openai"' 'api_key = "sk-test"'
  run_genote "OpenAI Topic"
  assert_eq "$CODE" 0
  assert_contains "$MOCK_LOG" '"/v1/chat/completions"'
  assert_contains "$MOCK_LOG" "Bearer sk-test"
}

test_llamacpp_provider() {
  mksandbox
  start_mock
  base_config 'provider = "llamacpp"'
  run_genote "Llama Topic"
  assert_eq "$CODE" 0
  assert_contains "$MOCK_LOG" '"/completion"'
  assert_not_contains "$MOCK_LOG" "Authorization"
}

test_anthropic_provider() {
  mksandbox
  start_mock
  base_config 'provider = "anthropic"' 'api_key = "sk-ant-123"'
  run_genote "Anthropic Topic"
  assert_eq "$CODE" 0
  assert_contains "$MOCK_LOG" '"/v1/messages"'
  assert_contains "$MOCK_LOG" "x-api-key"
  assert_contains "$MOCK_LOG" "sk-ant-123"
  assert_contains "$MOCK_LOG" "anthropic-version"
}

test_gemini_provider() {
  mksandbox
  start_mock
  base_config 'provider = "gemini"' 'api_key = "gk-abc"'
  run_genote "Gemini Topic"
  assert_eq "$CODE" 0
  assert_contains "$MOCK_LOG" ":generateContent"
  assert_contains "$MOCK_LOG" "x-goog-api-key"
  assert_not_contains "$MOCK_LOG" "key="
}

test_genote_api_env() {
  mksandbox
  start_mock
  base_config 'provider = "openai"' 'api_key = "cfg-key"'
  export GENOTE_API="env-key"
  run_genote "Env Key Topic"
  assert_eq "$CODE" 0
  assert_contains "$MOCK_LOG" "Bearer env-key"
  assert_not_contains "$MOCK_LOG" "Bearer cfg-key"
}

test_api_key_cli() {
  mksandbox
  start_mock
  base_config 'provider = "openai"'
  run_genote --api-key sk-cli "Cli Key Topic"
  assert_eq "$CODE" 0
  assert_contains "$MOCK_LOG" "Bearer sk-cli"
}

test_notes_include() {
  mksandbox
  start_mock
  base_config 'notes_include = ["rust.md"]'
  run_genote "Include Topic"
  assert_eq "$CODE" 0
  assert_contains "$MOCK_LOG" "# Rust basics"
  assert_not_contains "$MOCK_LOG" "# Linux basics"
}

test_notes_include_partial() {
  mksandbox
  start_mock
  base_config 'notes_include = ["rust.md", "missing.md"]'
  run_genote "Partial Include"
  assert_eq "$CODE" 0
  assert_contains "$SANDBOX/err" "skipped missing.md"
  assert_contains "$SANDBOX/notes/Partial_Include.md" "# Test Note"
}

test_use_covered_topics() {
  mksandbox
  start_mock
  base_config 'use_covered_topics = true'
  run_genote "Covered Topic"
  assert_eq "$CODE" 0
  assert_contains "$MOCK_LOG" "Already covered topics: linux, rust"
  assert_contains "$MOCK_LOG" "do NOT re-explain"
  assert_not_contains "$MOCK_LOG" "Restricted topics"
}

test_notes_include_covered_all() {
  mksandbox
  start_mock
  base_config 'notes_include = ["rust.md"]' 'use_covered_topics = true'
  run_genote "Covered Include"
  assert_eq "$CODE" 0
  assert_contains "$MOCK_LOG" "# Rust basics"
  assert_not_contains "$MOCK_LOG" "# Linux basics"
  assert_contains "$MOCK_LOG" "Already covered topics: linux, rust"
}

test_mid_line_warning() {
  mksandbox
  python3 - "$SANDBOX/spec.json" <<'PY'
import json, sys
json.dump({"/api/generate": {"text": "\n".join("line %d" % i for i in range(1, 41))}},
          open(sys.argv[1], "w"))
PY
  start_mock "$SANDBOX/spec.json"
  mkconfig <<EOF
model = "test-model"
api_url = "$MOCK_URL"
notes_dir = "$SANDBOX/notes"
lang = "ru"
note_size = "mid"
notes_count = 3
EOF
  run_genote "Mid Response"
  assert_eq "$CODE" 0
  assert_contains "$SANDBOX/err" "expected 45-68"
}

test_mid_ok_no_warning() {
  mksandbox
  python3 - "$SANDBOX/spec.json" <<'PY'
import json, sys
json.dump({"/api/generate": {"text": "\n".join("line %d" % i for i in range(1, 51))}},
          open(sys.argv[1], "w"))
PY
  start_mock "$SANDBOX/spec.json"
  mkconfig <<EOF
model = "test-model"
api_url = "$MOCK_URL"
notes_dir = "$SANDBOX/notes"
lang = "ru"
note_size = "mid"
notes_count = 3
EOF
  run_genote "Mid Good"
  assert_eq "$CODE" 0
  assert_not_contains "$SANDBOX/err" "expected 45-68"
}

test_cloud_async_multi_topics() {
  mksandbox
  start_mock
  base_config 'provider = "openai"' 'api_key = "sk-test"'
  run_genote "Async Topic One" "Async Topic Two"
  assert_eq "$CODE" 0
  assert_contains "$SANDBOX/notes/Async_Topic_One.md" "# Test Note"
  assert_contains "$SANDBOX/notes/Async_Topic_Two.md" "# Test Note"
  assert_eq "$(mock_count '/v1/chat/completions')" 2
}

test_skill_weak_mode_both_stages() {
  mksandbox
  start_mock
  echo "SKILL RULE 999" > "$SANDBOX/skill.md"
  base_config 'weak_mode = true'
  run_genote --skill "$SANDBOX/skill.md" "Skill Weak"
  assert_eq "$CODE" 0
  assert_eq "$(mock_count '/api/chat')" 2
  assert_eq "$(grep -o 'SKILL RULE 999' "$MOCK_LOG" | wc -l)" 3
}

test_skill_system_and_user_normal() {
  mksandbox
  start_mock
  echo "SKILL RULE 777" > "$SANDBOX/skill.md"
  base_config 'provider = "openai"' 'api_key = "sk-x"'
  run_genote --skill "$SANDBOX/skill.md" "Skill Both"
  assert_eq "$CODE" 0
  assert_eq "$(grep -o 'SKILL RULE 777' "$MOCK_LOG" | wc -l)" 2
  assert_contains "$MOCK_LOG" "STYLE SKILL (STRICT"
}

test_small_line_warning() {
  mksandbox
  python3 - "$SANDBOX/spec.json" <<'PY'
import json, sys
json.dump({"/api/generate": {"text": "\n".join("line %d" % i for i in range(1, 41))}},
          open(sys.argv[1], "w"))
PY
  start_mock "$SANDBOX/spec.json"
  base_config
  run_genote "Big Response"
  assert_eq "$CODE" 0
  assert_contains "$SANDBOX/err" "expected 25-30"
}

test_log_full() {
  mksandbox
  start_mock
  base_config 'log = true'
  run_genote "Logged Topic"
  assert_eq "$CODE" 0
  assert_contains "$SANDBOX/out" "[PROMPT]"
  assert_contains "$SANDBOX/out" "[RESPONSE]"
}

test_skill_file() {
  mksandbox
  start_mock
  echo "SKILL RULE 123" > "$SANDBOX/skill.md"
  base_config 'provider = "openai"' 'api_key = "sk-x"'
  run_genote --skill "$SANDBOX/skill.md" "Skill Topic"
  assert_eq "$CODE" 0
  assert_contains "$MOCK_LOG" "SKILL RULE 123"
}

test_profile_default() {
  mksandbox
  start_mock
  mkdir -p "$SANDBOX/work-notes"
  echo "# work seed" > "$SANDBOX/work-notes/seed.md"
  mkconfig <<EOF
default = "work"
model = "test-model"
api_url = "$MOCK_URL"
notes_dir = "$SANDBOX/notes"
lang = "ru"
note_size = "small"
notes_count = 1

[profile.work]
notes_dir = "$SANDBOX/work-notes"
EOF
  run_genote "Profile Test"
  assert_eq "$CODE" 0
  assert_contains "$SANDBOX/work-notes/Profile_Test.md" "# Test Note"
}

test_profile_cli_flag() {
  mksandbox
  start_mock
  mkdir -p "$SANDBOX/work-notes"
  echo "# work seed" > "$SANDBOX/work-notes/seed.md"
  mkconfig <<EOF
model = "test-model"
api_url = "$MOCK_URL"
notes_dir = "$SANDBOX/notes"
lang = "ru"
note_size = "small"
notes_count = 1

[profile.work]
notes_dir = "$SANDBOX/work-notes"
EOF
  run_genote --profile work "Profile Test"
  assert_eq "$CODE" 0
  assert_contains "$SANDBOX/work-notes/Profile_Test.md" "# Test Note"
}

test_home_expansion() {
  mksandbox
  start_mock
  mkdir -p "$HOME/nested"
  echo "# seed" > "$HOME/nested/seed.md"
  mkconfig <<EOF
model = "test-model"
api_url = "$MOCK_URL"
notes_dir = "~/nested"
lang = "ru"
note_size = "small"
notes_count = 1
EOF
  run_genote "Home Topic"
  assert_eq "$CODE" 0
  assert_contains "$HOME/nested/Home_Topic.md" "# Test Note"
}

test_xdg_default_location() {
  mksandbox
  unset XDG_CONFIG_HOME
  mkdir -p "$HOME/.config/Genote"
  start_mock
  cat > "$HOME/.config/Genote/config.toml" <<EOF
model = "test-model"
api_url = "$MOCK_URL"
notes_dir = "$SANDBOX/notes"
lang = "ru"
note_size = "small"
notes_count = 3
EOF
  run_genote "Default Location"
  assert_eq "$CODE" 0
  assert_contains "$SANDBOX/notes/Default_Location.md" "# Test Note"
}
