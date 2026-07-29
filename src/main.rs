use clap::Parser;
use keyring::Entry;
use reqwest::Client;
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use serde::Deserialize;
use serde_json::json;
use std::collections::HashMap;
use std::env;
use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::time::Instant;

mod provider;
use provider::Provider;

const KEYRING_SERVICE: &str = "genote";
const KEYRING_USER: &str = "api_key";

#[derive(Deserialize, Clone, Default)]
struct ConfigValues {
    model: Option<String>,
    api_url: Option<String>,
    provider: Option<String>,
    notes_dir: Option<String>,
    lang: Option<String>,
    note_size: Option<String>,
    notes_count: Option<u32>,
    use_covered_topics: Option<bool>,
    weak_mode: Option<bool>,
    log: Option<LogRaw>,
}

#[derive(Deserialize, Clone)]
#[serde(untagged)]
enum LogRaw {
    Bool(bool),
    Table(LogTable),
}

#[derive(Deserialize, Clone)]
struct LogTable {
    #[serde(default = "default_true")]
    prompt: bool,
    #[serde(default = "default_true")]
    response: bool,
    #[serde(default = "default_true")]
    status: bool,
    #[serde(default = "default_true")]
    timing: bool,
}

fn default_true() -> bool { true }

#[derive(Clone)]
struct LogConfig {
    prompt: bool,
    response: bool,
    status: bool,
    timing: bool,
}

impl LogConfig {
    fn resolve(raw: Option<LogRaw>) -> Self {
        match raw {
            None => LogConfig {
                prompt: false,
                response: false,
                status: true,
                timing: true,
            },
            Some(LogRaw::Bool(true)) => LogConfig {
                prompt: true,
                response: true,
                status: true,
                timing: true,
            },
            Some(LogRaw::Bool(false)) => LogConfig {
                prompt: false,
                response: false,
                status: false,
                timing: false,
            },
            Some(LogRaw::Table(t)) => LogConfig {
                prompt: t.prompt,
                response: t.response,
                status: t.status,
                timing: t.timing,
            },
        }
    }
}

#[derive(Deserialize)]
struct ConfigFile {
    default: Option<String>,
    profile: Option<HashMap<String, ConfigValues>>,
    #[serde(flatten)]
    values: ConfigValues,
}

#[derive(Parser)]
#[command(about = "Generate IT study notes using local LLMs")]
struct Args {
    #[arg(required = true)]
    topics: Vec<String>,

    #[arg(short = 'm', long = "model")]
    model: Option<String>,

    #[arg(long = "api-url")]
    api_url: Option<String>,

    #[arg(short = 'p', long = "provider")]
    provider: Option<String>,

    #[arg(short = 'd', long = "notes-dir")]
    notes_dir: Option<String>,

    #[arg(short = 'l', long = "lang")]
    lang: Option<String>,

    #[arg(short = 's', long = "note-size")]
    note_size: Option<String>,

    #[arg(short = 'n', long = "notes-count")]
    notes_count: Option<u32>,

    #[arg(long = "use-covered-topics")]
    use_covered_topics: Option<bool>,

    #[arg(long = "weak-mode", action = clap::ArgAction::Set)]
    weak_mode: Option<bool>,

    #[arg(long = "profile")]
    profile: Option<String>,

    #[arg(long = "api-key", num_args = 0..=1, default_missing_value = "reset", help = "Set API key (saves to system keyring). Pass without value to reset interactively.")]
    api_key: Option<String>,
}

fn expand_home(path: &str) -> PathBuf {
    let home_key = if cfg!(target_os = "windows") { "USERPROFILE" } else { "HOME" };
    if path == "~" {
        return PathBuf::from(env::var(home_key).unwrap_or_else(|_| ".".to_string()));
    }
    if path.starts_with("~/") {
        if let Ok(home) = env::var(home_key) {
            return PathBuf::from(home).join(&path[2..]);
        }
    }
    PathBuf::from(path)
}

fn req<T>(v: Option<T>, name: &str) -> T {
    v.unwrap_or_else(|| {
        eprintln!("Error: {name} is not set in config.toml or via CLI");
        issue_prompt(&format!("Missing required config: {name}"));
        std::process::exit(1);
    })
}

fn issue_prompt(ctx: &str) {
    if env::var("GENOTE_NO_ISSUE").is_ok() {
        return;
    }
    eprintln!();
    eprint!("  Open a GitHub issue for this error? [y/N] ");
    let _ = std::io::stderr().flush();
    let mut input = String::new();
    if std::io::stdin().read_line(&mut input).is_err() {
        return;
    }
    if !input.trim().eq_ignore_ascii_case("y") {
        return;
    }
    let title = urlencoding::encode(ctx).to_string();
    let body_text = format!(
        "## Error\n\n{}\n\n## Environment\n\n- OS: {}\n",
        ctx,
        std::env::consts::OS
    );
    let body = urlencoding::encode(&body_text).to_string();
    let url = format!(
        "https://github.com/xmb03/Genote/issues/new?title={}&body={}",
        title, body
    );
    let result = if cfg!(target_os = "windows") {
        std::process::Command::new("cmd").args(["/c", "start", &url]).spawn()
    } else if cfg!(target_os = "macos") {
        std::process::Command::new("open").arg(&url).spawn()
    } else {
        std::process::Command::new("xdg-open").arg(&url).spawn()
    };
    match result {
        Ok(_) => eprintln!("  Browser opened."),
        Err(_) => eprintln!("  Could not open browser. URL:\n  {}", url),
    }
}

fn get_keyring_entry() -> Entry {
    Entry::new(KEYRING_SERVICE, KEYRING_USER).unwrap_or_else(|e| {
        eprintln!("Error: failed to access system keyring: {}", e);
        std::process::exit(1);
    })
}

fn get_api_key_from_keyring() -> Option<String> {
    get_keyring_entry().get_password().ok()
}

fn set_api_key_in_keyring(key: &str) {
    if let Err(e) = get_keyring_entry().set_password(key) {
        eprintln!("Warning: failed to save API key to system keyring: {}", e);
    }
}

fn prompt_api_key() -> String {
    let key = rpassword::prompt_password("Enter API key: ").unwrap_or_else(|e| {
        eprintln!("Error: failed to read API key: {}", e);
        std::process::exit(1);
    });
    let trimmed = key.trim().to_string();
    if trimmed.is_empty() {
        eprintln!("Error: API key cannot be empty.");
        std::process::exit(1);
    }
    trimmed
}

struct Logger {
    config: LogConfig,
}

impl Logger {
    fn new(config: LogConfig) -> Self {
        Logger { config }
    }

    fn prompt(&self, msg: &str) {
        if self.config.prompt {
            println!("[PROMPT]\n{}\n[/PROMPT]", msg);
        }
    }

    fn response(&self, msg: &str) {
        if self.config.response {
            println!("[RESPONSE]\n{}\n[/RESPONSE]", msg);
        }
    }

    fn status(&self, msg: &str) {
        if self.config.status {
            println!("{}", msg);
        }
    }

    fn timing(&self, msg: &str) {
        if self.config.timing {
            println!("{}", msg);
        }
    }
}

fn build_headers(provider: &Provider, api_key: &str) -> HeaderMap {
    let mut map = HeaderMap::new();
    for (name, value) in provider.headers(api_key) {
        if let (Ok(n), Ok(v)) = (
            HeaderName::from_bytes(name.as_bytes()),
            HeaderValue::from_str(&value),
        ) {
            map.insert(n, v);
        }
    }
    map
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    let config_path = env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.join("config.toml")))
        .filter(|p| p.exists())
        .or_else(|| {
            let config_dir = if cfg!(target_os = "windows") {
                env::var("APPDATA").unwrap_or_default()
            } else if cfg!(target_os = "macos") {
                env::var("HOME")
                    .map(|h| format!("{}/Library/Application Support", h))
                    .unwrap_or_default()
            } else {
                env::var("XDG_CONFIG_HOME")
                    .unwrap_or_else(|_| {
                        env::var("HOME")
                            .map(|h| format!("{}/.config", h))
                            .unwrap_or_default()
                    })
            };
            let p = PathBuf::from(config_dir).join("Genote").join("config.toml");
            p.exists().then_some(p)
        })
        .unwrap_or_else(|| PathBuf::from("config.toml"));

    let config_content = fs::read_to_string(&config_path).unwrap_or_else(|_| {
        eprintln!("Error: config.toml not found next to the binary.");
        eprintln!("Place it at: {:?}", config_path);
        eprintln!("Or create it based on config.toml.example");
        issue_prompt("Config file not found");
        std::process::exit(1);
    });

    let config_file: ConfigFile = toml::from_str(&config_content).unwrap_or_else(|e| {
        eprintln!("Error parsing config.toml: {}", e);
        issue_prompt(&format!("Config parse error: {}", e));
        std::process::exit(1);
    });

    let has_profiles = config_file.profile.as_ref().map(|p| !p.is_empty()).unwrap_or(false);
    let cli_profile = args.profile.clone();

    let profile_vals = if has_profiles {
        let name = cli_profile.or(config_file.default.clone()).unwrap_or_else(|| {
            eprintln!("Error: --profile not specified and no default profile set in config.toml");
            issue_prompt("Missing --profile and no default in config");
            std::process::exit(1);
        });
        config_file.profile.as_ref()
            .and_then(|p| p.get(&name))
            .cloned()
            .unwrap_or_else(|| {
                eprintln!("Error: profile '{}' not found in config.toml", name);
                issue_prompt(&format!("Profile '{}' not found in config", name));
                std::process::exit(1)
            })
    } else {
        if cli_profile.is_some() {
            eprintln!("Error: --profile flag requires [profile] sections in config.toml");
            issue_prompt("--profile flag used without [profile] sections");
            std::process::exit(1);
        }
        ConfigValues::default()
    };

    let model = req(
        args.model.clone()
            .or_else(|| profile_vals.model.clone())
            .or_else(|| config_file.values.model.clone()),
        "model",
    );
    let api_url = req(
        args.api_url.clone()
            .or_else(|| profile_vals.api_url.clone())
            .or_else(|| config_file.values.api_url.clone()),
        "api_url",
    );
    let provider_name = args.provider.clone()
        .or_else(|| profile_vals.provider.clone())
        .or_else(|| config_file.values.provider.clone())
        .unwrap_or_else(|| "ollama".to_string());
    let provider = Provider::from_str(&provider_name);
    let api_key = match args.api_key.as_deref() {
        Some("reset") => {
            let key = prompt_api_key();
            set_api_key_in_keyring(&key);
            key
        }
        Some(k) => {
            set_api_key_in_keyring(k);
            k.to_string()
        }
        None => {
            get_api_key_from_keyring().unwrap_or_else(|| {
                let key = prompt_api_key();
                set_api_key_in_keyring(&key);
                key
            })
        }
    };
    let notes_dir = req(
        args.notes_dir.clone()
            .or_else(|| profile_vals.notes_dir.clone())
            .or_else(|| config_file.values.notes_dir.clone()),
        "notes_dir",
    );
    let lang = req(
        args.lang.clone()
            .or_else(|| profile_vals.lang.clone())
            .or_else(|| config_file.values.lang.clone()),
        "lang",
    );
    let note_size = req(
        args.note_size.clone()
            .or_else(|| profile_vals.note_size.clone())
            .or_else(|| config_file.values.note_size.clone()),
        "note_size",
    );
    if note_size != "small" && note_size != "big" {
        eprintln!(
            "Error: note_size must be either \"small\" or \"big\", got \"{}\"",
            note_size
        );
        issue_prompt(&format!("Invalid note_size: {}", note_size));
        std::process::exit(1);
    }
    let notes_count = args.notes_count
        .or(profile_vals.notes_count)
        .or(config_file.values.notes_count)
        .unwrap_or(7);
    if notes_count == 0 {
        eprintln!("Error: notes_count is set to 0, nothing to load.");
        std::process::exit(1);
    }
    let use_covered = args.use_covered_topics
        .or(profile_vals.use_covered_topics)
        .or(config_file.values.use_covered_topics)
        .unwrap_or(false);
    let weak_mode = args.weak_mode
        .or(profile_vals.weak_mode)
        .or(config_file.values.weak_mode)
        .unwrap_or(false);

    let notes_path = expand_home(&notes_dir);
    if !notes_path.exists() {
        eprintln!("Error: Notes directory does not exist: {:?}", notes_path);
        issue_prompt(&format!("Notes directory does not exist: {:?}", notes_path));
        std::process::exit(1);
    }

    let log_config = LogConfig::resolve(
        profile_vals.log.clone()
            .or_else(|| config_file.values.log.clone())
    );
    let logger = Logger::new(log_config);

    let mut examples = String::new();
    let mut count = 0u32;
    let mut covered_topics: Vec<String> = Vec::new();

    if let Ok(entries) = fs::read_dir(&notes_path) {
        for entry in entries.flatten() {
            let p = entry.path();
            if !p.extension().and_then(|s| s.to_str()).map(|s| s.eq_ignore_ascii_case("md")).unwrap_or(false) {
                continue;
            }
            if use_covered {
                if let Some(stem) = p.file_stem().and_then(|s| s.to_str()) {
                    covered_topics.push(stem.replace('_', " "));
                }
            }
            if count < notes_count {
                if let Ok(content) = fs::read_to_string(&p) {
                    examples.push_str(&format!(
                        "--- Example {} ---\n{}\n\n",
                        count + 1,
                        content
                    ));
                    count += 1;
                }
            }
        }
    }

    if count == 0 {
        eprintln!("Error: No .md files found in the notes directory.");
        issue_prompt("No .md files found in notes directory");
        std::process::exit(1);
    }

    let client = Client::new();

    for topic in &args.topics {
        let (clean_topic, user_hint) = {
            let mut depth = 0i32;
            let mut start = None;
            let mut end = 0;
            for (i, ch) in topic.char_indices() {
                match ch {
                    '(' if depth == 0 => { start = Some(i); depth = 1; }
                    '(' => depth += 1,
                    ')' if depth > 0 => { depth -= 1; if depth == 0 { end = i; break; } }
                    _ => {}
                }
            }
            start.filter(|_| depth == 0).map(|o| {
                (topic[..o].trim().to_string(), Some(topic[o + 1..end].trim().to_string()))
            }).unwrap_or((topic.clone(), None))
        };

        let hint_instruction = user_hint
            .as_ref()
            .map(|h| format!("- Additional instruction: {}\n", h))
            .unwrap_or_default();

        let covered_instruction = if use_covered && !covered_topics.is_empty() {
            format!(
                "- Restricted topics: {}. \
                 Only use concepts from EXACTLY these topics. \
                 Do NOT introduce anything outside this list.\n",
                covered_topics.join(", ")
            )
        } else {
            String::new()
        };

        logger.status(&format!(
            "[{}/{}] Sending request (Model: {}, Topic: \"{}\", style examples: {}){}...",
            args.topics.iter().position(|t| t == topic).unwrap_or(0) + 1,
            args.topics.len(),
            model,
            clean_topic,
            count,
            if weak_mode { " [weak-mode: 2 stages]" } else { "" },
        ));

        let start = Instant::now();

        let (generated_text, eval_count) = if weak_mode {
            // ----- Stage 1: style analysis -----
            let analysis_system = "\
You are a strict style-analysis engine. You have ONE task: extract structural/style facts from the notes below.

Rules:
- Output ONLY a bullet list of style facts. No greetings, no confirmations, no \"I understand\", no \"Here is my analysis\".
- Do NOT rewrite or summarize the content of the notes.
- Do NOT add any text before or after the analysis.
- Be precise and concise.

Extract these aspects:
- Heading style (which levels are used, capitalization, spacing)
- List style (bullets vs numbers, nesting depth)
- Code block style (inline `code` vs fenced blocks, with/without language tags)
- Typical section structure (e.g. always starts with a definition, then examples)
- Line density (short one-idea-per-line vs paragraph-style)
- Tone (formal, tutorial-like, concise, etc.)
- Any recurring formatting patterns

FAILURE TO FOLLOW THESE RULES WILL BE PENALIZED.";

            let hdrs = build_headers(&provider, &api_key);
            let res1 = match client
                .post(&provider.chat_url(&api_url, &model, &api_key))
                .headers(hdrs)
                .json(&provider.build_chat_body(&model, &json!([
                    {"role": "system", "content": analysis_system},
                    {"role": "user", "content": &examples}
                ])))
                .send()
                .await
            {
                Ok(r) => r,
                Err(e) => {
                    eprintln!("Error: Stage 1 request failed for \"{}\": {}", clean_topic, e);
                    issue_prompt(&format!("[{}] Stage 1 request failed: {}", clean_topic, e));
                    continue;
                }
            };

            if !res1.status().is_success() {
                let status = res1.status();
                let body = res1.text().await.unwrap_or_default();
                eprintln!("Error: API returned non-success status for \"{}\" (stage 1): {}. Body: {}", clean_topic, status, body);
                issue_prompt(&format!("[{}] Stage 1 non-success status: {}", clean_topic, status));
                continue;
            }

            let res1_json: serde_json::Value = match res1.json().await {
                Ok(v) => v,
                Err(e) => {
                    eprintln!("Error: Failed to parse stage 1 response JSON for \"{}\": {}", clean_topic, e);
                    issue_prompt(&format!("[{}] Stage 1 JSON parse error: {}", clean_topic, e));
                    continue;
                }
            };
            let analysis = provider.parse_chat_response(&res1_json).0;

            if analysis.is_empty() {
                eprintln!("Error: Stage 1 returned empty analysis for \"{}\".", clean_topic);
                issue_prompt(&format!("[{}] Stage 1 returned empty analysis", clean_topic));
                continue;
            }

            // ----- Stage 2: generation -----
            let size_instruction = if note_size == "small" {
                "SMALL — HARD LIMIT: EXACTLY 25-30 LINES.\nLINE BUDGET: 25-30 lines total. One idea per line, compact. Count lines as you write. Stop at 30 even if unfinished. Key points only, no fluff."
            } else {
                "BIG — comprehensive and detailed. Full coverage of the topic."
            };

            let size_reminder = if note_size == "small" {
                "REMINDER: 25-30 LINES ONLY. VERIFY COUNT BEFORE OUTPUT."
            } else {
                ""
            };

            let stage2_prompt = format!(
                "Now use the style patterns you identified above.\n\n\
                 Write a study note about \"{}\".\n\n\
                 STRICT RULES — YOU MUST FOLLOW EVERY ONE:\n\
                 - Language: {}. Write ONLY in this language. Zero words in any other language.\n\
                 - Size: {}\n\
                 {}\
                 {}\
                 - Use EXACTLY the heading style, list style, code style, section structure, and tone from your analysis above.\n\
                 - Do NOT add introductions like \"Here is your note\" or \"This note covers\".\n\
                 - Do NOT add conclusions like \"In summary\" or \"I hope this helps\".\n\
                 - OUTPUT ONLY THE NOTE. Nothing before. Nothing after.\n\n\
                 {}\n\n\
                 OUTPUT:",
                clean_topic,
                lang,
                size_instruction,
                hint_instruction,
                covered_instruction,
                size_reminder,
            );

            logger.prompt(&format!(
                "--- Stage 2 prompt for \"{}\" ---\n{}",
                clean_topic, stage2_prompt
            ));

            let hdrs = build_headers(&provider, &api_key);
            let res2 = match client
                .post(&provider.chat_url(&api_url, &model, &api_key))
                .headers(hdrs)
                .json(&provider.build_chat_body(&model, &json!([
                    {"role": "system", "content": "You are a helpful assistant that writes study notes."},
                    {"role": "user", "content": &examples},
                    {"role": "assistant", "content": &analysis},
                    {"role": "user", "content": &stage2_prompt}
                ])))
                .send()
                .await
            {
                Ok(r) => r,
                Err(e) => {
                    eprintln!("Error: Stage 2 request failed for \"{}\": {}", clean_topic, e);
                    issue_prompt(&format!("[{}] Stage 2 request failed: {}", clean_topic, e));
                    continue;
                }
            };

            if !res2.status().is_success() {
                let status = res2.status();
                let body = res2.text().await.unwrap_or_default();
                eprintln!("Error: API returned non-success status for \"{}\" (stage 2): {}. Body: {}", clean_topic, status, body);
                issue_prompt(&format!("[{}] Stage 2 non-success status: {}", clean_topic, status));
                continue;
            }

            let res2_json: serde_json::Value = match res2.json().await {
                Ok(v) => v,
                Err(e) => {
                    eprintln!("Error: Failed to parse stage 2 response JSON for \"{}\": {}", clean_topic, e);
                    issue_prompt(&format!("[{}] Stage 2 JSON parse error: {}", clean_topic, e));
                    continue;
                }
            };
            let (text, eval_count_str) = provider.parse_chat_response(&res2_json);

            if text.is_empty() {
                eprintln!("Error: Stage 2 returned empty response for \"{}\".", clean_topic);
                (String::new(), "N/A".to_string())
            } else {
                logger.response(&format!(
                    "--- Response for \"{}\" ---\n{}",
                    clean_topic, text
                ));
                (text, eval_count_str)
            }
        } else {
            // ----- Normal mode: single /api/generate request -----
            let size_reminder = if note_size == "small" {
                "REMINDER: 25-30 LINES ONLY. VERIFY COUNT BEFORE OUTPUT."
            } else {
                ""
            };

            let prompt = format!(
                "You are a strict note-writing assistant. Follow ALL rules EXACTLY.\n\n\
                 RULES:\n\
                 - Write a note about: \"{}\"\n\
                 - Language: {}. Write ONLY in this language.\n\
                 - Size: {}\n\
                 {}\
                 {}\
                 - Use the examples below for STYLE ONLY (headings, lists, code blocks). \
                 DO NOT copy their length or depth — follow the size rule above.\n\
                 - OUTPUT ONLY THE NOTE. No greetings, no introductions, no conclusions, \
                 no commentary, no extra text.\n\n\
                 STYLE EXAMPLES:\n{}\n\n\
                 {}\n\n\
                 OUTPUT:",
                clean_topic,
                lang,
                if note_size == "small" {
                    "SMALL — HARD LIMIT: EXACTLY 25-30 LINES.\n             - LINE BUDGET: 25-30 lines total. One idea per line, compact. Count lines as you write. Stop at 30 even if unfinished. Key points only, no fluff."
                } else {
                    "BIG — comprehensive and detailed. Full coverage of the topic."
                },
                hint_instruction,
                covered_instruction,
                examples,
                size_reminder,
            );

            logger.prompt(&format!(
                "--- Prompt for \"{}\" ---\n{}",
                clean_topic, prompt
            ));

            let hdrs = build_headers(&provider, &api_key);
            let res = match client
                .post(&provider.generate_url(&api_url, &model, &api_key))
                .headers(hdrs)
                .json(&provider.build_generate_body(&model, &prompt))
                .send()
                .await
            {
                Ok(r) => r,
                Err(e) => {
                    eprintln!("Error: Request failed for \"{}\": {}", clean_topic, e);
                    issue_prompt(&format!("[{}] Request failed: {}", clean_topic, e));
                    continue;
                }
            };

            if !res.status().is_success() {
                let status = res.status();
                let body = res.text().await.unwrap_or_else(|_| "Failed to read response body".to_string());
                eprintln!("Error: API returned non-success status for \"{}\": {}. Body: {}", clean_topic, status, body);
                issue_prompt(&format!("[{}] Non-success status: {}", clean_topic, status));
                continue;
            }

            let res_json: serde_json::Value = match res.json().await {
                Ok(v) => v,
                Err(e) => {
                    eprintln!("Error: Failed to parse response JSON for \"{}\": {}", clean_topic, e);
                    issue_prompt(&format!("[{}] JSON parse error: {}", clean_topic, e));
                    continue;
                }
            };
            let (text, eval_count_str) = provider.parse_generate_response(&res_json);

            if text.is_empty() {
                eprintln!("Error: API returned an empty response for \"{}\".", clean_topic);
                (String::new(), "N/A".to_string())
            } else {
                logger.response(&format!(
                    "--- Response for \"{}\" ---\n{}",
                    clean_topic, text
                ));
                (text, eval_count_str)
            }
        };

        let elapsed = start.elapsed();

        if generated_text.is_empty() {
            continue;
        }

        logger.timing(&format!(
            "  Took {} ms, generated tokens: {}",
            elapsed.as_millis(),
            eval_count
        ));

        if note_size == "small" {
            let line_count = generated_text.lines().count();
            if line_count < 25 || line_count > 30 {
                eprintln!(
                    "Warning: \"{}\" has {} lines (expected 25-30). Model exceeded limit.",
                    clean_topic, line_count
                );
            }
        }

        let safe_filename = clean_topic.replace([' ', '/', '\\', ':', '*', '?', '"', '<', '>', '|'], "_");
        let new_file_path = notes_path.join(format!("{}.md", safe_filename));

        if let Err(e) = fs::write(&new_file_path, &generated_text) {
            eprintln!("Error: Failed to write {}: {}", new_file_path.display(), e);
            issue_prompt(&format!("Failed to write file: {} - {}", new_file_path.display(), e));
            continue;
        }
        logger.status(&format!("  Saved: {}", new_file_path.display()));
    }

    Ok(())
}
