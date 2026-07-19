use std::{sync::OnceLock, time::Duration};

use base64::{engine::general_purpose::STANDARD, Engine};
use reqwest::{
    header::{HeaderMap, HeaderValue, AUTHORIZATION, CONTENT_TYPE},
    Client,
};
use serde_json::{json, Value};

use crate::{
    error::{AppError, AppResult},
    models::{ModelRecord, ProviderStatusRecord},
    storage::ProviderSecrets,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderKind {
    OpenAI,
    OpenRouter,
    Gemini,
    Claude,
    Mistral,
}

impl ProviderKind {
    pub const ALL: [ProviderKind; 5] = [
        ProviderKind::OpenAI,
        ProviderKind::OpenRouter,
        ProviderKind::Gemini,
        ProviderKind::Claude,
        ProviderKind::Mistral,
    ];

    pub fn id(self) -> &'static str {
        match self {
            ProviderKind::OpenAI => "openai",
            ProviderKind::OpenRouter => "openrouter",
            ProviderKind::Gemini => "gemini",
            ProviderKind::Claude => "claude",
            ProviderKind::Mistral => "mistral",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            ProviderKind::OpenAI => "OpenAI",
            ProviderKind::OpenRouter => "OpenRouter",
            ProviderKind::Gemini => "Gemini",
            ProviderKind::Claude => "Claude",
            ProviderKind::Mistral => "Mistral",
        }
    }

    pub fn default_model(self) -> &'static str {
        match self {
            ProviderKind::OpenAI => "gpt-4o",
            ProviderKind::OpenRouter => "openrouter/auto",
            ProviderKind::Gemini => "gemini-2.5-flash",
            ProviderKind::Claude => "claude-3-5-sonnet-latest",
            ProviderKind::Mistral => "mistral-large-latest",
        }
    }

    pub fn endpoint(self) -> &'static str {
        match self {
            ProviderKind::OpenAI => "https://api.openai.com/v1/chat/completions",
            ProviderKind::OpenRouter => "https://openrouter.ai/api/v1/chat/completions",
            ProviderKind::Gemini => {
                "https://generativelanguage.googleapis.com/v1beta/models/gemini-2.5-flash:generateContent"
            }
            ProviderKind::Claude => "https://api.anthropic.com/v1/messages",
            ProviderKind::Mistral => "https://api.mistral.ai/v1/chat/completions",
        }
    }

    pub fn model_record(self, enabled: bool, has_key: bool) -> Option<ModelRecord> {
        if !(enabled && has_key) {
            return None;
        }
        Some(ModelRecord {
            id: format!("provider:{}", self.id()),
            name: format!(
                "{} · Provider default ({})",
                self.label(),
                self.default_model()
            ),
            path: format!("provider://{}", self.id()),
            kind: "chat".to_string(),
            size_bytes: 0,
            last_modified: None,
            available: true,
            favorite: false,
            missing: false,
        })
    }

    pub fn status_record(self, enabled: bool, has_key: bool) -> ProviderStatusRecord {
        ProviderStatusRecord {
            id: self.id().to_string(),
            label: self.label().to_string(),
            default_model_id: self.default_model().to_string(),
            has_key,
            available: enabled && has_key,
            note: if enabled && has_key {
                format!("Ready. Uses {} by default.", self.default_model())
            } else if has_key {
                format!(
                    "Saved locally. Enable remote providers to use {}.",
                    self.default_model()
                )
            } else {
                format!(
                    "No saved API key. Add a key to use {}.",
                    self.default_model()
                )
            },
        }
    }
}

static HTTP_CLIENT: OnceLock<Client> = OnceLock::new();

fn client() -> &'static Client {
    HTTP_CLIENT.get_or_init(Client::new)
}

fn provider_error(provider: ProviderKind, message: impl Into<String>) -> AppError {
    AppError::Message(format!(
        "{} provider error: {}",
        provider.label(),
        message.into()
    ))
}

fn header_value(value: &str) -> AppResult<HeaderValue> {
    HeaderValue::from_str(value)
        .map_err(|error| AppError::Message(format!("Invalid API key: {error}")))
}

fn text_from_parts(parts: &[Value]) -> Option<String> {
    let mut collected = String::new();
    for part in parts {
        if let Some(text) = text_from_value(part) {
            collected.push_str(&text);
        }
    }
    if collected.trim().is_empty() {
        None
    } else {
        Some(collected)
    }
}

fn text_from_value(value: &Value) -> Option<String> {
    match value {
        Value::String(text) => Some(text.clone()),
        Value::Array(items) => text_from_parts(items),
        Value::Object(map) => {
            if let Some(text) = map.get("text").and_then(Value::as_str) {
                return Some(text.to_string());
            }
            if let Some(parts) = map.get("parts").and_then(Value::as_array) {
                return text_from_parts(parts);
            }
            if let Some(content) = map.get("content") {
                return text_from_value(content);
            }
            if let Some(message) = map.get("message") {
                return text_from_value(message);
            }
            None
        }
        _ => None,
    }
}

fn extract_chat_completions_text(provider: ProviderKind, value: &Value) -> AppResult<String> {
    let choices = value
        .get("choices")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            AppError::Message(format!(
                "{} response did not include choices",
                provider.label()
            ))
        })?;
    let content = choices
        .first()
        .and_then(|choice| choice.get("message"))
        .and_then(|message| message.get("content"))
        .ok_or_else(|| {
            AppError::Message(format!(
                "{} response did not include message content",
                provider.label()
            ))
        })?;
    text_from_value(content).ok_or_else(|| provider_error(provider, "empty message content"))
}

fn extract_gemini_text(value: &Value) -> AppResult<String> {
    let candidates = value
        .get("candidates")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            AppError::Message("Gemini response did not include candidates".to_string())
        })?;
    let parts = candidates
        .first()
        .and_then(|candidate| candidate.get("content"))
        .and_then(|content| content.get("parts"))
        .and_then(Value::as_array)
        .ok_or_else(|| {
            AppError::Message("Gemini response did not include text parts".to_string())
        })?;
    text_from_parts(parts).ok_or_else(|| provider_error(ProviderKind::Gemini, "empty response"))
}

fn extract_anthropic_text(value: &Value) -> AppResult<String> {
    let content = value
        .get("content")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            AppError::Message("Anthropic response did not include content".to_string())
        })?;
    text_from_parts(content).ok_or_else(|| provider_error(ProviderKind::Claude, "empty response"))
}

async fn post_json(
    provider: ProviderKind,
    url: &str,
    headers: HeaderMap,
    body: Value,
) -> AppResult<Value> {
    let response = client()
        .post(url)
        .headers(headers)
        .json(&body)
        .send()
        .await
        .map_err(|error| provider_error(provider, format!("request failed: {error}")))?;

    let status = response.status();
    let raw = response
        .text()
        .await
        .map_err(|error| provider_error(provider, format!("failed to read response: {error}")))?;

    if !status.is_success() {
        return Err(provider_error(
            provider,
            format!("HTTP {}: {}", status, raw.trim()),
        ));
    }

    serde_json::from_str(&raw)
        .map_err(|error| provider_error(provider, format!("invalid JSON response: {error}")))
}

async fn get_json(provider: ProviderKind, url: &str, headers: HeaderMap) -> AppResult<Value> {
    let response = client()
        .get(url)
        .headers(headers)
        .send()
        .await
        .map_err(|error| provider_error(provider, format!("request failed: {error}")))?;

    let status = response.status();
    let raw = response
        .text()
        .await
        .map_err(|error| provider_error(provider, format!("failed to read response: {error}")))?;

    if !status.is_success() {
        return Err(provider_error(
            provider,
            format!("HTTP {}: {}", status, raw.trim()),
        ));
    }

    serde_json::from_str(&raw)
        .map_err(|error| provider_error(provider, format!("invalid JSON response: {error}")))
}

async fn get_bytes(provider: ProviderKind, url: &str, headers: HeaderMap) -> AppResult<Vec<u8>> {
    let response = client()
        .get(url)
        .headers(headers)
        .send()
        .await
        .map_err(|error| provider_error(provider, format!("request failed: {error}")))?;

    let status = response.status();
    let bytes = response
        .bytes()
        .await
        .map_err(|error| provider_error(provider, format!("failed to read response: {error}")))?;

    if !status.is_success() {
        let raw = String::from_utf8_lossy(&bytes);
        return Err(provider_error(
            provider,
            format!("HTTP {}: {}", status, raw.trim()),
        ));
    }

    Ok(bytes.to_vec())
}

fn provider_key(secrets: &ProviderSecrets, provider: ProviderKind) -> Option<&str> {
    secrets.key_for(provider)
}

pub fn provider_models(enabled: bool, secrets: &ProviderSecrets) -> Vec<ModelRecord> {
    ProviderKind::ALL
        .into_iter()
        .filter_map(|provider| {
            provider.model_record(enabled, provider_key(secrets, provider).is_some())
        })
        .collect()
}

pub fn provider_statuses(enabled: bool, secrets: &ProviderSecrets) -> Vec<ProviderStatusRecord> {
    ProviderKind::ALL
        .into_iter()
        .map(|provider| provider.status_record(enabled, provider_key(secrets, provider).is_some()))
        .collect()
}

pub fn preferred_provider_kind(secrets: &ProviderSecrets) -> Option<ProviderKind> {
    ProviderKind::ALL
        .into_iter()
        .find(|provider| provider_key(secrets, *provider).is_some())
}

/// Returns the first provider that can generate images and has a saved API key.
/// Currently only OpenAI is supported.
pub fn preferred_image_provider(secrets: &ProviderSecrets) -> Option<ProviderKind> {
    if provider_key(secrets, ProviderKind::OpenAI).is_some() {
        Some(ProviderKind::OpenAI)
    } else {
        None
    }
}

/// Returns the first provider that can generate videos and has a saved API key.
/// Currently only OpenAI is supported.
pub fn preferred_video_provider(secrets: &ProviderSecrets) -> Option<ProviderKind> {
    if provider_key(secrets, ProviderKind::OpenAI).is_some() {
        Some(ProviderKind::OpenAI)
    } else {
        None
    }
}

pub fn provider_from_model_id(model_id: &str) -> Option<ProviderKind> {
    let id = model_id.strip_prefix("provider:")?;
    match id {
        "openai" => Some(ProviderKind::OpenAI),
        "openrouter" => Some(ProviderKind::OpenRouter),
        "gemini" => Some(ProviderKind::Gemini),
        "claude" => Some(ProviderKind::Claude),
        "mistral" => Some(ProviderKind::Mistral),
        _ => None,
    }
}

pub async fn generate_provider_reply(
    provider: ProviderKind,
    secrets: &ProviderSecrets,
    system_prompt: &str,
    user_prompt: &str,
    max_tokens: u32,
    temperature: f32,
) -> AppResult<String> {
    let api_key = provider_key(secrets, provider)
        .ok_or_else(|| provider_error(provider, "no API key is saved"))?;

    match provider {
        ProviderKind::OpenAI | ProviderKind::OpenRouter => {
            let mut headers = HeaderMap::new();
            headers.insert(AUTHORIZATION, header_value(&format!("Bearer {api_key}"))?);
            headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
            if provider == ProviderKind::OpenRouter {
                headers.insert(
                    "HTTP-Referer",
                    HeaderValue::from_static("https://github.com/Kivitas/ai-chat"),
                );
                headers.insert("X-OpenRouter-Title", HeaderValue::from_static("AI Chat"));
            }
            let mut body = json!({
                "model": provider.default_model(),
                "messages": [
                    { "role": "system", "content": system_prompt },
                    { "role": "user", "content": user_prompt }
                ],
                "temperature": temperature,
                "max_tokens": max_tokens
            });
            if provider == ProviderKind::OpenAI {
                body["max_completion_tokens"] = json!(max_tokens);
                body["store"] = json!(false);
                if let Some(object) = body.as_object_mut() {
                    object.remove("max_tokens");
                }
            }
            let response = post_json(provider, provider.endpoint(), headers, body).await?;
            extract_chat_completions_text(provider, &response)
        }
        ProviderKind::Gemini => {
            let mut headers = HeaderMap::new();
            headers.insert("x-goog-api-key", header_value(api_key)?);
            headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
            let body = json!({
                "system_instruction": {
                    "parts": [
                        { "text": system_prompt }
                    ]
                },
                "contents": [
                    {
                        "role": "user",
                        "parts": [
                            { "text": user_prompt }
                        ]
                    }
                ],
                "generationConfig": {
                    "temperature": temperature,
                    "maxOutputTokens": max_tokens
                }
            });
            let response = post_json(provider, provider.endpoint(), headers, body).await?;
            extract_gemini_text(&response)
        }
        ProviderKind::Claude => {
            let mut headers = HeaderMap::new();
            headers.insert("x-api-key", header_value(api_key)?);
            headers.insert("anthropic-version", HeaderValue::from_static("2023-06-01"));
            headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
            let body = json!({
                "model": provider.default_model(),
                "max_tokens": max_tokens,
                "temperature": temperature,
                "system": system_prompt,
                "messages": [
                    { "role": "user", "content": user_prompt }
                ]
            });
            let response = post_json(provider, provider.endpoint(), headers, body).await?;
            extract_anthropic_text(&response)
        }
        ProviderKind::Mistral => {
            let mut headers = HeaderMap::new();
            headers.insert(AUTHORIZATION, header_value(&format!("Bearer {api_key}"))?);
            headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
            let body = json!({
                "model": provider.default_model(),
                "messages": [
                    { "role": "system", "content": system_prompt },
                    { "role": "user", "content": user_prompt }
                ],
                "temperature": temperature,
                "max_tokens": max_tokens
            });
            let response = post_json(provider, provider.endpoint(), headers, body).await?;
            extract_chat_completions_text(provider, &response)
        }
    }
}

/// Drain complete SSE `data:` lines from `buf` into `text`, calling `on_partial`
/// with the growing accumulated reply after each new delta token.
fn drain_sse_lines(
    buf: &mut String,
    text: &mut String,
    extract: impl Fn(&Value) -> Option<String>,
    on_partial: &impl Fn(String),
) {
    while let Some(nl) = buf.find('\n') {
        let line = buf[..nl].trim_end_matches('\r').to_string();
        *buf = buf[nl + 1..].to_string();
        if line.is_empty() || line == "data: [DONE]" {
            continue;
        }
        let Some(data) = line.strip_prefix("data: ") else {
            continue;
        };
        let Ok(value) = serde_json::from_str::<Value>(data) else {
            continue;
        };
        if let Some(delta) = extract(&value) {
            text.push_str(&delta);
            if !text.trim().is_empty() {
                on_partial(text.clone());
            }
        }
    }
}

/// Streaming variant of `generate_provider_reply`.  Emits SSE delta tokens via
/// `on_partial` so the UI can display tokens as they arrive, matching the
/// behaviour of the local GGUF runtime.  All four providers are supported.
pub async fn generate_provider_reply_streaming<F: Fn(String)>(
    provider: ProviderKind,
    secrets: &ProviderSecrets,
    system_prompt: &str,
    user_prompt: &str,
    max_tokens: u32,
    temperature: f32,
    on_partial: F,
) -> AppResult<String> {
    let api_key = provider_key(secrets, provider)
        .ok_or_else(|| provider_error(provider, "no API key is saved"))?;

    match provider {
        // ── OpenAI-compatible SSE (OpenAI, OpenRouter, Mistral) ──────────────
        ProviderKind::OpenAI | ProviderKind::OpenRouter | ProviderKind::Mistral => {
            let mut headers = HeaderMap::new();
            headers.insert(AUTHORIZATION, header_value(&format!("Bearer {api_key}"))?);
            headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
            if provider == ProviderKind::OpenRouter {
                headers.insert(
                    "HTTP-Referer",
                    HeaderValue::from_static("https://github.com/Kivitas/ai-chat"),
                );
                headers.insert("X-OpenRouter-Title", HeaderValue::from_static("AI Chat"));
            }
            let mut body = json!({
                "model": provider.default_model(),
                "messages": [
                    { "role": "system", "content": system_prompt },
                    { "role": "user", "content": user_prompt }
                ],
                "temperature": temperature,
                "max_tokens": max_tokens,
                "stream": true
            });
            if provider == ProviderKind::OpenAI {
                body["max_completion_tokens"] = json!(max_tokens);
                body["store"] = json!(false);
                if let Some(object) = body.as_object_mut() {
                    object.remove("max_tokens");
                }
            }
            let mut response = client()
                .post(provider.endpoint())
                .headers(headers)
                .json(&body)
                .send()
                .await
                .map_err(|e| provider_error(provider, format!("request failed: {e}")))?;
            let status = response.status();
            if !status.is_success() {
                let raw = response.bytes().await.unwrap_or_default();
                return Err(provider_error(
                    provider,
                    format!("HTTP {}: {}", status, String::from_utf8_lossy(&raw).trim()),
                ));
            }
            let mut buf = String::new();
            let mut partial_text = String::new();
            while let Some(chunk) = response
                .chunk()
                .await
                .map_err(|e| provider_error(provider, format!("stream read error: {e}")))?
            {
                buf.push_str(&String::from_utf8_lossy(&chunk));
                drain_sse_lines(
                    &mut buf,
                    &mut partial_text,
                    |v| {
                        v.get("choices")
                            .and_then(Value::as_array)
                            .and_then(|c| c.first())
                            .and_then(|c| c.get("delta"))
                            .and_then(|d| d.get("content"))
                            .and_then(Value::as_str)
                            .map(String::from)
                    },
                    &on_partial,
                );
            }
            if partial_text.trim().is_empty() {
                Err(provider_error(provider, "empty streaming response"))
            } else {
                Ok(partial_text)
            }
        }

        // ── Gemini streamGenerateContent + alt=sse ───────────────────────────
        ProviderKind::Gemini => {
            let mut headers = HeaderMap::new();
            headers.insert("x-goog-api-key", header_value(api_key)?);
            headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
            let body = json!({
                "system_instruction": { "parts": [{ "text": system_prompt }] },
                "contents": [{ "role": "user", "parts": [{ "text": user_prompt }] }],
                "generationConfig": { "temperature": temperature, "maxOutputTokens": max_tokens }
            });
            let url = format!(
                "https://generativelanguage.googleapis.com/v1beta/models/{}:streamGenerateContent?alt=sse",
                provider.default_model()
            );
            let mut response = client()
                .post(&url)
                .headers(headers)
                .json(&body)
                .send()
                .await
                .map_err(|e| provider_error(provider, format!("request failed: {e}")))?;
            let status = response.status();
            if !status.is_success() {
                let raw = response.bytes().await.unwrap_or_default();
                return Err(provider_error(
                    provider,
                    format!("HTTP {}: {}", status, String::from_utf8_lossy(&raw).trim()),
                ));
            }
            let mut buf = String::new();
            let mut partial_text = String::new();
            while let Some(chunk) = response
                .chunk()
                .await
                .map_err(|e| provider_error(provider, format!("stream read error: {e}")))?
            {
                buf.push_str(&String::from_utf8_lossy(&chunk));
                drain_sse_lines(
                    &mut buf,
                    &mut partial_text,
                    |v| {
                        v.get("candidates")
                            .and_then(Value::as_array)
                            .and_then(|c| c.first())
                            .and_then(|c| c.get("content"))
                            .and_then(|c| c.get("parts"))
                            .and_then(Value::as_array)
                            .and_then(|p| p.first())
                            .and_then(|p| p.get("text"))
                            .and_then(Value::as_str)
                            .map(String::from)
                    },
                    &on_partial,
                );
            }
            if partial_text.trim().is_empty() {
                Err(provider_error(provider, "empty streaming response"))
            } else {
                Ok(partial_text)
            }
        }

        // ── Anthropic content_block_delta SSE ────────────────────────────────
        ProviderKind::Claude => {
            let mut headers = HeaderMap::new();
            headers.insert("x-api-key", header_value(api_key)?);
            headers.insert("anthropic-version", HeaderValue::from_static("2023-06-01"));
            headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
            let body = json!({
                "model": provider.default_model(),
                "max_tokens": max_tokens,
                "temperature": temperature,
                "system": system_prompt,
                "messages": [{ "role": "user", "content": user_prompt }],
                "stream": true
            });
            let mut response = client()
                .post(provider.endpoint())
                .headers(headers)
                .json(&body)
                .send()
                .await
                .map_err(|e| provider_error(provider, format!("request failed: {e}")))?;
            let status = response.status();
            if !status.is_success() {
                let raw = response.bytes().await.unwrap_or_default();
                return Err(provider_error(
                    provider,
                    format!("HTTP {}: {}", status, String::from_utf8_lossy(&raw).trim()),
                ));
            }
            let mut buf = String::new();
            let mut partial_text = String::new();
            while let Some(chunk) = response
                .chunk()
                .await
                .map_err(|e| provider_error(provider, format!("stream read error: {e}")))?
            {
                buf.push_str(&String::from_utf8_lossy(&chunk));
                drain_sse_lines(
                    &mut buf,
                    &mut partial_text,
                    |v| {
                        if v.get("type").and_then(Value::as_str) == Some("content_block_delta") {
                            v.get("delta")
                                .and_then(|d| d.get("text"))
                                .and_then(Value::as_str)
                                .map(String::from)
                        } else {
                            None
                        }
                    },
                    &on_partial,
                );
            }
            if partial_text.trim().is_empty() {
                Err(provider_error(provider, "empty streaming response"))
            } else {
                Ok(partial_text)
            }
        }
    }
}

/// Generate an image via a cloud provider API and return the raw image bytes.
/// Currently supports OpenAI GPT Image only; all other providers return an error.
pub async fn generate_provider_image(
    provider: ProviderKind,
    secrets: &ProviderSecrets,
    prompt: &str,
    width: u32,
    height: u32,
) -> AppResult<Vec<u8>> {
    match provider {
        ProviderKind::OpenAI => {
            let api_key = provider_key(secrets, provider)
                .ok_or_else(|| provider_error(provider, "no API key is saved"))?;
            let mut headers = HeaderMap::new();
            headers.insert(AUTHORIZATION, header_value(&format!("Bearer {api_key}"))?);
            headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
            let size = if width > height {
                "1536x1024"
            } else if height > width {
                "1024x1536"
            } else {
                "1024x1024"
            };
            let body = serde_json::json!({
                "model": "gpt-image-1",
                "prompt": prompt,
                "n": 1,
                "size": size,
                "quality": "medium",
                "output_format": "png"
            });
            let response = post_json(
                provider,
                "https://api.openai.com/v1/images/generations",
                headers,
                body,
            )
            .await?;
            if let Some(b64) = response
                .get("data")
                .and_then(|d| d.as_array())
                .and_then(|arr| arr.first())
                .and_then(|item| item.get("b64_json"))
                .and_then(|v| v.as_str())
            {
                return STANDARD.decode(b64).map_err(|error| {
                    provider_error(provider, format!("failed to decode image bytes: {error}"))
                });
            }

            let url = response
                .get("data")
                .and_then(|d| d.as_array())
                .and_then(|arr| arr.first())
                .and_then(|item| item.get("url"))
                .and_then(|v| v.as_str())
                .ok_or_else(|| provider_error(provider, "response did not include image data"))?;
            get_bytes(provider, url, HeaderMap::new()).await
        }
        _ => Err(provider_error(
            provider,
            "image generation is not supported for this provider",
        )),
    }
}

/// Generate a video via a cloud provider API and return the raw MP4 bytes.
/// Currently supports OpenAI Sora only; all other providers return an error.
pub async fn generate_provider_video<F>(
    provider: ProviderKind,
    secrets: &ProviderSecrets,
    prompt: &str,
    mut on_progress: F,
) -> AppResult<Vec<u8>>
where
    F: FnMut(String, Option<u64>),
{
    match provider {
        ProviderKind::OpenAI => {
            let api_key = provider_key(secrets, provider)
                .ok_or_else(|| provider_error(provider, "no API key is saved"))?;
            let mut headers = HeaderMap::new();
            headers.insert(AUTHORIZATION, header_value(&format!("Bearer {api_key}"))?);
            headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));

            let body = serde_json::json!({
                "model": "sora-2",
                "prompt": prompt,
                "size": "1280x720",
                "seconds": "4"
            });
            let mut video = post_json(
                provider,
                "https://api.openai.com/v1/videos",
                headers.clone(),
                body,
            )
            .await?;
            let video_id = video
                .get("id")
                .and_then(Value::as_str)
                .ok_or_else(|| provider_error(provider, "response did not include video id"))?
                .to_string();
            let mut status = video
                .get("status")
                .and_then(Value::as_str)
                .unwrap_or("queued")
                .to_string();
            let mut progress = video
                .get("progress")
                .and_then(Value::as_f64)
                .map(|value| value.round().clamp(0.0, 100.0) as u64);
            on_progress(status.clone(), progress);

            for _ in 0..180 {
                if status != "queued" && status != "in_progress" {
                    break;
                }
                tokio::time::sleep(Duration::from_secs(5)).await;
                video = get_json(
                    provider,
                    &format!("https://api.openai.com/v1/videos/{video_id}"),
                    headers.clone(),
                )
                .await?;
                status = video
                    .get("status")
                    .and_then(Value::as_str)
                    .unwrap_or("in_progress")
                    .to_string();
                progress = video
                    .get("progress")
                    .and_then(Value::as_f64)
                    .map(|value| value.round().clamp(0.0, 100.0) as u64);
                on_progress(status.clone(), progress);
            }

            if status == "completed" {
                return get_bytes(
                    provider,
                    &format!("https://api.openai.com/v1/videos/{video_id}/content?variant=video"),
                    headers,
                )
                .await;
            }

            if status == "failed" {
                let message = video
                    .get("error")
                    .and_then(|error| error.get("message"))
                    .and_then(Value::as_str)
                    .unwrap_or("video generation failed");
                return Err(provider_error(provider, message));
            }

            Err(provider_error(
                provider,
                "video generation timed out before completion",
            ))
        }
        _ => Err(provider_error(
            provider,
            "video generation is not supported for this provider",
        )),
    }
}
