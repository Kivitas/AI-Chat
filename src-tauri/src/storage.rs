use std::{
    collections::HashSet,
    fs,
    path::{Path, PathBuf},
};

use base64::{engine::general_purpose::STANDARD, Engine};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;
use walkdir::WalkDir;

use crate::{
    crypto::{
        decrypt_bytes, default_security_file, encrypt_bytes, unlock_master_key, SecurityFile,
    },
    error::{AppError, AppResult},
    models::{
        CharacterRecord, ChatRecord, ChatSettings, DiagnosticsReport, MediaPromptPreview,
        ModelRecord, ProfileRecord, RuntimeRecord,
    },
    portable::{folder_size, read_json, to_iso_time, write_json, ResolvedPaths},
    providers::ProviderKind,
};

const ATTACHMENT_PREVIEW_CHAR_LIMIT: usize = 12_000;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderSecrets {
    pub openai_key: Option<String>,
    pub openrouter_key: Option<String>,
    pub gemini_key: Option<String>,
    pub claude_key: Option<String>,
    pub mistral_key: Option<String>,
}

impl ProviderSecrets {
    pub fn key_for(&self, provider: ProviderKind) -> Option<&str> {
        match provider {
            ProviderKind::OpenAI => self.openai_key.as_deref(),
            ProviderKind::OpenRouter => self.openrouter_key.as_deref(),
            ProviderKind::Gemini => self.gemini_key.as_deref(),
            ProviderKind::Claude => self.claude_key.as_deref(),
            ProviderKind::Mistral => self.mistral_key.as_deref(),
        }
    }

    pub fn set_key(&mut self, provider: ProviderKind, value: Option<String>) {
        match provider {
            ProviderKind::OpenAI => self.openai_key = value,
            ProviderKind::OpenRouter => self.openrouter_key = value,
            ProviderKind::Gemini => self.gemini_key = value,
            ProviderKind::Claude => self.claude_key = value,
            ProviderKind::Mistral => self.mistral_key = value,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.openai_key.is_none()
            && self.openrouter_key.is_none()
            && self.gemini_key.is_none()
            && self.claude_key.is_none()
            && self.mistral_key.is_none()
    }
}

pub struct AppStorage {
    pub resolved: ResolvedPaths,
}

impl AppStorage {
    pub fn new(resolved: ResolvedPaths) -> Self {
        Self { resolved }
    }

    fn account_slug(username: &str) -> AppResult<String> {
        let slug = username
            .trim()
            .to_ascii_lowercase()
            .chars()
            .map(|character| {
                if character.is_ascii_alphanumeric() || character == '-' || character == '_' {
                    character
                } else {
                    '-'
                }
            })
            .collect::<String>()
            .trim_matches('-')
            .to_string();
        if slug.is_empty() {
            return Err(AppError::Message("Username is required".to_string()));
        }
        Ok(slug)
    }

    fn account_config_dir(&self, username: &str) -> AppResult<PathBuf> {
        Ok(self
            .resolved
            .config_dir
            .join("Accounts")
            .join(Self::account_slug(username)?))
    }

    fn account_chats_dir(&self, username: &str) -> AppResult<PathBuf> {
        Ok(self.resolved.chats_dir.join(Self::account_slug(username)?))
    }

    pub fn security_path(&self, username: &str) -> AppResult<PathBuf> {
        Ok(self.account_config_dir(username)?.join("security.json"))
    }

    pub fn profile_path(&self, username: &str) -> AppResult<PathBuf> {
        Ok(self
            .resolved
            .profiles_dir
            .join(format!("{}.profile.json", Self::account_slug(username)?)))
    }

    pub fn character_path(&self, username: &str) -> AppResult<PathBuf> {
        Ok(self
            .resolved
            .characters_dir
            .join(format!("{}.character.json", Self::account_slug(username)?)))
    }

    pub fn provider_secrets_path(&self, username: &str) -> AppResult<PathBuf> {
        Ok(self
            .account_config_dir(username)?
            .join("provider-secrets.json"))
    }

    pub fn save_security(&self, username: &str, password: &str) -> AppResult<Vec<u8>> {
        if self.security_path(username)?.exists() {
            return Err(AppError::Message(
                "That username already exists. Choose another username or sign in.".to_string(),
            ));
        }
        let (security_file, master_key) = default_security_file(password)?;
        let path = self.security_path(username)?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        write_json(&path, &security_file)?;
        Ok(master_key)
    }

    pub fn unlock_security(&self, username: &str, password: &str) -> AppResult<Vec<u8>> {
        let path = self.security_path(username)?;
        if !path.exists() {
            return Err(AppError::Message("Account not found".to_string()));
        }
        let security: SecurityFile = read_json(&path)?;
        unlock_master_key(password, &security)
    }

    pub fn has_accounts(&self) -> bool {
        self.resolved
            .config_dir
            .join("Accounts")
            .read_dir()
            .ok()
            .into_iter()
            .flat_map(|entries| entries.filter_map(Result::ok))
            .any(|entry| entry.path().join("security.json").exists())
            || self.resolved.config_dir.join("security.json").exists()
    }

    pub fn save_profile(&self, username: &str, profile: &ProfileRecord) -> AppResult<()> {
        write_json(&self.profile_path(username)?, profile)
    }

    pub fn save_character(&self, username: &str, character: &CharacterRecord) -> AppResult<()> {
        write_json(&self.character_path(username)?, character)
    }

    pub fn load_provider_secrets(
        &self,
        username: &str,
        master_key: &[u8],
    ) -> AppResult<ProviderSecrets> {
        let path = self.provider_secrets_path(username)?;
        if !path.exists() {
            return Ok(ProviderSecrets::default());
        }
        let raw: serde_json::Value = read_json(&path)?;
        let nonce = raw
            .get("nonce")
            .and_then(|value| value.as_str())
            .ok_or_else(|| {
                AppError::Message("Missing nonce in provider secret file".to_string())
            })?;
        let ciphertext = raw
            .get("ciphertext")
            .and_then(|value| value.as_str())
            .ok_or_else(|| {
                AppError::Message("Missing ciphertext in provider secret file".to_string())
            })?;
        let plaintext = decrypt_bytes(master_key, nonce, ciphertext)?;
        Ok(serde_json::from_slice(&plaintext)?)
    }

    pub fn save_provider_secrets(
        &self,
        username: &str,
        master_key: &[u8],
        secrets: &ProviderSecrets,
    ) -> AppResult<()> {
        let path = self.provider_secrets_path(username)?;
        if secrets.is_empty() {
            if path.exists() {
                fs::remove_file(path)?;
            }
            return Ok(());
        }

        let plaintext = serde_json::to_vec(secrets)?;
        let (ciphertext, nonce) = encrypt_bytes(master_key, &plaintext)?;
        let payload = serde_json::json!({
            "nonce": nonce,
            "ciphertext": ciphertext,
        });
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(path, serde_json::to_vec_pretty(&payload)?)?;
        Ok(())
    }

    pub fn load_profile(&self, username: Option<&str>) -> AppResult<Option<ProfileRecord>> {
        if let Some(username) = username {
            let path = self.profile_path(username)?;
            if path.exists() {
                return Ok(Some(read_json(&path)?));
            }
        }
        if self
            .resolved
            .profiles_dir
            .join("active-profile.json")
            .exists()
        {
            Ok(Some(read_json(
                &self.resolved.profiles_dir.join("active-profile.json"),
            )?))
        } else {
            Ok(None)
        }
    }

    pub fn load_character(&self, username: Option<&str>) -> AppResult<Option<CharacterRecord>> {
        if let Some(username) = username {
            let path = self.character_path(username)?;
            if path.exists() {
                return Ok(Some(read_json(&path)?));
            }
        }
        if self
            .resolved
            .characters_dir
            .join("active-character.json")
            .exists()
        {
            Ok(Some(read_json(
                &self.resolved.characters_dir.join("active-character.json"),
            )?))
        } else {
            Ok(None)
        }
    }

    pub fn persist_chat(
        &self,
        username: &str,
        master_key: &[u8],
        chat: &ChatRecord,
    ) -> AppResult<()> {
        let plaintext = serde_json::to_vec(chat)?;
        let (ciphertext, nonce) = encrypt_bytes(master_key, &plaintext)?;
        let payload = serde_json::json!({
            "nonce": nonce,
            "ciphertext": ciphertext,
        });
        let chats_dir = self.account_chats_dir(username)?;
        fs::create_dir_all(&chats_dir)?;
        fs::write(
            chats_dir.join(format!("{}.chat.enc", chat.id)),
            serde_json::to_vec_pretty(&payload)?,
        )?;
        Ok(())
    }

    pub fn delete_chat(&self, username: &str, chat_id: &str) -> AppResult<()> {
        let chats_dir = self.account_chats_dir(username)?;
        let path = chats_dir.join(format!("{chat_id}.chat.enc"));
        if path.exists() {
            fs::remove_file(path)?;
        }
        Ok(())
    }

    pub fn load_chats(&self, username: &str, master_key: &[u8]) -> AppResult<Vec<ChatRecord>> {
        let mut chats: Vec<ChatRecord> = Vec::new();
        let chats_dir = self.account_chats_dir(username)?;
        fs::create_dir_all(&chats_dir)?;
        for entry in fs::read_dir(&chats_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().and_then(|ext| ext.to_str()) != Some("enc") {
                continue;
            }
            let raw: serde_json::Value = serde_json::from_slice(&fs::read(&path)?)?;
            let nonce = raw
                .get("nonce")
                .and_then(|value| value.as_str())
                .ok_or_else(|| AppError::Message("Missing nonce in chat file".to_string()))?;
            let ciphertext = raw
                .get("ciphertext")
                .and_then(|value| value.as_str())
                .ok_or_else(|| AppError::Message("Missing ciphertext in chat file".to_string()))?;
            let plaintext = decrypt_bytes(master_key, nonce, ciphertext)?;
            chats.push(serde_json::from_slice(&plaintext)?);
        }
        chats.sort_by(|left, right| right.updated_at.cmp(&left.updated_at));
        Ok(chats)
    }

    pub fn import_avatar(&self, base64: &str, prefix: &str) -> AppResult<String> {
        self.write_media_blob(base64, &self.resolved.images_dir, prefix, "png")
    }

    pub fn import_media_attachment(
        &self,
        base64: &str,
        file_name: &str,
        kind: &str,
    ) -> AppResult<String> {
        let extension = Path::new(file_name)
            .extension()
            .and_then(|ext| ext.to_str())
            .unwrap_or_else(|| {
                if kind == "video" {
                    "webm"
                } else if kind == "file" {
                    "bin"
                } else {
                    "png"
                }
            });
        let target_dir = if kind == "video" {
            self.resolved.videos_dir.clone()
        } else if kind == "file" {
            self.resolved.media_dir.join("Files")
        } else {
            self.resolved.images_dir.clone()
        };
        self.write_media_blob(base64, &target_dir, "attachment", extension)
    }

    fn write_media_blob(
        &self,
        base64: &str,
        target_dir: &Path,
        prefix: &str,
        extension: &str,
    ) -> AppResult<String> {
        let bytes = STANDARD.decode(base64)?;
        let file_name = format!("{prefix}-{}.{}", Uuid::new_v4(), extension);
        let path = target_dir.join(file_name);
        fs::create_dir_all(target_dir)?;
        fs::write(&path, bytes)?;
        Ok(path.to_string_lossy().to_string())
    }

    pub fn import_generated_media_file(
        &self,
        source_path: &Path,
        kind: &str,
        prefix: &str,
    ) -> AppResult<String> {
        let target_dir = if kind == "video" {
            self.resolved.videos_dir.clone()
        } else {
            self.resolved.images_dir.clone()
        };
        let extension = source_path
            .extension()
            .and_then(|ext| ext.to_str())
            .unwrap_or_else(|| if kind == "video" { "webm" } else { "png" });
        let file_name = format!("{prefix}-{}.{}", Uuid::new_v4(), extension);
        let destination = target_dir.join(file_name);
        fs::create_dir_all(&target_dir)?;
        fs::copy(source_path, &destination)?;
        Ok(destination.to_string_lossy().to_string())
    }

    pub fn build_attachment_preview(
        &self,
        path: &str,
        file_name: &str,
        kind: &str,
    ) -> Option<String> {
        if kind != "file" {
            return None;
        }

        let path = Path::new(path);
        let extension = path
            .extension()
            .and_then(|ext| ext.to_str())
            .unwrap_or_default()
            .to_ascii_lowercase();

        let extracted = if extension == "pdf" {
            pdf_extract::extract_text(path).ok()
        } else if is_text_like_extension(&extension) {
            fs::read(path)
                .ok()
                .map(|bytes| String::from_utf8_lossy(&bytes).into_owned())
                .map(|text| {
                    if matches!(extension.as_str(), "html" | "htm" | "xml" | "svg") {
                        strip_markup(&text)
                    } else {
                        text
                    }
                })
        } else {
            None
        }?;

        let normalized = normalize_attachment_text(&extracted);
        if normalized.is_empty() {
            return None;
        }

        let clipped = clip_chars(&normalized, ATTACHMENT_PREVIEW_CHAR_LIMIT);
        Some(format!("Extracted text from {file_name}:\n{clipped}"))
    }

    pub fn scan_models(&self) -> AppResult<(Vec<ModelRecord>, Vec<ModelRecord>, Vec<ModelRecord>)> {
        let chat_model_dirs = model_search_dirs(
            &self.resolved.models_dir,
            [
                self.resolved.data_dir.join("Models"),
                self.resolved.app_root.join("Models"),
            ],
        );
        Ok((
            scan_model_dirs(&chat_model_dirs, "chat", &["gguf"])?,
            scan_model_dir(
                &self.resolved.image_models_dir,
                "image",
                &["gguf", "safetensors", "ckpt", "onnx", "pt", "pth"],
            )?,
            scan_model_dir(
                &self.resolved.video_models_dir,
                "video",
                &[
                    "gguf",
                    "safetensors",
                    "ckpt",
                    "onnx",
                    "pt",
                    "pth",
                    "mp4",
                    "webm",
                ],
            )?,
        ))
    }

    pub fn scan_runtimes(&self) -> AppResult<Vec<RuntimeRecord>> {
        let runtimes = [
            ("chat", self.resolved.runtimes_dir.join("Chat")),
            ("image", self.resolved.runtimes_dir.join("Image")),
            ("video", self.resolved.runtimes_dir.join("Video")),
        ];

        let mut records = Vec::new();
        for (kind, path) in runtimes {
            let available = runtime_executable_exists(kind, &path);
            let has_external_chat_runtime = kind == "chat" && available;
            records.push(RuntimeRecord {
                id: format!("{kind}-runtime"),
                kind: kind.to_string(),
                name: format!("{} runtime", capitalize(kind)),
                path: path.to_string_lossy().to_string(),
                available,
                note: if has_external_chat_runtime {
                    "Compatible isolated chat runtime detected. GGUF replies can run safely.".to_string()
                } else if kind == "chat" {
                    "Add llama-cli.exe here to run GGUF models. In-process GGUF is disabled to prevent app closes.".to_string()
                } else if available {
                    if kind == "image" {
                        "Compatible local image runtime detected. The executable should read the media job JSON or AI_CHAT_* environment variables and write an image file into the output directory.".to_string()
                    } else {
                        "Compatible local video runtime detected. The executable should read the media job JSON or AI_CHAT_* environment variables and write a video file into the output directory.".to_string()
                    }
                } else {
                    if kind == "image" {
                        "Add a compatible image runtime executable here. The app will pass a job JSON and output directory, then import the generated file back into Data/Media/Images.".to_string()
                    } else {
                        "Add a compatible video runtime executable here. The app will pass a job JSON and output directory, then import the generated file back into Data/Media/Videos.".to_string()
                    }
                },
            });
        }
        Ok(records)
    }

    pub fn diagnostics(
        &self,
        models: &[ModelRecord],
        runtimes: &[RuntimeRecord],
        repo_limit_mb: u64,
    ) -> DiagnosticsReport {
        let missing = models
            .iter()
            .filter(|model| model.missing)
            .map(|model| model.path.clone())
            .collect();
        DiagnosticsReport {
            app_root: self.resolved.app_root.to_string_lossy().to_string(),
            data_dir: self.resolved.data_dir.to_string_lossy().to_string(),
            runtimes_dir: self.resolved.runtimes_dir.to_string_lossy().to_string(),
            detected_models: models.to_vec(),
            missing_model_paths: missing,
            runtime_availability: runtimes.to_vec(),
            disk_usage_bytes: folder_size(&self.resolved.data_dir),
            repo_oversize_warnings: repo_oversize_warnings(&self.resolved.app_root, repo_limit_mb),
        }
    }

    pub fn create_initial_chat(
        &self,
        username: &str,
        master_key: &[u8],
        character_name: &str,
    ) -> AppResult<ChatRecord> {
        let now = Utc::now().to_rfc3339();
        let chat = ChatRecord {
            id: Uuid::new_v4().to_string(),
            title: "Offline chat".to_string(),
            created_at: now.clone(),
            updated_at: now.clone(),
            favorite: false,
            archived: false,
            settings: ChatSettings {
                model_id: None,
                image_model_id: None,
                video_model_id: None,
                profile_override_id: None,
                user_name: Some("You".to_string()),
                ai_name: Some(character_name.to_string()),
                ai_instructions: Some(
                    "Use clean Markdown, show derivations step by step with LaTeX, and use fenced code blocks with language labels.".to_string(),
                ),
                user_avatar: None,
                ai_avatar: None,
                context_folder_path: None,
            },
            messages: Vec::new(),
        };
        self.persist_chat(username, master_key, &chat)?;
        Ok(chat)
    }

    pub fn build_media_prompt(
        &self,
        chat: &ChatRecord,
        character: Option<&CharacterRecord>,
        user_prompt: &str,
    ) -> MediaPromptPreview {
        let recent_messages = chat
            .messages
            .iter()
            .rev()
            .take(4)
            .map(|message| format!("{}: {}", message.role, message.text))
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect::<Vec<_>>()
            .join("\n");
        let file_context = chat
            .messages
            .iter()
            .flat_map(|message| message.attachments.iter())
            .map(|asset| {
                let mut parts = vec![format!(
                    "{}: {} ({})",
                    asset.kind,
                    asset.source_label.as_deref().unwrap_or("uploaded file"),
                    asset.path
                )];
                if let Some(preview) = asset.content_preview.as_deref() {
                    parts.push(preview.to_string());
                }
                parts.join("\n")
            })
            .collect::<Vec<_>>()
            .join("\n");

        MediaPromptPreview {
            prompt: format!(
                "Chat user name: {}\nChat AI name: {}\nCharacter profile: {}\nChat preset instructions: {}\nRecent messages:\n{}\nUploaded file, image, and video context:\n{}\nExplicit request:\n{}",
                chat.settings.user_name.as_deref().unwrap_or("User"),
                chat.settings.ai_name.as_deref().unwrap_or_else(|| character.map(|value| value.name.as_str()).unwrap_or("Assistant")),
                character.map(|value| value.description.as_str()).unwrap_or("Local AI companion"),
                chat.settings.ai_instructions.as_deref().unwrap_or_else(|| character.map(|value| value.style_notes.as_str()).unwrap_or("minimal")),
                recent_messages,
                if file_context.is_empty() { "No uploaded files in this chat." } else { &file_context },
                user_prompt
            ),
            sources: vec![
                "recent messages".to_string(),
                "character profile".to_string(),
                "chat-specific AI instructions".to_string(),
                "uploaded file metadata".to_string(),
                "explicit user prompt".to_string(),
            ],
        }
    }
}

fn is_text_like_extension(extension: &str) -> bool {
    matches!(
        extension,
        "txt"
            | "md"
            | "markdown"
            | "rst"
            | "tex"
            | "csv"
            | "tsv"
            | "json"
            | "jsonl"
            | "yaml"
            | "yml"
            | "toml"
            | "ini"
            | "log"
            | "xml"
            | "html"
            | "htm"
            | "svg"
            | "css"
            | "js"
            | "jsx"
            | "ts"
            | "tsx"
            | "py"
            | "rs"
            | "java"
            | "c"
            | "cc"
            | "cpp"
            | "cxx"
            | "h"
            | "hpp"
            | "cs"
            | "go"
            | "php"
            | "rb"
            | "swift"
            | "kt"
            | "sql"
            | "sh"
            | "ps1"
            | "bat"
    )
}

fn strip_markup(input: &str) -> String {
    let mut output = String::with_capacity(input.len());
    let mut in_tag = false;
    for ch in input.chars() {
        match ch {
            '<' => {
                in_tag = true;
                output.push(' ');
            }
            '>' => {
                in_tag = false;
                output.push(' ');
            }
            _ if !in_tag => output.push(ch),
            _ => {}
        }
    }

    output
}

fn normalize_attachment_text(input: &str) -> String {
    input
        .replace("\r\n", "\n")
        .replace('\r', "\n")
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

fn clip_chars(input: &str, max_chars: usize) -> String {
    if input.chars().count() <= max_chars {
        return input.to_string();
    }
    let clipped = input.chars().take(max_chars).collect::<String>();
    format!("{clipped}\n\n[truncated]")
}

fn runtime_executable_exists(kind: &str, path: &Path) -> bool {
    let candidates: &[&str] = match kind {
        "chat" => &["llama-cli.exe", "llama-cli", "main.exe", "main"],
        "image" => &[
            "image-runtime.exe",
            "sd.exe",
            "comfy-portable.exe",
            "image-runtime",
            "sd",
            "comfy-portable",
        ],
        "video" => &["video-runtime.exe", "video-runtime"],
        _ => &[],
    };

    candidates
        .iter()
        .map(|candidate| path.join(candidate))
        .any(|candidate| candidate.is_file())
}

fn model_search_dirs<const N: usize>(primary: &Path, extras: [PathBuf; N]) -> Vec<PathBuf> {
    let mut seen = HashSet::new();
    std::iter::once(primary.to_path_buf())
        .chain(extras)
        .filter_map(|path| {
            let key = path
                .to_string_lossy()
                .replace('\\', "/")
                .to_ascii_lowercase();
            seen.insert(key).then_some(path)
        })
        .collect()
}

fn scan_model_dirs(
    dirs: &[PathBuf],
    kind: &str,
    allowed_extensions: &[&str],
) -> AppResult<Vec<ModelRecord>> {
    let mut seen = HashSet::new();
    let mut models = Vec::new();

    for dir in dirs {
        for model in scan_model_dir(dir, kind, allowed_extensions)? {
            let key = fs::canonicalize(&model.path)
                .unwrap_or_else(|_| PathBuf::from(&model.path))
                .to_string_lossy()
                .replace('\\', "/")
                .to_ascii_lowercase();
            if seen.insert(key) {
                models.push(model);
            }
        }
    }

    models.sort_by(|left, right| left.name.cmp(&right.name));
    Ok(models)
}

fn scan_model_dir(
    dir: &Path,
    kind: &str,
    allowed_extensions: &[&str],
) -> AppResult<Vec<ModelRecord>> {
    let mut models = Vec::new();
    if !dir.exists() {
        return Ok(models);
    }

    for entry in WalkDir::new(dir).into_iter().filter_map(Result::ok) {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let extension = path
            .extension()
            .and_then(|ext| ext.to_str())
            .map(|value| value.to_ascii_lowercase())
            .unwrap_or_default();
        if !allowed_extensions
            .iter()
            .any(|allowed| *allowed == extension)
        {
            continue;
        }

        let Ok(metadata) = entry.metadata() else {
            continue;
        };
        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("model");
        let mut hasher = Sha256::new();
        hasher.update(path.to_string_lossy().as_bytes());
        let digest = format!("{:x}", hasher.finalize());

        let display_name = path
            .strip_prefix(dir)
            .ok()
            .and_then(|rel| rel.to_str())
            .map(|s| s.replace('\\', "/"))
            .unwrap_or_else(|| name.to_string());

        models.push(ModelRecord {
            id: digest,
            name: display_name,
            path: path.to_string_lossy().to_string(),
            kind: kind.to_string(),
            size_bytes: metadata.len(),
            last_modified: to_iso_time(&metadata),
            available: true,
            favorite: false,
            missing: false,
        });
    }

    models.sort_by(|left, right| left.name.cmp(&right.name));
    Ok(models)
}

fn repo_oversize_warnings(root: &Path, repo_limit_mb: u64) -> Vec<String> {
    let limit_bytes = repo_limit_mb * 1024 * 1024;
    WalkDir::new(root)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|entry| {
            let path = entry.path().to_string_lossy();
            !path.contains("node_modules")
                && !path.contains("src-tauri\\target")
                && !path.contains("\\.git\\")
                && entry.path().is_file()
        })
        .filter_map(|entry| {
            let metadata = entry.metadata().ok()?;
            if metadata.len() > limit_bytes {
                Some(format!(
                    "{} ({:.2} MB)",
                    entry.path().to_string_lossy(),
                    metadata.len() as f64 / 1024_f64 / 1024_f64
                ))
            } else {
                None
            }
        })
        .collect()
}

fn capitalize(input: &str) -> String {
    let mut chars = input.chars();
    match chars.next() {
        Some(first) => format!("{}{}", first.to_ascii_uppercase(), chars.as_str()),
        None => String::new(),
    }
}
