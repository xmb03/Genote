use serde_json::{Value, json};

#[derive(Debug)]
pub enum Provider {
    Ollama,
    OpenAI,
    Llamacpp,
    Anthropic,
    Gemini,
}

impl Provider {
    pub fn from_str(s: &str) -> Result<Self, String> {
        match s {
            "ollama" => Ok(Provider::Ollama),
            "openai" => Ok(Provider::OpenAI),
            "llamacpp" => Ok(Provider::Llamacpp),
            "anthropic" => Ok(Provider::Anthropic),
            "gemini" => Ok(Provider::Gemini),
            other => Err(format!(
                "unknown provider \"{}\" (expected: ollama, openai, llamacpp, anthropic, gemini)",
                other
            )),
        }
    }

    fn base_url(api_url: &str) -> String {
        let api_url = match api_url.find(":generateContent") {
            Some(pos) => {
                let truncated = &api_url[..pos];
                match truncated.rfind("models/") {
                    Some(m) => &truncated[..m],
                    None => truncated,
                }
            }
            None => api_url,
        };
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
            "/v1",
            "/v1/",
            "/v1beta",
            "/v1beta/",
        ];
        for suffix in &known_suffixes {
            if let Some(stripped) = api_url.strip_suffix(suffix) {
                return stripped.trim_end_matches('/').to_string();
            }
        }
        let trimmed = api_url.trim_end_matches('/');
        let authority_start = trimmed.find("://").map_or(0, |i| i + 3);
        match trimmed.rfind('/') {
            Some(pos) if pos > authority_start => trimmed[..pos].to_string(),
            _ => trimmed.to_string(),
        }
    }

    pub fn headers(&self, api_key: &str) -> Vec<(&'static str, String)> {
        match self {
            Provider::Ollama | Provider::Llamacpp => vec![],
            Provider::OpenAI => vec![("Authorization", format!("Bearer {}", api_key))],
            Provider::Anthropic => vec![
                ("x-api-key", api_key.to_string()),
                ("anthropic-version", "2023-06-01".to_string()),
            ],
            Provider::Gemini => vec![("x-goog-api-key", api_key.to_string())],
        }
    }

    pub fn generate_url(&self, api_url: &str, model: &str, _api_key: &str) -> String {
        let base = Provider::base_url(api_url);
        match self {
            Provider::Ollama => format!("{}/api/generate", base),
            Provider::OpenAI => format!("{}/v1/chat/completions", base),
            Provider::Llamacpp => format!("{}/completion", base),
            Provider::Anthropic => format!("{}/v1/messages", base),
            Provider::Gemini => format!("{}/v1beta/models/{}:generateContent", base, model),
        }
    }

    pub fn chat_url(&self, api_url: &str, model: &str, _api_key: &str) -> String {
        let base = Provider::base_url(api_url);
        match self {
            Provider::Ollama => format!("{}/api/chat", base),
            Provider::OpenAI | Provider::Llamacpp => format!("{}/v1/chat/completions", base),
            Provider::Anthropic => format!("{}/v1/messages", base),
            Provider::Gemini => format!("{}/v1beta/models/{}:generateContent", base, model),
        }
    }

    pub fn build_generate_body(
        &self,
        model: &str,
        prompt: &str,
        system: Option<&str>,
        max_tokens: u32,
        temperature: Option<f32>,
    ) -> Value {
        match self {
            Provider::OpenAI => {
                let mut messages = Vec::new();
                if let Some(s) = system {
                    messages.push(json!({"role": "system", "content": s}));
                }
                messages.push(json!({"role": "user", "content": prompt}));
                let mut body = json!({
                    "model": model,
                    "messages": messages,
                    "max_tokens": max_tokens,
                    "stream": false
                });
                if let Some(t) = temperature {
                    body["temperature"] = json!(t);
                }
                body
            }
            Provider::Ollama => {
                let mut body = json!({
                    "model": model,
                    "prompt": prompt,
                    "stream": false,
                    "options": {"num_predict": max_tokens}
                });
                if let Some(s) = system {
                    body["system"] = json!(s);
                }
                if let Some(t) = temperature {
                    body["options"]["temperature"] = json!(t);
                }
                body
            }
            Provider::Llamacpp => {
                let mut body = json!({
                    "prompt": prompt,
                    "stream": false,
                    "n_predict": max_tokens
                });
                if let Some(t) = temperature {
                    body["temperature"] = json!(t);
                }
                body
            }
            Provider::Anthropic => {
                let mut body = json!({
                    "model": model,
                    "max_tokens": max_tokens,
                    "messages": [{"role": "user", "content": prompt}],
                });
                if let Some(s) = system {
                    body["system"] = json!(s);
                }
                if let Some(t) = temperature {
                    body["temperature"] = json!(t);
                }
                body
            }
            Provider::Gemini => {
                let mut body = json!({
                    "contents": [{"parts": [{"text": prompt}]}]
                });
                if let Some(s) = system {
                    body["system_instruction"] = json!({"parts": [{"text": s}]});
                }
                let mut gen_cfg = json!({"maxOutputTokens": max_tokens});
                if let Some(t) = temperature {
                    gen_cfg["temperature"] = json!(t);
                }
                body["generationConfig"] = gen_cfg;
                body
            }
        }
    }

    pub fn build_chat_body(
        &self,
        model: &str,
        messages: &Value,
        max_tokens: u32,
        temperature: Option<f32>,
    ) -> Value {
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
                    "max_tokens": max_tokens,
                    "messages": filtered,
                });
                if let Some(s) = system {
                    body["system"] = json!(s);
                }
                if let Some(t) = temperature {
                    body["temperature"] = json!(t);
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
                        let role = if msg["role"] == "assistant" {
                            "model"
                        } else {
                            "user"
                        };
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
                let mut gen_cfg = json!({"maxOutputTokens": max_tokens});
                if let Some(t) = temperature {
                    gen_cfg["temperature"] = json!(t);
                }
                body["generationConfig"] = gen_cfg;
                body
            }
            Provider::OpenAI => {
                let mut body = json!({
                    "model": model,
                    "messages": messages,
                    "max_tokens": max_tokens,
                    "stream": false
                });
                if let Some(t) = temperature {
                    body["temperature"] = json!(t);
                }
                body
            }
            Provider::Ollama => {
                let mut body = json!({
                    "model": model,
                    "messages": messages,
                    "stream": false,
                    "options": {"num_predict": max_tokens}
                });
                if let Some(t) = temperature {
                    body["options"]["temperature"] = json!(t);
                }
                body
            }
            Provider::Llamacpp => {
                let mut body = json!({
                    "model": model,
                    "messages": messages,
                    "max_tokens": max_tokens,
                    "stream": false
                });
                if let Some(t) = temperature {
                    body["temperature"] = json!(t);
                }
                body
            }
        }
    }

    pub(crate) fn parse_response(&self, body: &Value) -> (String, String) {
        match self {
            Provider::Ollama => {
                let text = body
                    .get("message")
                    .and_then(|m| m.get("content"))
                    .and_then(|v| v.as_str())
                    .or_else(|| body.get("response").and_then(|v| v.as_str()))
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
            Provider::OpenAI => {
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
            Provider::Llamacpp => {
                let text = body
                    .get("content")
                    .and_then(|v| v.as_str())
                    .or_else(|| {
                        body.get("choices")
                            .and_then(|c| c.as_array())
                            .and_then(|c| c.first())
                            .and_then(|c| c.get("message"))
                            .and_then(|m| m.get("content"))
                            .and_then(|v| v.as_str())
                    })
                    .unwrap_or("")
                    .trim()
                    .to_string();
                let tokens = body
                    .get("timings")
                    .and_then(|t| t.get("predicted_n"))
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
                    let inp = body
                        .get("usage")
                        .and_then(|u| u.get("input_tokens"))
                        .and_then(|v| v.as_u64())
                        .unwrap_or(0);
                    let out = body
                        .get("usage")
                        .and_then(|u| u.get("output_tokens"))
                        .and_then(|v| v.as_u64())
                        .unwrap_or(0);
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_str_accepts_known_providers() {
        for s in ["ollama", "openai", "llamacpp", "anthropic", "gemini"] {
            assert!(Provider::from_str(s).is_ok());
        }
        assert!(Provider::from_str("openaai").is_err());
    }

    #[test]
    fn base_url_strips_known_suffixes() {
        assert_eq!(
            Provider::base_url("http://h:11434/api/generate"),
            "http://h:11434"
        );
        assert_eq!(
            Provider::base_url("http://h:8000/v1/chat/completions"),
            "http://h:8000"
        );
        assert_eq!(
            Provider::base_url("http://h:8080/completion/"),
            "http://h:8080"
        );
        assert_eq!(
            Provider::base_url("https://api.anthropic.com/v1/messages"),
            "https://api.anthropic.com"
        );
        assert_eq!(
            Provider::base_url("https://api.openai.com/v1"),
            "https://api.openai.com"
        );
    }

    #[test]
    fn base_url_strips_generate_content_endpoint() {
        assert_eq!(
            Provider::base_url(
                "https://generativelanguage.googleapis.com/v1beta/models/gemini-2.0-flash:generateContent?key=abc"
            ),
            "https://generativelanguage.googleapis.com"
        );
        assert_eq!(
            Provider::base_url("https://generativelanguage.googleapis.com/v1beta"),
            "https://generativelanguage.googleapis.com"
        );
    }

    #[test]
    fn base_url_fallback_strips_last_segment() {
        assert_eq!(Provider::base_url("http://host/api/v1"), "http://host/api");
        assert_eq!(Provider::base_url("http://host/llm"), "http://host");
        assert_eq!(Provider::base_url("http://host:8080"), "http://host:8080");
        assert_eq!(Provider::base_url("http://host"), "http://host");
    }

    #[test]
    fn parse_response_llamacpp_native() {
        let body = json!({
            "content": "Some note text",
            "timings": {"predicted_n": 42}
        });
        let (text, tokens) = Provider::Llamacpp.parse_response(&body);
        assert_eq!(text, "Some note text");
        assert_eq!(tokens, "42");
    }

    #[test]
    fn parse_response_llamacpp_chat_fallback() {
        let body = json!({
            "choices": [{"message": {"content": "Chat text"}}]
        });
        let (text, _) = Provider::Llamacpp.parse_response(&body);
        assert_eq!(text, "Chat text");
    }

    #[test]
    fn parse_response_other_providers() {
        let ollama = json!({"message": {"content": "O"}, "eval_count": 5});
        assert_eq!(
            Provider::Ollama.parse_response(&ollama),
            ("O".to_string(), "5".to_string())
        );

        let openai =
            json!({"choices": [{"message": {"content": "A"}}], "usage": {"total_tokens": 3}});
        assert_eq!(
            Provider::OpenAI.parse_response(&openai),
            ("A".to_string(), "3".to_string())
        );

        let anthropic = json!({"content": [{"type": "text", "text": "N"}], "usage": {"input_tokens": 1, "output_tokens": 2}});
        assert_eq!(
            Provider::Anthropic.parse_response(&anthropic),
            ("N".to_string(), "3".to_string())
        );

        let gemini = json!({"candidates": [{"content": {"parts": [{"text": "G"}]}}], "usageMetadata": {"totalTokenCount": 7}});
        assert_eq!(
            Provider::Gemini.parse_response(&gemini),
            ("G".to_string(), "7".to_string())
        );
    }

    #[test]
    fn headers_gemini_uses_header_not_url() {
        let headers = Provider::Gemini.headers("secret-key");
        assert_eq!(headers, vec![("x-goog-api-key", "secret-key".to_string())]);
        let url = Provider::Gemini.generate_url(
            "https://generativelanguage.googleapis.com/v1beta",
            "gemini-2.0-flash",
            "secret-key",
        );
        assert!(!url.contains("key="));
    }

    #[test]
    fn build_chat_body_uses_max_tokens_and_temperature() {
        let body = Provider::Anthropic.build_chat_body(
            "claude",
            &json!([{"role": "user", "content": "hi"}]),
            8192,
            Some(0.5),
        );
        assert_eq!(body["max_tokens"], 8192);
        assert_eq!(body["temperature"], 0.5);
    }

    #[test]
    fn generate_body_sends_token_cap_per_provider() {
        let cases = [
            (Provider::OpenAI, "max_tokens", false),
            (Provider::Ollama, "num_predict", true),
            (Provider::Llamacpp, "n_predict", false),
            (Provider::Anthropic, "max_tokens", false),
            (Provider::Gemini, "maxOutputTokens", true),
        ];
        for (p, field, nested) in cases {
            let body = p.build_generate_body("m", "p", None, 6144, None);
            let found = if nested && matches!(p, Provider::Ollama) {
                body["options"][field] == 6144
            } else if nested {
                body["generationConfig"][field] == 6144
            } else {
                body[field] == 6144
            };
            assert!(found, "generate body for {:?} missing {}", p, field);
        }
    }

    #[test]
    fn chat_body_sends_token_cap_per_provider() {
        let msgs = json!([{"role": "user", "content": "hi"}]);
        let cases = [
            (Provider::OpenAI, "max_tokens", false),
            (Provider::Ollama, "num_predict", true),
            (Provider::Llamacpp, "max_tokens", false),
            (Provider::Anthropic, "max_tokens", false),
            (Provider::Gemini, "maxOutputTokens", true),
        ];
        for (p, field, nested) in cases {
            let body = p.build_chat_body("m", &msgs, 4096, None);
            let found = if nested && matches!(p, Provider::Ollama) {
                body["options"][field] == 4096
            } else if nested {
                body["generationConfig"][field] == 4096
            } else {
                body[field] == 4096
            };
            assert!(found, "chat body for {:?} missing {}", p, field);
        }
    }

    #[test]
    fn ollama_temperature_goes_into_options() {
        let body = Provider::Ollama.build_generate_body("m", "p", None, 100, Some(0.7));
        assert_eq!(body["options"]["temperature"], 0.7f32 as f64);
        assert!(body.get("temperature").is_none());
        let chat = Provider::Ollama.build_chat_body(
            "m",
            &json!([{"role": "user", "content": "hi"}]),
            100,
            Some(0.3),
        );
        assert_eq!(chat["options"]["temperature"], 0.3f32 as f64);
    }

    #[test]
    fn gemini_cap_and_temperature_in_generation_config() {
        let body = Provider::Gemini.build_generate_body("m", "p", None, 8192, Some(0.9));
        assert_eq!(body["generationConfig"]["maxOutputTokens"], 8192);
        assert_eq!(body["generationConfig"]["temperature"], 0.9f32 as f64);
    }
}
