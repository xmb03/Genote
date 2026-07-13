use clap::Parser;
use reqwest::Client;
use serde::Deserialize;
use serde_json::json;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Instant;

// Maps directly to config.toml. lang, note_size and notes_count control
// how the generated notes look. notes_count is optional so serde sets it to None.
#[derive(Deserialize)]
struct Config {
    model: String,
    api_url: String,
    notes_dir: String,
    lang: String,
    note_size: String,
    notes_count: Option<u32>,
    use_covered_topics: Option<bool>,
}

#[derive(Parser)]
#[command(name = "genote", about = "Generate IT study notes using Ollama")]
struct Args {
    topic: String,

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
}

/// Expands paths like "~/Documents" to the full path using the $HOME env var.
/// Rust does not expand tildes on its own, so we do it manually.
fn expand_home(path: &str) -> PathBuf {
    if path.starts_with("~/") {
        if let Ok(home) = env::var("HOME") {
            return PathBuf::from(home).join(&path[2..]);
        }
    }
    PathBuf::from(path)
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Step 1: parse CLI args
    let args = Args::parse();

    let topic = &args.topic;

    // Extract optional hint from parentheses: "topic (hint)" → clean_topic + hint
    let (clean_topic, user_hint): (String, Option<String>) = topic
        .find('(')
        .and_then(|o| topic.rfind(')').map(|c| (o, c)))
        .filter(|(o, c)| o < c)
        .map(|(o, c)| {
            let clean = topic[..o].trim().to_string();
            let hint = topic[o + 1..c].trim().to_string();
            (clean, Some(hint))
        })
        .unwrap_or((topic.clone(), None));

    // Step 2: read config.toml
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

    let config: Config = toml::from_str(&config_content).unwrap_or_else(|e| {
        eprintln!("Error parsing config.toml: {}", e);
        std::process::exit(1);
    });

    // Merge CLI args into config (CLI takes precedence)
    let notes_dir = args.notes_dir.unwrap_or(config.notes_dir);
    let notes_count = args.notes_count.or(config.notes_count);
    let use_covered_topics = args.use_covered_topics.or(config.use_covered_topics);
    let note_size = args.note_size.unwrap_or(config.note_size);
    let lang = args.lang.unwrap_or(config.lang);
    let model = args.model.unwrap_or(config.model);
    let api_url = args.api_url.unwrap_or(config.api_url);

    // note_size must be either "small" or "big". We check it early
    // before doing anything else to fail fast.
    if note_size != "small" && note_size != "big" {
        eprintln!(
            "Error: note_size must be either \"small\" or \"big\", got \"{}\"",
            note_size
        );
        std::process::exit(1);
    }

    // Step 3: resolve the notes directory and check it exists
    // expand_home handles the tilde in the path if present.
    let notes_pathbuf = expand_home(&notes_dir);
    let path = Path::new(&notes_pathbuf);

    if !path.exists() {
        eprintln!(
            "Error: Notes directory does not exist: {:?}",
            notes_pathbuf
        );
        std::process::exit(1);
    }

    // Step 4: scan the notes directory for .md files to use as style examples
    // We read up to notes_count files (default 7) so the model can learn the writing style.
    let mut examples_text = String::new();
    let mut count: u32 = 0;
    let max_examples = notes_count.unwrap_or(7);

    // Collect all .md filenames as covered topics (before the limit, so we get the full list)
    let mut covered_topics: Vec<String> = Vec::new();
    if use_covered_topics.unwrap_or(false) {
        if let Ok(entries) = fs::read_dir(path) {
            for entry in entries.flatten() {
                let file_path = entry.path();
                if file_path.extension().and_then(|s| s.to_str()) == Some("md") {
                    if let Some(stem) = file_path.file_stem().and_then(|s| s.to_str()) {
                        covered_topics.push(stem.replace('_', " "));
                    }
                }
            }
        }
    }

    if let Ok(entries) = fs::read_dir(path) {
        for entry in entries.flatten() {
            let file_path = entry.path();

            if file_path.extension().and_then(|s| s.to_str()) == Some("md") {
                if let Ok(content) = fs::read_to_string(&file_path) {
                    examples_text.push_str(&format!(
                        "--- Example {} ---\n{}\n\n",
                        count + 1,
                        content
                    ));
                    count += 1;

                    if count >= max_examples {
                        break;
                    }
                }
            }
        }
    }

    if count == 0 {
        eprintln!("Error: No .md files found in the notes directory to extract style.");
        std::process::exit(1);
    }

    // Step 5: build a strict prompt for the model
    let size_instruction = match note_size.as_str() {
        "small" => "SMALL — brief note, 15-30 lines depending on the topic. Key points only, no fluff.",
        "big" => "BIG — comprehensive and detailed. Full coverage of the topic.",
        _ => unreachable!(), // we validated this above so it should never hit this branch
    };

    let hint_instruction = user_hint
        .as_ref()
        .map(|h| format!("- Additional instruction: {}\n", h))
        .unwrap_or_default();

    let covered_instruction = if use_covered_topics.unwrap_or(false) && !covered_topics.is_empty() {
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
        OUTPUT:",
        clean_topic,
        lang,
        size_instruction,
        hint_instruction,
        covered_instruction,
        examples_text,
    );

    println!(
        "Sending request (Model: {}, Topic: \"{}\", style examples: {})...",
        model, clean_topic, count
    );

    // Step 6: send the request to Ollama and time it
    let client = Client::new();
    let start = Instant::now();
    let res = client
        .post(&api_url)
        .json(&json!({
            "model": model,
            "prompt": prompt,
            "stream": false
        }))
        .send()
        .await?;

    let elapsed = start.elapsed();

    // Check that the API returned a 2xx status and print the error if not.
    if !res.status().is_success() {
        let status = res.status();
        let body = res
            .text()
            .await
            .unwrap_or_else(|_| "Failed to read response body".to_string());
        eprintln!(
            "Error: Ollama API returned non-success status: {}. Body: {}",
            status, body
        );
        std::process::exit(1);
    }

    // Parse the JSON response and pull out the generated text.
    let res_json: serde_json::Value = res.json().await?;
    let response_str = res_json
        .get("response")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let generated_text = response_str.trim().to_string();

    if generated_text.is_empty() {
        eprintln!("Error: Ollama returned an empty response field.");
        std::process::exit(1);
    }

    // Grab the token count from the response if the model provides it.
    let eval_count = res_json["eval_count"]
        .as_u64()
        .map(|v| v.to_string())
        .unwrap_or_else(|| "N/A".to_string());

    println!(
        "Request took {} ms, generated tokens: {}",
        elapsed.as_millis(),
        eval_count
    );

    // Step 7: save the generated note as a .md file in the notes directory
    // We replace spaces and slashes in the clean topic name so it works as a filename.
    let safe_filename = clean_topic.replace(' ', "_").replace('/', "_");
    let new_file_path = path.join(format!("{}.md", safe_filename));

    fs::write(&new_file_path, &generated_text)?;
    println!(
        "Success! New note saved to: {}",
        new_file_path.display()
    );

    Ok(())
}
