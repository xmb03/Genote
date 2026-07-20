use clap::Parser;
use reqwest::Client;
use serde::Deserialize;
use serde_json::json;
use std::collections::HashMap;
use std::env;
use std::fs;
use std::path::PathBuf;
use std::time::Instant;

#[derive(Deserialize, Clone, Default)]
struct ConfigValues {
    model: Option<String>,
    api_url: Option<String>,
    notes_dir: Option<String>,
    lang: Option<String>,
    note_size: Option<String>,
    notes_count: Option<u32>,
    use_covered_topics: Option<bool>,
}

#[derive(Deserialize)]
struct ConfigFile {
    default: Option<String>,
    profile: Option<HashMap<String, ConfigValues>>,
    #[serde(flatten)]
    values: ConfigValues,
}

#[derive(Parser)]
#[command(about = "Generate IT study notes using Ollama")]
struct Args {
    #[arg(required = true)]
    topics: Vec<String>,

    #[arg(short = 'm', long = "model")]
    model: Option<String>,

    #[arg(long = "api-url")]
    api_url: Option<String>,

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

    #[arg(long = "profile")]
    profile: Option<String>,
}

fn expand_home(path: &str) -> PathBuf {
    if path.starts_with("~/") {
        if let Ok(home) = env::var("HOME") {
            return PathBuf::from(home).join(&path[2..]);
        }
    }
    PathBuf::from(path)
}

fn req<T>(v: Option<T>, name: &str) -> T {
    v.unwrap_or_else(|| {
        eprintln!("Error: {name} is not set in config.toml or via CLI");
        std::process::exit(1);
    })
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    let config_path = env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.join("config.toml")))
        .filter(|p| p.exists())
        .unwrap_or_else(|| PathBuf::from("config.toml"));

    let config_content = fs::read_to_string(&config_path).unwrap_or_else(|_| {
        eprintln!("Error: config.toml not found next to the binary.");
        eprintln!("Place it at: {:?}", config_path);
        eprintln!("Or create it based on config.toml.example");
        std::process::exit(1);
    });

    let config_file: ConfigFile = toml::from_str(&config_content).unwrap_or_else(|e| {
        eprintln!("Error parsing config.toml: {}", e);
        std::process::exit(1);
    });

    let has_profiles = config_file.profile.as_ref().map(|p| !p.is_empty()).unwrap_or(false);
    let cli_profile = args.profile.clone();

    let profile_vals = if has_profiles {
        let name = cli_profile.or(config_file.default.clone()).unwrap_or_else(|| {
            eprintln!("Error: --profile not specified and no default profile set in config.toml");
            std::process::exit(1);
        });
        config_file.profile.as_ref()
            .and_then(|p| p.get(&name))
            .cloned()
            .unwrap_or_else(|| {
                eprintln!("Error: profile '{}' not found in config.toml", name);
                std::process::exit(1)
            })
    } else {
        if cli_profile.is_some() {
            eprintln!("Error: --profile flag requires [profile] sections in config.toml");
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
        std::process::exit(1);
    }
    let notes_count = args.notes_count
        .or(profile_vals.notes_count)
        .or(config_file.values.notes_count)
        .unwrap_or(7);
    let use_covered = args.use_covered_topics
        .or(profile_vals.use_covered_topics)
        .or(config_file.values.use_covered_topics)
        .unwrap_or(false);

    let notes_path = expand_home(&notes_dir);
    if !notes_path.exists() {
        eprintln!("Error: Notes directory does not exist: {:?}", notes_path);
        std::process::exit(1);
    }

    let mut examples = String::new();
    let mut count = 0u32;
    let mut covered_topics: Vec<String> = Vec::new();

    if let Ok(entries) = fs::read_dir(&notes_path) {
        for entry in entries.flatten() {
            let p = entry.path();
            if p.extension().and_then(|s| s.to_str()) != Some("md") {
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
        std::process::exit(1);
    }

    let client = Client::new();

    for topic in &args.topics {
        let (clean_topic, user_hint) = topic
            .find('(')
            .and_then(|o| topic.rfind(')').map(|c| (o, c)))
            .filter(|(o, c)| o < c)
            .map(|(o, c)| {
                (topic[..o].trim().to_string(), Some(topic[o + 1..c].trim().to_string()))
            })
            .unwrap_or((topic.clone(), None));

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
             REMINDER: 25-30 LINES ONLY. VERIFY COUNT BEFORE OUTPUT.\n\n\
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
        );

        println!(
            "[{}/{}] Sending request (Model: {}, Topic: \"{}\", style examples: {})...",
            args.topics.iter().position(|t| t == topic).unwrap_or(0) + 1,
            args.topics.len(),
            model,
            clean_topic,
            count
        );

        let start = Instant::now();
        let res = match client
            .post(&api_url)
            .json(&json!({
                "model": model,
                "prompt": prompt,
                "stream": false
            }))
            .send()
            .await
        {
            Ok(r) => r,
            Err(e) => {
                eprintln!("Error: Request failed for \"{}\": {}", clean_topic, e);
                continue;
            }
        };

        let elapsed = start.elapsed();

        if !res.status().is_success() {
            let status = res.status();
            let body = res.text().await.unwrap_or_else(|_| "Failed to read response body".to_string());
            eprintln!("Error: Ollama API returned non-success status for \"{}\": {}. Body: {}", clean_topic, status, body);
            continue;
        }

        let res_json: serde_json::Value = match res.json().await {
            Ok(v) => v,
            Err(e) => {
                eprintln!("Error: Failed to parse response JSON for \"{}\": {}", clean_topic, e);
                continue;
            }
        };
        let generated_text = res_json
            .get("response")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .trim()
            .to_string();

        if generated_text.is_empty() {
            eprintln!("Error: Ollama returned an empty response for \"{}\".", clean_topic);
            continue;
        }

        let eval_count = res_json["eval_count"]
            .as_u64()
            .map(|v| v.to_string())
            .unwrap_or_else(|| "N/A".to_string());

        println!(
            "  Took {} ms, generated tokens: {}",
            elapsed.as_millis(),
            eval_count
        );

        if note_size == "small" {
            let line_count = generated_text.lines().count();
            if line_count < 25 || line_count > 30 {
                eprintln!(
                    "Warning: \"{}\" has {} lines (expected 25-30). Model exceeded limit.",
                    clean_topic, line_count
                );
            }
        }

        let safe_filename = clean_topic.replace(' ', "_").replace('/', "_");
        let new_file_path = notes_path.join(format!("{}.md", safe_filename));

        if let Err(e) = fs::write(&new_file_path, &generated_text) {
            eprintln!("Error: Failed to write {}: {}", new_file_path.display(), e);
            continue;
        }
        println!("  Saved: {}", new_file_path.display());
    }

    Ok(())
}
