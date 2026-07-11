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
    // Step 1: grab the topic from the command line
    // The program takes exactly one argument. No argument, no point in continuing.
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: {} \"TOPIC NAME\"", args[0]);
        std::process::exit(1);
    }
    let topic = &args[1];

    // Step 2: read config.toml from the binary's directory
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

    // note_size must be either "small" or "big". We check it early
    // before doing anything else to fail fast.
    if config.note_size != "small" && config.note_size != "big" {
        eprintln!(
            "Error: note_size must be either \"small\" or \"big\", got \"{}\"",
            config.note_size
        );
        std::process::exit(1);
    }

    // Step 3: resolve the notes directory and check it exists
    // expand_home handles the tilde in the path if present.
    let notes_pathbuf = expand_home(&config.notes_dir);
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
    let max_examples = config.notes_count.unwrap_or(7);

    // Collect all .md filenames as covered topics (before the limit, so we get the full list)
    let mut covered_topics: Vec<String> = Vec::new();
    if config.use_covered_topics.unwrap_or(false) {
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

    // Step 5: build the prompt for the model
    // We feed the style examples, the topic, the target language, and a size hint.
    let size_instruction = match config.note_size.as_str() {
        "small" => "The note should be small — concise and to the point.",
        "big" => "The note should be big — comprehensive and detailed.",
        _ => unreachable!(), // we validated this above so it should never hit this branch
    };

    let covered_instruction = if config.use_covered_topics.unwrap_or(false) && !covered_topics.is_empty() {
        format!(
            "\n\nYou have already studied the following topics: {}.\n\
            Only use concepts from exactly these topics. \
            Do NOT introduce any concepts, terms, or ideas from topics outside this list. \
            If you do not have enough covered topics to write about the requested subject, \
            write only about what is directly related to the listed topics.",
            covered_topics.join(", ")
        )
    } else {
        String::new()
    };

    let prompt = format!(
        "Below are examples of my IT notes in Markdown format.\n\n\
        {}\n\
        Based on the style, structure, level of detail, and presentation of these examples, \
        write a new note on the topic: \"{}\".\n\n\
        Write the note strictly in {} language. {}\
        {}\n\
        OUTPUT ONLY THE NOTE IN MARKDOWN FORMAT. Do not add any introductory or concluding remarks.",
        examples_text, topic, config.lang, size_instruction, covered_instruction
    );

    println!(
        "Sending request (Model: {}, Topic: \"{}\", style examples: {})...",
        config.model, topic, count
    );

    // Step 6: send the request to Ollama and time it
    let client = Client::new();
    let start = Instant::now();
    let res = client
        .post(&config.api_url)
        .json(&json!({
            "model": config.model,
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
    // We replace spaces and slashes in the topic name so it works as a filename.
    let safe_filename = topic.replace(' ', "_").replace('/', "_");
    let new_file_path = path.join(format!("{}.md", safe_filename));

    fs::write(&new_file_path, &generated_text)?;
    println!(
        "Success! New note saved to: {}",
        new_file_path.display()
    );

    Ok(())
}
