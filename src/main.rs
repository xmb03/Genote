use clap::Parser;
use reqwest::Client;
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use serde::Deserialize;
use serde_json::{Value, json};
use std::collections::{HashMap, HashSet};
use std::env;
use std::fs;
use std::io::{IsTerminal, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

mod provider;
use provider::Provider;

#[derive(Deserialize, Clone, Default)]
struct ConfigValues {
    model: Option<String>,
    api_url: Option<String>,
    provider: Option<String>,
    api_key: Option<String>,
    notes_dir: Option<String>,
    notes_include: Option<Vec<String>>,
    lang: Option<String>,
    note_size: Option<String>,
    notes_count: Option<u32>,
    use_covered_topics: Option<bool>,
    weak_mode: Option<bool>,
    temperature: Option<f32>,
    skill: Option<String>,
    log: Option<LogRaw>,
    #[serde(flatten)]
    extra: HashMap<String, Value>,
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

fn default_true() -> bool {
    true
}

#[derive(Deserialize)]
struct ConfigFile {
    default: Option<String>,
    profile: Option<HashMap<String, ConfigValues>>,
    #[serde(flatten)]
    values: ConfigValues,
}

struct Log {
    prompt: bool,
    response: bool,
    status: bool,
    timing: bool,
}

impl Log {
    fn resolve(raw: Option<LogRaw>) -> Self {
        match raw {
            None => Log {
                prompt: false,
                response: false,
                status: true,
                timing: true,
            },
            Some(LogRaw::Bool(true)) => Log {
                prompt: true,
                response: true,
                status: true,
                timing: true,
            },
            Some(LogRaw::Bool(false)) => Log {
                prompt: false,
                response: false,
                status: false,
                timing: false,
            },
            Some(LogRaw::Table(t)) => Log {
                prompt: t.prompt,
                response: t.response,
                status: t.status,
                timing: t.timing,
            },
        }
    }

    fn out(&self, on: bool, msg: &str) {
        if on {
            println!("{}", msg);
        }
    }

    fn prompt(&self, msg: &str) {
        self.out(self.prompt, &format!("[PROMPT]\n{}\n[/PROMPT]", msg));
    }

    fn response(&self, msg: &str) {
        self.out(self.response, &format!("[RESPONSE]\n{}\n[/RESPONSE]", msg));
    }

    fn status(&self, msg: &str) {
        self.out(self.status, msg);
    }
    fn timing(&self, msg: &str) {
        self.out(self.timing, msg);
    }
}

#[derive(Parser)]
#[command(
    about = "Generate IT study notes using local LLMs",
    version,
    help_template = "{about}\nVersion: {version}\n\n{usage-heading} {usage}\n\n{all-args}"
)]
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

    #[arg(long = "use-covered-topics", num_args = 0..=1, default_missing_value = "true", require_equals = true)]
    use_covered_topics: Option<bool>,

    #[arg(long = "weak-mode", action = clap::ArgAction::Set, num_args = 0..=1, default_missing_value = "true", require_equals = true)]
    weak_mode: Option<bool>,

    #[arg(
        long = "skill",
        help = "Path to a skill file: style rules injected into the model's system prompt"
    )]
    skill: Option<String>,

    #[arg(long = "profile")]
    profile: Option<String>,

    #[arg(long = "api-key", num_args = 0..=1, default_missing_value = "prompt", help = "API key override for this run. Pass without value to enter interactively. Persist in config.toml instead.")]
    api_key: Option<String>,
}

struct AppConfig {
    model: String,
    api_url: String,
    provider: Provider,
    api_key: String,
    notes_path: PathBuf,
    notes_include: Option<Vec<String>>,
    lang: String,
    note_size: String,
    notes_count: u32,
    use_covered: bool,
    weak_mode: bool,
    temperature: Option<f32>,
    skill: Option<String>,
    log: Log,
}

const ANALYSIS_SYSTEM: &str = "\
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

const DEFAULT_SYSTEM: &str = "You are a helpful assistant that writes study notes.";

const HTTP_TIMEOUT_SECS: u64 = 300;

fn warn_unknown_keys(label: &str, extra: &HashMap<String, Value>) {
    if !extra.is_empty() {
        let keys: Vec<&str> = extra.keys().map(|k| k.as_str()).collect();
        eprintln!(
            "Warning: unknown config key(s) in {}: {}",
            label,
            keys.join(", ")
        );
    }
}

fn expand_home(path: &str) -> PathBuf {
    let home_key = if cfg!(target_os = "windows") {
        "USERPROFILE"
    } else {
        "HOME"
    };
    if path == "~" {
        return PathBuf::from(env::var(home_key).unwrap_or_else(|_| ".".to_string()));
    }
    if path.starts_with("~/") {
        if let Ok(home) = env::var(home_key) {
            return PathBuf::from(home).join(path.strip_prefix("~/").unwrap_or(path));
        }
    }
    PathBuf::from(path)
}

fn pick<T: Clone>(cli: Option<T>, profile: Option<T>, root: Option<T>) -> Option<T> {
    cli.or(profile).or(root)
}

fn req<T>(v: Option<T>, name: &str) -> T {
    v.unwrap_or_else(|| fail(&format!("{name} is not set in config.toml or via CLI")))
}

fn on_error(ctx: &str) {
    eprintln!("Error: {}", ctx);
    issue_prompt(ctx);
}

fn fail(msg: &str) -> ! {
    eprintln!("Error: {}", msg);
    issue_prompt(msg);
    std::process::exit(1);
}

static ISSUE_PROMPTED: AtomicBool = AtomicBool::new(false);

fn issue_prompt(ctx: &str) {
    if env::var("GENOTE_NO_ISSUE").is_ok() || !std::io::stdin().is_terminal() {
        return;
    }
    if ISSUE_PROMPTED.swap(true, Ordering::Relaxed) {
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
    let short: String = ctx.chars().take(800).collect();
    let title = urlencoding::encode(&short).to_string();
    let body_text = format!(
        "## Error\n\n{}\n\n## Environment\n\n- OS: {}\n",
        short,
        std::env::consts::OS
    );
    let body = urlencoding::encode(&body_text).to_string();
    let url = format!(
        "https://github.com/xmb03/Genote/issues/new?title={}&body={}",
        title, body
    );
    let result = if cfg!(target_os = "windows") {
        std::process::Command::new("cmd")
            .args(["/c", "start", &url])
            .spawn()
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

async fn api_call(
    client: &Client,
    url: &str,
    headers: HeaderMap,
    body: Value,
) -> Result<Value, String> {
    let mut attempt = 0u32;
    loop {
        attempt += 1;
        let res = client
            .post(url)
            .headers(headers.clone())
            .json(&body)
            .send()
            .await
            .map_err(|e| e.to_string())?;
        let status = res.status();
        if status.is_success() {
            return res.json().await.map_err(|e| e.to_string());
        }
        let text = res.text().await.unwrap_or_default();
        if attempt < 3
            && (status == reqwest::StatusCode::TOO_MANY_REQUESTS || status.is_server_error())
        {
            tokio::time::sleep(Duration::from_secs(attempt as u64)).await;
            continue;
        }
        return Err(format!("HTTP {}: {}", status, text));
    }
}

async fn chat_once(
    client: &Client,
    cfg: &AppConfig,
    system: &str,
    messages: &[Value],
) -> Result<(String, String), String> {
    let mut msgs = vec![json!({"role": "system", "content": system})];
    msgs.extend_from_slice(messages);
    let url = cfg
        .provider
        .chat_url(&cfg.api_url, &cfg.model, &cfg.api_key);
    let body = cfg.provider.build_chat_body(
        &cfg.model,
        &json!(msgs),
        max_tokens_for(&cfg.note_size),
        cfg.temperature,
    );
    let res = api_call(
        client,
        &url,
        build_headers(&cfg.provider, &cfg.api_key),
        body,
    )
    .await?;
    Ok(cfg.provider.parse_response(&res))
}

fn size_instruction(size: &str) -> &'static str {
    if size == "small" {
        "SMALL — HARD LIMIT: EXACTLY 25-30 LINES.\nLINE BUDGET: 25-30 lines total. One idea per line, compact. Count lines as you write. Stop at 30 even if unfinished. Key points only, no fluff."
    } else if size == "mid" {
        "MID — HARD LIMIT: 45-68 LINES.\nLINE BUDGET: 45-68 lines total. One idea per line, compact but covering the topic well. Count lines as you write. Stop at 68 even if unfinished."
    } else {
        "BIG — comprehensive and detailed. Full coverage of the topic."
    }
}

fn size_reminder(size: &str) -> &'static str {
    if size == "small" {
        "REMINDER: 25-30 LINES ONLY. VERIFY COUNT BEFORE OUTPUT."
    } else if size == "mid" {
        "REMINDER: 45-68 LINES ONLY. VERIFY COUNT BEFORE OUTPUT."
    } else {
        ""
    }
}

fn size_line_bounds(size: &str) -> (u32, u32) {
    match size {
        "small" => (25, 30),
        "mid" => (45, 68),
        _ => (0, 0),
    }
}

fn max_tokens_for(note_size: &str) -> u32 {
    if note_size == "big" {
        8192
    } else if note_size == "mid" {
        6144
    } else {
        4096
    }
}

const WINDOWS_RESERVED: &[&str] = &[
    "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6",
    "COM7", "COM8", "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7",
    "LPT8", "LPT9",
];

fn sanitize_filename(topic: &str) -> String {
    let mut name = topic
        .trim_end_matches(|c| c == '.' || c == ' ')
        .replace([' ', '/', '\\', ':', '*', '?', '"', '<', '>', '|'], "_");
    if name.is_empty() {
        return "_".to_string();
    }
    let stem = name.split('.').next().unwrap_or("");
    if WINDOWS_RESERVED.contains(&stem.to_ascii_uppercase().as_str()) {
        name.insert_str(0, "_");
    }
    name
}

fn canonical_write_key(path: &PathBuf) -> PathBuf {
    if cfg!(target_os = "windows") || cfg!(target_os = "macos") {
        PathBuf::from(path.to_string_lossy().to_lowercase())
    } else {
        path.clone()
    }
}

fn parse_hint(topic: &str) -> (String, Option<String>) {
    let mut depth = 0i32;
    let mut start = None;
    let mut end = 0;
    for (i, ch) in topic.char_indices() {
        match ch {
            '(' if depth == 0 => {
                start = Some(i);
                depth = 1;
            }
            '(' => depth += 1,
            ')' if depth > 0 => {
                depth -= 1;
                if depth == 0 {
                    end = i;
                    break;
                }
            }
            _ => {}
        }
    }
    start
        .filter(|_| depth == 0)
        .map(|o| {
            (
                topic[..o].trim().to_string(),
                Some(topic[o + 1..end].trim().to_string()),
            )
        })
        .unwrap_or((topic.to_string(), None))
}

fn config_path() -> PathBuf {
    let config_dir = if cfg!(target_os = "windows") {
        env::var("APPDATA").unwrap_or_default()
    } else if cfg!(target_os = "macos") {
        env::var("HOME")
            .map(|h| format!("{}/Library/Application Support", h))
            .unwrap_or_default()
    } else {
        env::var("XDG_CONFIG_HOME").unwrap_or_else(|_| {
            env::var("HOME")
                .map(|h| format!("{}/.config", h))
                .unwrap_or_default()
        })
    };
    PathBuf::from(config_dir).join("Genote").join("config.toml")
}

fn select_profile(args: &Args, file: &ConfigFile) -> ConfigValues {
    let has_profiles = file
        .profile
        .as_ref()
        .map(|p| !p.is_empty())
        .unwrap_or(false);
    if !has_profiles {
        if args.profile.is_some() {
            fail("--profile flag requires [profile] sections in config.toml");
        }
        return ConfigValues::default();
    }
    let name = match args.profile.clone().or_else(|| file.default.clone()) {
        Some(n) => n,
        None => {
            eprintln!(
                "Warning: no default profile set and --profile not passed; using root-level settings"
            );
            return ConfigValues::default();
        }
    };
    file.profile
        .as_ref()
        .and_then(|p| p.get(&name))
        .cloned()
        .unwrap_or_else(|| fail(&format!("profile '{}' not found in config.toml", name)))
}

fn env_api_key() -> Option<String> {
    match env::var("GENOTE_API") {
        Ok(v) if !v.trim().is_empty() => Some(v.trim().to_string()),
        _ => None,
    }
}

fn resolve_config(args: &Args, profile: &ConfigValues, root: &ConfigValues) -> AppConfig {
    let provider = match Provider::from_str(
        &pick(
            args.provider.clone(),
            profile.provider.clone(),
            root.provider.clone(),
        )
        .unwrap_or_else(|| "ollama".to_string()),
    ) {
        Ok(p) => p,
        Err(e) => fail(&e),
    };
    let needs_api_key = matches!(
        provider,
        Provider::OpenAI | Provider::Anthropic | Provider::Gemini
    );
    let api_key = match args.api_key.as_deref() {
        Some("prompt") => prompt_api_key(),
        Some(k) => k.to_string(),
        None => env_api_key()
            .or_else(|| pick(None, profile.api_key.clone(), root.api_key.clone()))
            .or_else(|| needs_api_key.then(prompt_api_key))
            .unwrap_or_default(),
    };
    let model = req(
        pick(
            args.model.clone(),
            profile.model.clone(),
            root.model.clone(),
        ),
        "model",
    );
    let api_url = req(
        pick(
            args.api_url.clone(),
            profile.api_url.clone(),
            root.api_url.clone(),
        ),
        "api_url",
    );
    let notes_dir = req(
        pick(
            args.notes_dir.clone(),
            profile.notes_dir.clone(),
            root.notes_dir.clone(),
        ),
        "notes_dir",
    );
    let notes_include = pick(
        None,
        profile.notes_include.clone(),
        root.notes_include.clone(),
    )
    .map(|list| {
        list.into_iter()
            .map(|name| {
                if name.ends_with(".md") {
                    name
                } else {
                    format!("{}.md", name)
                }
            })
            .collect()
    })
    .filter(|l: &Vec<String>| !l.is_empty());
    let lang = req(
        pick(args.lang.clone(), profile.lang.clone(), root.lang.clone()),
        "lang",
    );
    let note_size = req(
        pick(
            args.note_size.clone(),
            profile.note_size.clone(),
            root.note_size.clone(),
        ),
        "note_size",
    );
    if note_size != "small" && note_size != "mid" && note_size != "big" {
        fail(&format!(
            "note_size must be either \"small\", \"mid\", or \"big\", got \"{}\"",
            note_size
        ));
    }
    let notes_count = pick(args.notes_count, profile.notes_count, root.notes_count).unwrap_or(7);
    if notes_count == 0 {
        fail("notes_count is set to 0, nothing to load.");
    }
    let use_covered = pick(
        args.use_covered_topics,
        profile.use_covered_topics,
        root.use_covered_topics,
    )
    .unwrap_or(false);
    let weak_mode = pick(args.weak_mode, profile.weak_mode, root.weak_mode).unwrap_or(false);
    let temperature = pick(None, profile.temperature, root.temperature);
    let notes_path = expand_home(&notes_dir);
    if !notes_path.is_dir() {
        fail(&format!(
            "Notes directory does not exist or is not a directory: {:?}",
            notes_path
        ));
    }
    let skill = pick(
        args.skill.clone(),
        profile.skill.clone(),
        root.skill.clone(),
    )
    .map(|p| {
        let path = expand_home(&p);
        fs::read_to_string(&path).unwrap_or_else(|e| {
            fail(&format!(
                "failed to read skill file {}: {}",
                path.display(),
                e
            ))
        })
    })
    .filter(|s| !s.trim().is_empty());
    let log = Log::resolve(pick(None, profile.log.clone(), root.log.clone()));
    AppConfig {
        model,
        api_url,
        provider,
        api_key,
        notes_path,
        notes_include,
        lang,
        note_size,
        notes_count,
        use_covered,
        weak_mode,
        temperature,
        skill,
        log,
    }
}

struct GenState {
    cfg: AppConfig,
    examples: String,
    covered_topics: Vec<String>,
    note_count: u32,
    skill_system: Option<String>,
    skill_preamble: String,
    failed: AtomicU32,
    written_files: Mutex<HashSet<PathBuf>>,
}

async fn generate_topic(
    client: &Client,
    state: &GenState,
    topic: &str,
    index: usize,
    total: usize,
) {
    let cfg = &state.cfg;
    let (clean_topic, user_hint) = parse_hint(topic);

    if clean_topic.is_empty() {
        on_error(&format!(
            "Topic \"{}\" is empty after hint removal. Hint must follow the topic text.",
            topic
        ));
        state.failed.fetch_add(1, Ordering::Relaxed);
        return;
    }

    let hint_instruction = user_hint
        .as_ref()
        .map(|h| format!("- Additional instruction: {}\n", h))
        .unwrap_or_default();

    let covered_instruction = if cfg.use_covered && !state.covered_topics.is_empty() {
        format!(
            "- Already covered topics: {}. \
             The reader ALREADY knows these topics — do NOT re-explain, redefine, \
             or recap them. Use them as a foundation and build on them to explain \
             the new topic.\n",
            state.covered_topics.join(", ")
        )
    } else {
        String::new()
    };

    cfg.log.status(&format!(
        "[{}/{}] Sending request (Model: {}, Topic: \"{}\", style examples: {}){}...",
        index + 1,
        total,
        cfg.model,
        clean_topic,
        state.note_count,
        if cfg.weak_mode {
            " [weak-mode: 2 stages]"
        } else {
            ""
        },
    ));

    let start = Instant::now();

    let (generated_text, eval_count) = if cfg.weak_mode {
        // ----- Stage 1: style analysis (skill goes into system AND user) -----
        let stage1_system = match &cfg.skill {
            Some(s) => format!("{}\n\n{}", s, ANALYSIS_SYSTEM),
            None => ANALYSIS_SYSTEM.to_string(),
        };
        let stage1_user = format!("{}{}", state.skill_preamble, state.examples);
        let analysis = match chat_once(
            client,
            cfg,
            &stage1_system,
            &[json!({"role": "user", "content": &stage1_user})],
        )
        .await
        {
            Ok((a, _)) if !a.is_empty() => a,
            Ok(_) => {
                on_error(&format!(
                    "[{}] Stage 1 returned empty analysis",
                    clean_topic
                ));
                state.failed.fetch_add(1, Ordering::Relaxed);
                return;
            }
            Err(e) => {
                on_error(&format!("[{}] Stage 1 request failed: {}", clean_topic, e));
                state.failed.fetch_add(1, Ordering::Relaxed);
                return;
            }
        };

        // ----- Stage 2: generation (skill stays in system only) -----
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
            cfg.lang,
            size_instruction(&cfg.note_size),
            hint_instruction,
            covered_instruction,
            size_reminder(&cfg.note_size),
        );

        cfg.log.prompt(&format!(
            "--- Stage 2 prompt for \"{}\" ---\n{}",
            clean_topic, stage2_prompt
        ));

        match chat_once(
            client,
            cfg,
            cfg.skill.as_deref().unwrap_or(DEFAULT_SYSTEM),
            &[
                json!({"role": "user", "content": &state.examples}),
                json!({"role": "assistant", "content": &analysis}),
                json!({"role": "user", "content": &stage2_prompt}),
            ],
        )
        .await
        {
            Ok((text, tokens)) if !text.is_empty() => {
                cfg.log.response(&format!(
                    "--- Response for \"{}\" ---\n{}",
                    clean_topic, text
                ));
                (text, tokens)
            }
            Ok(_) => {
                on_error(&format!(
                    "[{}] Stage 2 returned empty response",
                    clean_topic
                ));
                state.failed.fetch_add(1, Ordering::Relaxed);
                return;
            }
            Err(e) => {
                on_error(&format!("[{}] Stage 2 request failed: {}", clean_topic, e));
                state.failed.fetch_add(1, Ordering::Relaxed);
                return;
            }
        }
    } else {
        // ----- Normal mode: single generate request (skill in system AND user) -----
        let prompt = format!(
            "You are a strict note-writing assistant. Follow ALL rules EXACTLY.\n\n\
             {}\
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
            state.skill_preamble,
            clean_topic,
            cfg.lang,
            size_instruction(&cfg.note_size),
            hint_instruction,
            covered_instruction,
            state.examples,
            size_reminder(&cfg.note_size),
        );

        cfg.log.prompt(&format!(
            "--- Prompt for \"{}\" ---\n{}",
            clean_topic, prompt
        ));

        let url = cfg
            .provider
            .generate_url(&cfg.api_url, &cfg.model, &cfg.api_key);
        let body = cfg.provider.build_generate_body(
            &cfg.model,
            &prompt,
            state.skill_system.as_deref(),
            max_tokens_for(&cfg.note_size),
            cfg.temperature,
        );
        let res_json = match api_call(
            client,
            &url,
            build_headers(&cfg.provider, &cfg.api_key),
            body,
        )
        .await
        {
            Ok(v) => v,
            Err(e) => {
                on_error(&format!("Request failed for \"{}\": {}", clean_topic, e));
                state.failed.fetch_add(1, Ordering::Relaxed);
                return;
            }
        };
        let (text, eval_count) = cfg.provider.parse_response(&res_json);

        if text.is_empty() {
            on_error(&format!(
                "API returned an empty response for \"{}\".",
                clean_topic
            ));
            state.failed.fetch_add(1, Ordering::Relaxed);
            return;
        }
        cfg.log.response(&format!(
            "--- Response for \"{}\" ---\n{}",
            clean_topic, text
        ));
        (text, eval_count)
    };

    let elapsed = start.elapsed();

    cfg.log.timing(&format!(
        "  Took {} ms, generated tokens: {}",
        elapsed.as_millis(),
        eval_count
    ));

    let (min_lines, max_lines) = size_line_bounds(&cfg.note_size);
    if max_lines > 0 {
        let line_count = generated_text
            .lines()
            .filter(|l| !l.trim().is_empty())
            .count() as u32;
        if !(min_lines..=max_lines).contains(&line_count) {
            eprintln!(
                "Warning: \"{}\" has {} lines (expected {}-{}). Model exceeded limit.",
                clean_topic, line_count, min_lines, max_lines
            );
        }
    }

    let safe_filename = sanitize_filename(&clean_topic);
    let new_file_path = cfg.notes_path.join(format!("{}.md", safe_filename));

    let write_key = canonical_write_key(&new_file_path);
    let mut written = state.written_files.lock().unwrap();
    if new_file_path.exists() || written.contains(&write_key) {
        eprintln!("Warning: overwriting {}", new_file_path.display());
    }
    if let Err(e) = fs::write(&new_file_path, &generated_text) {
        on_error(&format!(
            "Failed to write file: {} - {}",
            new_file_path.display(),
            e
        ));
        state.failed.fetch_add(1, Ordering::Relaxed);
        return;
    }
    written.insert(write_key);
    cfg.log
        .status(&format!("  Saved: {}", new_file_path.display()));
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    let config_path = config_path();
    let config_content = fs::read_to_string(&config_path).unwrap_or_else(|_| {
        fail(&format!(
            "config.toml not found. Place it at: {:?}. Or create it based on config.toml.example",
            config_path
        ))
    });

    let config_file: ConfigFile = toml::from_str(&config_content)
        .unwrap_or_else(|e| fail(&format!("config parse error: {}", e)));
    warn_unknown_keys("root config", &config_file.values.extra);
    if let Some(profiles) = &config_file.profile {
        for (name, vals) in profiles {
            warn_unknown_keys(&format!("profile '{}'", name), &vals.extra);
        }
    }
    let profile_vals = select_profile(&args, &config_file);
    let cfg = resolve_config(&args, &profile_vals, &config_file.values);

    let mut examples = String::new();
    let mut count = 0u32;

    match &cfg.notes_include {
        Some(list) => {
            let mut seen = HashSet::new();
            for name in list {
                if !seen.insert(name) {
                    continue;
                }
                let p = cfg.notes_path.join(name);
                if !p.is_file() {
                    eprintln!(
                        "Warning: skipped {} — not found in {:?}",
                        name, cfg.notes_path
                    );
                    continue;
                }
                if count < cfg.notes_count {
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
        None => {
            let mut files: Vec<PathBuf> = fs::read_dir(&cfg.notes_path)
                .unwrap_or_else(|e| {
                    fail(&format!(
                        "Failed to read notes directory {:?}: {}",
                        cfg.notes_path, e
                    ))
                })
                .flatten()
                .map(|e| e.path())
                .filter(|p| {
                    p.extension()
                        .and_then(|s| s.to_str())
                        .map(|s| s.eq_ignore_ascii_case("md"))
                        .unwrap_or(false)
                })
                .collect();
            files.sort();
            for p in files {
                if count < cfg.notes_count {
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
    }

    let mut covered_topics: Vec<String> = Vec::new();
    if cfg.use_covered {
        let entries = fs::read_dir(&cfg.notes_path).unwrap_or_else(|e| {
            fail(&format!(
                "Failed to read notes directory {:?}: {}",
                cfg.notes_path, e
            ))
        });
        for entry in entries.flatten() {
            let p = entry.path();
            if !p
                .extension()
                .and_then(|s| s.to_str())
                .map(|s| s.eq_ignore_ascii_case("md"))
                .unwrap_or(false)
            {
                continue;
            }
            if let Some(stem) = p.file_stem().and_then(|s| s.to_str()) {
                covered_topics.push(stem.replace('_', " "));
            }
        }
        covered_topics.sort();
    }

    if count == 0 {
        fail(if cfg.notes_include.is_some() {
            "None of the files listed in notes_include were found in the notes directory."
        } else {
            "No .md files found in the notes directory."
        });
    }

    let client = Client::builder()
        .timeout(Duration::from_secs(HTTP_TIMEOUT_SECS))
        .build()
        .unwrap_or_else(|e| fail(&format!("failed to build HTTP client: {}", e)));

    let skill_system = if matches!(&cfg.provider, Provider::Llamacpp) {
        None
    } else {
        cfg.skill.clone()
    };
    let skill_preamble = cfg.skill.as_ref().map(|s| {
        format!(
            "STYLE SKILL (STRICT — these rules override the style examples below):\n{}\n\n",
            s
        )
    }).unwrap_or_default();

    let state = Arc::new(GenState {
        cfg,
        examples,
        covered_topics,
        note_count: count,
        skill_system,
        skill_preamble,
        failed: AtomicU32::new(0),
        written_files: Mutex::new(HashSet::new()),
    });

    let topics: Vec<String> = args.topics.clone();

    let is_cloud = matches!(
        state.cfg.provider,
        Provider::OpenAI | Provider::Anthropic | Provider::Gemini
    );

    if is_cloud && topics.len() > 1 {
        // Cloud providers: generate all topics concurrently.
        let mut handles = Vec::new();
        for (i, topic) in topics.iter().enumerate() {
            let client = client.clone();
            let state = Arc::clone(&state);
            let topic = topic.clone();
            let total = topics.len();
            handles.push(tokio::spawn(async move {
                generate_topic(&client, &state, &topic, i, total).await;
            }));
        }
        for h in handles {
            let _ = h.await;
        }
    } else {
        // Local providers (or single topic): sequential.
        for (i, topic) in topics.iter().enumerate() {
            generate_topic(&client, &state, topic, i, topics.len()).await;
        }
    }

    if state.failed.load(Ordering::Relaxed) > 0 {
        std::process::exit(1);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_hint_basic() {
        assert_eq!(parse_hint("Rust"), ("Rust".to_string(), None));
        assert_eq!(
            parse_hint("Closures (skip FnOnce)"),
            ("Closures".to_string(), Some("skip FnOnce".to_string()))
        );
        assert_eq!(
            parse_hint("Generics (no (traits)) here"),
            ("Generics".to_string(), Some("no (traits)".to_string()))
        );
    }

    #[test]
    fn parse_hint_edges() {
        assert_eq!(parse_hint("()"), ("".to_string(), Some("".to_string())));
        assert_eq!(
            parse_hint("(hint)"),
            ("".to_string(), Some("hint".to_string()))
        );
        assert_eq!(parse_hint("(unclosed"), ("(unclosed".to_string(), None));
        assert_eq!(parse_hint("Topic )("), ("Topic )(".to_string(), None));
    }

    #[test]
    fn pick_merge_order() {
        assert_eq!(pick(Some("cli"), Some("prof"), Some("root")), Some("cli"));
        assert_eq!(pick(None, Some("prof"), Some("root")), Some("prof"));
        assert_eq!(pick(None, None, Some("root")), Some("root"));
        assert_eq!(pick::<String>(None, None, None), None);
    }

    #[test]
    fn max_tokens_by_size() {
        assert_eq!(max_tokens_for("big"), 8192);
        assert_eq!(max_tokens_for("mid"), 6144);
        assert_eq!(max_tokens_for("small"), 4096);
    }

    #[test]
    fn size_line_bounds_by_size() {
        assert_eq!(size_line_bounds("small"), (25, 30));
        assert_eq!(size_line_bounds("mid"), (45, 68));
        assert_eq!(size_line_bounds("big"), (0, 0));
    }

    #[test]
    fn sanitize_filename_handles_windows_reserved_and_edges() {
        assert_eq!(sanitize_filename("CON"), "_CON");
        assert_eq!(sanitize_filename("con"), "_con");
        assert_eq!(sanitize_filename("COM1"), "_COM1");
        assert_eq!(sanitize_filename("NUL"), "_NUL");
        assert_eq!(sanitize_filename("CON.md"), "_CON.md");
        assert_eq!(sanitize_filename("Rust"), "Rust");
        assert_eq!(sanitize_filename("Rust ownership"), "Rust_ownership");
        assert_eq!(sanitize_filename("topic."), "topic");
        assert_eq!(sanitize_filename("topic.. "), "topic");
        assert_eq!(sanitize_filename("..."), "_");
        assert_eq!(sanitize_filename("a/b:c"), "a_b_c");
    }
}
