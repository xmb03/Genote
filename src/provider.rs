use serde_json::{json, Value};

pub enum Provider {
    Ollama,
    OpenAI,
    Llamacpp,
    Anthropic,
    Gemini,
}

impl Provider {
    pub fn from_str(s: &str) -> Self {
        match s {
            "openai" => Provider::OpenAI,
            "llamacpp" => Provider::Llamacpp,
            "anthropic" => Provider::Anthropic,
            "gemini" => Provider::Gemini,
            _ => Provider::Ollama,
        }
    }

    pub fn base_url(api_url: &str) -> String {
        let known_suffixes = [
            "/api/generate",
            "/api/generate/",
            "/v1/chat/completions",
            "/v1/chat/completions/",
            "/v1/completions",
            "/v1/completions/",
            "/completion",
            "/completion/",
            "/v1/messages",
            "/v1/messages/",
        ];
        for suffix in &known_suffixes {
            if let Some(stripped) = api_url.strip_suffix(suffix) {
                return stripped.trim_end_matches('/').to_string();
            }
        }
        api_url.trim_end_matches('/').to_string()
    }

    pub fn headers(&self, api_key: &str) -> Vec<(&'static str, String)> {
        match self {
            Provider::Ollama | Provider::Llamacpp | Provider::Gemini => vec![],
            Provider::OpenAI => vec![("Authorization", format!("Bearer {}", api_key))],
            Provider::Anthropic => vec![
                ("x-api-key", api_key.to_string()),
                ("anthropic-version", "2023-06-01".to_string()),
            ],
        }
    }

    pub fn generate_url(&self, api_url: &str, model: &str, api_key: &str) -> String {
        let base = Provider::base_url(api_url);
        match self {
            Provider::Ollama => format!("{}/api/generate", base),
            Provider::OpenAI => format!("{}/v1/chat/completions", base),
            Provider::Llamacpp => format!("{}/completion", base),
            Provider::Anthropic => format!("{}/v1/messages", base),
            Provider::Gemini => format!("{}/v1beta/models/{}:generateContent?key={}", base, model, api_key),
        }
    }

    pub fn chat_url(&self, api_url: &str, model: &str, api_key: &str) -> String {
        let base = Provider::base_url(api_url);
        match self {
            Provider::Ollama => format!("{}/api/chat", base),
            Provider::OpenAI | Provider::Llamacpp => format!("{}/v1/chat/completions", base),
            Provider::Anthropic => format!("{}/v1/messages", base),
            Provider::Gemini => format!("{}/v1beta/models/{}:generateContent?key={}", base, model, api_key),
        }
    }

    pub fn build_generate_body(&self, model: &str, prompt: &str) -> Value {
        match self {
            Provider::OpenAI => json!({
                "model": model,
                "messages": [{"role": "user", "content": prompt}],
                "stream": false
            }),
            Provider::Ollama => json!({
                "model": model,
                "prompt": prompt,
                "stream": false
            }),
            Provider::Llamacpp => json!({
                "prompt": prompt,
                "stream": false
            }),
            Provider::Anthropic => json!({
                "model": model,
                "max_tokens": 4096,
                "messages": [{"role": "user", "content": prompt}],
            }),
            Provider::Gemini => json!({
                "contents": [{"parts": [{"text": prompt}]}]
            }),
        }
    }

    pub fn build_chat_body(&self, model: &str, messages: &Value) -> Value {
        match self {
            Provider::Anthropic => {
                let msgs = messages.as_array().cloned().unwrap_or_default();
                let mut system = None;
                let mut filtered = Vec::new();
                for msg in &msgs {
                    if msg["role"] == "system" {
                        system = Some(msg["content"].as_str().unwrap_or("").to_string());
                    } else {
                        filtered.push(msg.clone());
                    }
                }
                let mut body = json!({
                    "model": model,
                    "max_tokens": 4096,
                    "messages": filtered,
                });
                if let Some(s) = system {
                    body["system"] = json!(s);
                }
                body
            }
            Provider::Gemini => {
                let msgs = messages.as_array().cloned().unwrap_or_default();
                let mut contents = Vec::new();
                let mut system = None;
                for msg in &msgs {
                    if msg["role"] == "system" {
                        system = Some(msg["content"].as_str().unwrap_or("").to_string());
                    } else {
                        let role = if msg["role"] == "assistant" { "model" } else { "user" };
                        contents.push(json!({
                            "role": role,
                            "parts": [{"text": msg["content"]}]
                        }));
                    }
                }
                let mut body = json!({ "contents": contents });
                if let Some(s) = system {
                    body["system_instruction"] = json!({"parts": [{"text": s}]});
                }
                body
            }
            _ => json!({
                "model": model,
                "messages": messages,
                "stream": false
            }),
        }
    }

    pub fn parse_generate_response(&self, body: &Value) -> (String, String) {
        self.parse_response(body)
    }

    pub fn parse_chat_response(&self, body: &Value) -> (String, String) {
        self.parse_response(body)
    }

    fn parse_response(&self, body: &Value) -> (String, String) {
        match self {
            Provider::Ollama => {
                let text = body
                    .get("response")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .trim()
                    .to_string();
                let tokens = body
                    .get("eval_count")
                    .and_then(|v| v.as_u64())
                    .map(|v| v.to_string())
                    .unwrap_or_else(|| "N/A".to_string());
                (text, tokens)
            }
            Provider::OpenAI | Provider::Llamacpp => {
                let text = body
                    .get("choices")
                    .and_then(|c| c.as_array())
                    .and_then(|c| c.first())
                    .and_then(|c| c.get("message"))
                    .and_then(|m| m.get("content"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .trim()
                    .to_string();
                let tokens = body
                    .get("usage")
                    .and_then(|u| u.get("total_tokens"))
                    .and_then(|v| v.as_u64())
                    .map(|v| v.to_string())
                    .unwrap_or_else(|| "N/A".to_string());
                (text, tokens)
            }
            Provider::Anthropic => {
                let text = body
                    .get("content")
                    .and_then(|c| c.as_array())
                    .and_then(|c| c.first())
                    .and_then(|b| b.get("text"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .trim()
                    .to_string();
                let tokens = {
                    let inp = body.get("usage").and_then(|u| u.get("input_tokens")).and_then(|v| v.as_u64()).unwrap_or(0);
                    let out = body.get("usage").and_then(|u| u.get("output_tokens")).and_then(|v| v.as_u64()).unwrap_or(0);
                    (inp + out).to_string()
                };
                (text, tokens)
            }
            Provider::Gemini => {
                let text = body
                    .get("candidates")
                    .and_then(|c| c.as_array())
                    .and_then(|c| c.first())
                    .and_then(|c| c.get("content"))
                    .and_then(|c| c.get("parts"))
                    .and_then(|p| p.as_array())
                    .and_then(|p| p.first())
                    .and_then(|p| p.get("text"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .trim()
                    .to_string();
                let tokens = body
                    .get("usageMetadata")
                    .and_then(|u| u.get("totalTokenCount"))
                    .and_then(|v| v.as_u64())
                    .map(|v| v.to_string())
                    .unwrap_or_else(|| "N/A".to_string());
                (text, tokens)
            }
        }
    }
}
