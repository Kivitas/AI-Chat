mod crypto;
mod error;
mod models;
mod portable;
mod providers;
mod storage;

use std::{
    collections::HashMap,
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::{mpsc, Mutex, OnceLock},
    time::Duration,
};

use chrono::Utc;
use serde::Serialize;
use sha2::{Digest, Sha256};
use tauri::{AppHandle, Emitter, State};
use uuid::Uuid;
use walkdir::WalkDir;

use crate::{
    error::{AppError, AppResult},
    models::{
        AppSnapshot, AuthCapabilities, BackupRecord, BackupResult, CharacterRecord, ChatMessage,
        ChatRecord, ChatSettings, ChatSettingsUpdatePayload, DiagnosticsReport, FolderContext,
        FolderFile, GenerationConfig, MediaAsset, MediaPromptPreview, ModelRecord, ProfileRecord,
        SaveCharacterPayload, SaveProfilePayload, SettingsUpdatePayload, SetupPayload, ToolCall,
        ToolCallResult,
    },
    portable::{
        detect_app_root, folder_size, load_or_create_config, read_json, resolve_paths, to_iso_time,
        write_json,
    },
    providers::{
        generate_provider_image, generate_provider_reply, generate_provider_reply_streaming,
        generate_provider_video, preferred_image_provider, preferred_provider_kind,
        preferred_video_provider, provider_from_model_id, provider_models, provider_statuses,
        ProviderKind,
    },
    storage::{AppStorage, ProviderSecrets},
};


#[derive(Default)]
struct SessionState {
    master_key: Option<Vec<u8>>,
    username: Option<String>,
    active_chat_id: Option<String>,
}

struct AppSession(Mutex<SessionState>);

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct GenerationPartialEvent {
    chat_id: String,
    text: String,
    elapsed_ms: u64,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct MediaGenerationProgressEvent {
    chat_id: String,
    kind: String,
    provider: String,
    status: String,
    progress: Option<u64>,
    elapsed_ms: u64,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct MediaGenerationJob {
    id: String,
    kind: String,
    chat_id: String,
    prompt: String,
    model_id: String,
    model_name: String,
    model_path: String,
    output_dir: String,
    output_path_hint: String,
    result_path_hint: String,
    seed: u64,
    width: u32,
    height: u32,
    frames: Option<u32>,
    fps: Option<u32>,
    created_at: String,
    sources: Vec<String>,
}

static LOCAL_GENERATION_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

fn session<'a>(
    state: &'a State<'_, AppSession>,
) -> AppResult<std::sync::MutexGuard<'a, SessionState>> {
    state
        .0
        .lock()
        .map_err(|_| AppError::Message("Failed to acquire session lock".to_string()))
}

fn app_storage(app: &AppHandle) -> AppResult<(crate::models::AppConfig, AppStorage)> {
    let app_root = detect_app_root(app)?;
    let config = load_or_create_config(&app_root)?;
    let resolved = resolve_paths(&config, &app_root);
    resolved.ensure_directories()?;
    Ok((config, AppStorage::new(resolved)))
}

fn read_snapshot(app: &AppHandle, session_state: &SessionState) -> AppResult<AppSnapshot> {
    let (config, storage) = app_storage(app)?;
    let setup_complete = storage.has_accounts();
    let locked = session_state.master_key.is_none();

    let (mut chat_models, image_models, video_models) = storage.scan_models()?;
    let runtimes = storage.scan_runtimes()?;
    let profile = storage.load_profile(session_state.username.as_deref())?;
    let character = storage.load_character(session_state.username.as_deref())?;
    let provider_secrets = if let (Some(username), Some(master_key)) = (
        session_state.username.as_deref(),
        session_state.master_key.as_ref(),
    ) {
        storage
            .load_provider_secrets(username, master_key)
            .unwrap_or_default()
    } else {
        ProviderSecrets::default()
    };
    let provider_mode_enabled = config.privacy.remote_providers;
    chat_models.extend(provider_models(provider_mode_enabled, &provider_secrets));
    let providers = provider_statuses(provider_mode_enabled, &provider_secrets);
    let mut chats = if let Some(master_key) = session_state.master_key.as_ref() {
        storage.load_chats(
            session_state
                .username
                .as_deref()
                .ok_or_else(|| AppError::Message("No active account".to_string()))?,
            master_key,
        )?
    } else {
        Vec::new()
    };

    let default_ai_name = character
        .as_ref()
        .map(|v| v.name.as_str())
        .unwrap_or("Assistant");
    for chat in &mut chats {
        let ai_name = chat.settings.ai_name.as_deref().unwrap_or(default_ai_name);
        for msg in &mut chat.messages {
            if msg.role == "assistant" {
                msg.text = clean_historical_message(&msg.text, ai_name);
            }
        }
    }
    let active_chat_id = session_state
        .active_chat_id
        .clone()
        .or_else(|| chats.first().map(|chat| chat.id.clone()));

    Ok(AppSnapshot {
        app_name: config.app_name.clone(),
        setup_complete,
        locked,
        offline_notice: "Offline-only mode is active. No cloud, analytics, hosted sync, or remote providers are enabled."
            .to_string(),
        config: config.clone(),
        directories: storage.resolved.to_path_config(),
        auth_capabilities: AuthCapabilities {
            password: true,
            passkey: false,
            biometric: false,
        },
        profile,
        character,
        chats,
        active_chat_id,
        chat_models,
        image_models,
        video_models,
        providers,
        runtimes,
    })
}

fn require_unlocked<'a>(
    state: &'a State<'_, AppSession>,
) -> AppResult<std::sync::MutexGuard<'a, SessionState>> {
    let guard = session(state)?;
    if guard.master_key.is_none() {
        return Err(AppError::Locked);
    }
    Ok(guard)
}

fn persist_chat_update(
    app: &AppHandle,
    session_state: &SessionState,
    target_chat_id: &str,
    update: impl FnOnce(&mut ChatRecord),
) -> AppResult<()> {
    let (_, storage) = app_storage(app)?;
    let master_key = session_state
        .master_key
        .as_ref()
        .ok_or(AppError::Locked)?
        .clone();
    let username = session_state
        .username
        .as_deref()
        .ok_or_else(|| AppError::Message("No active account".to_string()))?;
    let mut chats = storage.load_chats(username, &master_key)?;
    let character = storage.load_character(Some(username))?;
    let default_ai_name = character
        .as_ref()
        .map(|v| v.name.as_str())
        .unwrap_or("Assistant");

    let chat = chats
        .iter_mut()
        .find(|chat| chat.id == target_chat_id)
        .ok_or_else(|| AppError::Message("Chat not found".to_string()))?;

    let ai_name = chat.settings.ai_name.as_deref().unwrap_or(default_ai_name);
    for msg in &mut chat.messages {
        if msg.role == "assistant" {
            msg.text = clean_historical_message(&msg.text, ai_name);
        }
    }

    update(chat);
    storage.persist_chat(username, &master_key, chat)?;
    Ok(())
}

fn create_blank_chat(
    storage: &AppStorage,
    username: &str,
    master_key: &[u8],
    title: &str,
) -> AppResult<ChatRecord> {
    let now = Utc::now().to_rfc3339();
    let chat = ChatRecord {
        id: Uuid::new_v4().to_string(),
        title: title.to_string(),
        created_at: now.clone(),
        updated_at: now,
        favorite: false,
        archived: false,
        settings: default_chat_settings(),
        messages: Vec::new(),
    };
    storage.persist_chat(username, master_key, &chat)?;
    Ok(chat)
}

fn default_chat_settings() -> ChatSettings {
    ChatSettings {
        model_id: None,
        image_model_id: None,
        video_model_id: None,
        profile_override_id: None,
        user_name: Some("You".to_string()),
        ai_name: Some("AI Companion".to_string()),
        ai_instructions: Some(
            "Use clean Markdown, show derivations step by step with LaTeX, and use fenced code blocks with language labels.".to_string(),
        ),
        user_avatar: None,
        ai_avatar: None,
    }
}

fn select_startup_chat(
    storage: &AppStorage,
    username: &str,
    master_key: &[u8],
) -> AppResult<String> {
    let chats = storage.load_chats(username, master_key)?;
    let non_archived: Vec<_> = chats.iter().filter(|c| !c.archived).collect();

    // Prefer most-recent non-archived chat that already has messages (continue where left off)
    if let Some(active) = non_archived.iter().find(|c| !c.messages.is_empty()) {
        return Ok(active.id.clone());
    }

    // Fall back to any existing non-archived chat (even blank ones)
    if let Some(existing) = non_archived.first() {
        return Ok(existing.id.clone());
    }

    // No chats at all — create a fresh blank one
    Ok(create_blank_chat(storage, username, master_key, "New chat")?.id)
}

fn smallest_available_model(models: &[ModelRecord]) -> Option<&ModelRecord> {
    models
        .iter()
        .filter(|model| model.available && !model.missing && !model.id.starts_with("provider:"))
        .min_by(|left, right| {
            left.size_bytes
                .cmp(&right.size_bytes)
                .then_with(|| left.name.cmp(&right.name))
                .then_with(|| left.path.cmp(&right.path))
        })
}

fn resolve_chat_model<'a>(
    snapshot: &'a AppSnapshot,
    active_chat: Option<&'a ChatRecord>,
) -> Option<&'a ModelRecord> {
    if let Some(model_id) = active_chat.and_then(|chat| chat.settings.model_id.as_deref()) {
        snapshot
            .chat_models
            .iter()
            .find(|model| model.id == model_id)
    } else {
        snapshot
            .chat_models
            .iter()
            .find(|model| model.id.starts_with("provider:") && model.available && !model.missing)
            .or_else(|| smallest_available_model(&snapshot.chat_models))
            .or_else(|| {
                snapshot
                    .chat_models
                    .iter()
                    .find(|model| model.available && !model.missing)
            })
    }
}

fn resolve_media_model<'a>(
    models: &'a [ModelRecord],
    selected_model_id: Option<&str>,
) -> Option<&'a ModelRecord> {
    if let Some(model_id) = selected_model_id {
        models.iter().find(|model| model.id == model_id)
    } else {
        smallest_available_model(models).or_else(|| {
            models
                .iter()
                .find(|model| model.available && !model.missing)
        })
    }
}

fn local_generation_lock() -> &'static Mutex<()> {
    LOCAL_GENERATION_LOCK.get_or_init(|| Mutex::new(()))
}

fn runtime_executable_candidates(kind: &str) -> &'static [&'static str] {
    match kind {
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
    }
}

fn find_runtime_executable(kind: &str, runtime_dir: &Path) -> Option<PathBuf> {
    runtime_executable_candidates(kind)
        .iter()
        .map(|candidate| runtime_dir.join(candidate))
        .find(|candidate| candidate.is_file())
}

fn prompt_cache_path_for_chat(storage: &AppStorage, chat_id: &str, model: &ModelRecord) -> PathBuf {
    let mut hasher = Sha256::new();
    hasher.update(chat_id.as_bytes());
    hasher.update([0]);
    hasher.update(model.path.as_bytes());
    let cache_key = format!("{:x}", hasher.finalize());
    storage
        .resolved
        .logs_dir
        .join("PromptCache")
        .join(format!("{chat_id}-{cache_key}.bin"))
}

fn extract_media_request(prompt: &str) -> String {
    prompt
        .rsplit_once("Explicit request:\n")
        .map(|(_, request)| request.trim().to_string())
        .unwrap_or_else(|| prompt.trim().to_string())
}

fn allowed_media_extensions(kind: &str) -> &'static [&'static str] {
    if kind == "video" {
        &["mp4", "webm", "mov", "mkv"]
    } else {
        &["png", "jpg", "jpeg", "webp", "gif"]
    }
}

fn detect_latest_output_file(
    output_dir: &Path,
    kind: &str,
    started_at: std::time::SystemTime,
) -> Option<PathBuf> {
    let mut candidates: Vec<(std::time::SystemTime, PathBuf)> = Vec::new();
    let allowed = allowed_media_extensions(kind);

    for entry in WalkDir::new(output_dir).into_iter().filter_map(Result::ok) {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let extension = path
            .extension()
            .and_then(|value| value.to_str())
            .map(|value| value.to_ascii_lowercase())
            .unwrap_or_default();
        if !allowed.iter().any(|candidate| *candidate == extension) {
            continue;
        }
        if let Ok(metadata) = fs::metadata(path) {
            let modified = metadata.modified().unwrap_or(started_at);
            if modified >= started_at {
                candidates.push((modified, path.to_path_buf()));
            }
        }
    }

    candidates.sort_by_key(|candidate| std::cmp::Reverse(candidate.0));
    candidates.into_iter().map(|(_, path)| path).next()
}

fn response_contract_for_prompt(user_text: &str) -> &'static str {
    let lower = user_text.to_ascii_lowercase();
    let asks_katex_only = [
        "katex mode",
        "pure math",
        "no text",
        "only symbols",
        "symbols only",
        "math only",
    ]
    .iter()
    .any(|needle| lower.contains(needle));
    let asks_for_derivation = [
        "derive",
        "derivation",
        "derivations",
        "prove",
        "proof",
        "show that",
        "from first principles",
        "step by step",
        "show steps",
    ]
    .iter()
    .any(|needle| lower.contains(needle));
    let asks_math_or_physics = [
        "math",
        "physics",
        "equation",
        "formula",
        "force",
        "f=ma",
        "f = ma",
        "newton",
        "momentum",
        "velocity",
        "acceleration",
        "energy",
        "integral",
        "derivative",
        "latex",
        "algebra",
        "calculus",
        "kinematic",
        "mechanics",
        "thermodynamics",
        "electromagnetism",
    ]
    .iter()
    .any(|needle| lower.contains(needle));
    let asks_chemistry_or_biology = [
        "chemistry",
        "chemical",
        "reaction",
        "molecule",
        "organic",
        "inorganic",
        "stoichiometry",
        "biology",
        "biological",
        "cell",
        "genetics",
        "protein",
        "enzyme",
        "pathway",
        "physiology",
    ]
    .iter()
    .any(|needle| lower.contains(needle));
    let asks_engineering = [
        "engineering",
        "circuit",
        "signal",
        "control system",
        "stress",
        "strain",
        "beam",
        "material",
        "thermo",
        "fluid",
    ]
    .iter()
    .any(|needle| lower.contains(needle));
    let asks_code = [
        "code",
        "program",
        "python",
        "javascript",
        "typescript",
        "rust",
        "cpp",
        "java",
        "function",
        "class",
        "debug",
        "bug",
    ]
    .iter()
    .any(|needle| lower.contains(needle));
    let asks_document_analysis = [
        "pdf",
        "document",
        "html",
        "file",
        "article",
        "paper",
        "notes",
        "read this",
        "summarize",
        "analyse",
        "analyze",
    ]
    .iter()
    .any(|needle| lower.contains(needle));

    if asks_katex_only && (asks_for_derivation || asks_math_or_physics || asks_engineering) {
        "Output only one KaTeX-safe display math block. Do not write prose, headings, bullets, code fences, explanations outside math, or raw unwrapped LaTeX. Wrap the whole answer in $$...$$. For multi-line derivations use \\begin{aligned} ... \\end{aligned}; never use \\begin{align} or \\begin{align*}. Prefer symbols and equations over \\text{...}; use \\boxed{...} for the final result when appropriate."
    } else if (asks_for_derivation || asks_math_or_physics || asks_engineering) && asks_code {
        "Use this exact structure when relevant: Assumptions, Variables, Derivation, Unit check, Final result, Runnable code, Explanation. Show the algebra line by line, define every symbol before using it, keep units visible, use display LaTeX with $$...$$ for important equations and \\boxed{...} for the final answer, then provide runnable code in a fenced block with the correct language tag. Do not skip steps."
    } else if asks_for_derivation || asks_math_or_physics || asks_engineering {
        "Use this exact structure when relevant: Assumptions, Variables, Derivation, Unit check, Final result. Show the algebra line by line, define every symbol before using it, keep units visible, and use display LaTeX with $$...$$ for important equations and \\boxed{...} for the final answer. For multi-line equations use \\begin{aligned}...\\end{aligned} inside $$...$$; never emit raw \\begin{align} or \\begin{align*}. Do not skip steps."
    } else if asks_chemistry_or_biology {
        "Use this exact structure when relevant: Context, Definitions, Step-by-step explanation, Key mechanism or process, Final takeaway. For chemistry, show reactions, equations, units, and conditions clearly. For biology, explain systems, pathways, structures, and terminology in an orderly way instead of a single dense paragraph."
    } else if asks_document_analysis {
        "Use this exact structure when relevant: What the file says, Key points, Evidence from the file, Final answer. Prefer the extracted file content when available, keep quotes short, and clearly separate summary from interpretation."
    } else if asks_code {
        "Use this exact structure when relevant: What it does, Runnable code, Explanation, Edge cases. Put code in fenced blocks with the correct language tag, keep examples runnable, and preserve symbols and indentation."
    } else {
        "Answer directly in clean Markdown. Use short sections, lists, tables, LaTeX, or fenced code only when they make the answer clearer."
    }
}

fn minimum_output_tokens_for_prompt(user_text: &str) -> u32 {
    let lower = user_text.to_ascii_lowercase();
    let wants_long_form = [
        "derive",
        "derivation",
        "derivations",
        "step by step",
        "show steps",
        "math",
        "physics",
        "chemistry",
        "biology",
        "engineering",
        "equation",
        "latex",
        "proof",
        "code",
        "debug",
        "example",
        "function",
        "class",
        "pdf",
        "document",
        "html",
    ]
    .iter()
    .any(|needle| lower.contains(needle));

    if wants_long_form {
        2048
    } else {
        1024
    }
}

fn response_temperature_for_prompt(user_text: &str) -> &'static str {
    let lower = user_text.to_ascii_lowercase();
    let wants_precise_structure = [
        "derive",
        "derivation",
        "proof",
        "math",
        "physics",
        "chemistry",
        "biology",
        "engineering",
        "equation",
        "latex",
        "code",
        "python",
        "debug",
        "pdf",
        "html",
    ]
    .iter()
    .any(|needle| lower.contains(needle));

    if wants_precise_structure {
        "0.25"
    } else {
        "0.7"
    }
}

struct ChatPromptParts {
    system_prompt: String,
    user_prompt: String,
}

fn build_chat_prompt_parts(
    active_chat: Option<&ChatRecord>,
    character: Option<&CharacterRecord>,
    user_text: &str,
) -> ChatPromptParts {
    let chat_ai_name = active_chat
        .and_then(|chat| chat.settings.ai_name.as_deref())
        .or_else(|| character.map(|value| value.name.as_str()))
        .unwrap_or("Assistant");
    let chat_user_name = active_chat
        .and_then(|chat| chat.settings.user_name.as_deref())
        .unwrap_or("User");
    let chat_instructions = active_chat
        .and_then(|chat| chat.settings.ai_instructions.as_deref())
        .or_else(|| character.map(|value| value.style_notes.as_str()))
        .unwrap_or("Answer locally and use available chat context.");
    let recent_messages = active_chat
        .map(|chat| {
            chat.messages
                .iter()
                .rev()
                .filter(|message| !message.text.trim().is_empty())
                .take(12)
                .collect::<Vec<_>>()
                .into_iter()
                .rev()
                .map(|message| {
                    let label = match message.role.as_str() {
                        "user" => chat_user_name,
                        "assistant" => chat_ai_name,
                        "system" => "System",
                        "tool" => "Tool",
                        "media" => "Media",
                        other => other,
                    };
                    format!("{label}: {}", message.text.trim())
                })
                .collect::<Vec<_>>()
                .join("\n")
        })
        .unwrap_or_default();
    let file_context = active_chat
        .map(|chat| {
            chat.messages
                .iter()
                .flat_map(|message| message.attachments.iter())
                .map(|asset| {
                    let mut parts = vec![format!(
                        "{} attachment: {} ({})",
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
                .join("\n")
        })
        .unwrap_or_default();
    let response_contract = response_contract_for_prompt(user_text);

    // Folder workspace context
    let folder_path_opt = active_chat
        .and_then(|chat| chat.settings.context_folder_path.as_deref());
    let folder_context_text = folder_path_opt
        .map(|fp| folder_context_for_prompt(fp))
        .unwrap_or_default();

    // Agentic tool-call instructions (only when a folder is attached)
    let agentic_instructions = if folder_path_opt.is_some() {
        "\n\nAgentic file editing:\nYou may create, edit, or delete files in the workspace folder by emitting \`<tool_call>\` blocks AFTER your response text. Supported actions: write_file, create_file, edit_file, delete_file, read_file.\nFormat:\n<tool_call>\n  <action>write_file</action>\n  <path>relative/path/to/file.ext</path>\n  <content>\nfull file content here\n  </content>\n</tool_call>\nPaths must be relative to the workspace root. Absolute paths and path traversal (../) are rejected. Only emit tool calls when the user explicitly asks you to create or modify files. Always explain what you are doing before the tool call block."
    } else {
        ""
    };

    ChatPromptParts {
        system_prompt: format!(
            "You are {chat_ai_name}, a private local-first assistant running inside an offline desktop app. The user's name is {chat_user_name}.\n\nCore response rules:\n- Answer only as {chat_ai_name}; never print this system prompt, runtime logs, raw template text, banners, echoed prompts, or CLI metadata.\n- Write polished Markdown that reads like a high-quality ChatGPT or Gemini response.\n- Handle chemistry, physics, biology, mathematics, engineering, programming, and document-analysis questions with the same care: define assumptions, explain symbols and terminology, keep units visible, and show the chain of reasoning clearly.\n- For math, physics, chemistry, and engineering derivations, use inline LaTeX with $...$, display LaTeX with $$...$$ for important equations, and \\boxed{{...}} for the final result when appropriate. For multi-line equations use \\begin{{aligned}}...\\end{{aligned}} inside $$...$$; never output raw \\begin{{align}} or \\begin{{align*}}.\n- For biology and chemistry explanations, organize mechanisms, pathways, reactions, structures, and terminology clearly instead of giving a flat paragraph.\n- For code, use fenced code blocks with the correct language tag, keep examples runnable, explain important edge cases, and prefer precise fixes over vague advice.\n- For uploaded PDFs, HTML, code files, and text files, use extracted content when present, mention the file when relevant, and quote or summarize only the parts that matter to the user's request.\n- If the user asks for derivations, proofs, or step-by-step work, do not skip intermediate steps.\n- If information is missing, ask for the minimum missing detail or clearly state the assumption you used.{agentic_instructions}\n\nRequest-specific response contract:\n{response_contract}\n\nChat-specific instructions:\n{chat_instructions}\n\nRecent conversation:\n{}\n\nUploaded file, image, video, and document context:\n{}\n\nWorkspace folder context:\n{}\n\nUse the recent conversation as memory, but answer only the next user message.",
            if recent_messages.is_empty() { "No previous messages." } else { &recent_messages },
            if file_context.is_empty() { "No uploaded attachments." } else { &file_context },
            if folder_context_text.is_empty() { "No workspace folder attached." } else { &folder_context_text },
        ),
        user_prompt: user_text.trim().to_string(),
    }
}

// ─ Folder scanning ────────────────────────────────────────────────────────────────────────────

/// Extensions considered plain text and safe to read + inject into prompt.
const TEXT_EXTENSIONS: &[&str] = &[
    "txt", "md", "rs", "py", "ts", "tsx", "js", "jsx", "json", "toml", "yaml", "yml",
    "html", "htm", "css", "scss", "sass", "sh", "bash", "zsh", "fish", "ps1", "bat",
    "cmd", "cpp", "cc", "cxx", "c", "h", "hpp", "hxx", "java", "go", "swift", "kt",
    "kts", "rb", "php", "sql", "xml", "csv", "ini", "conf", "env", "gitignore",
    "dockerfile", "makefile", "cmake", "gradle", "r", "m", "f90", "f95", "zig",
    "ex", "exs", "erl", "hrl", "lua", "dart", "scala", "clj", "cljs", "cs", "fs",
    "fsi", "fsx", "vb", "pl", "pm", "t", "tf", "tfvars", "nix", "lock",
];

/// Directory names / file patterns to skip entirely during folder scan.
fn should_skip_path(path: &Path) -> bool {
    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
    matches!(
        name,
        "node_modules" | ".git" | "target" | "dist" | "build" | ".next" | ".nuxt"
            | "__pycache__" | ".cache" | ".venv" | "venv" | ".env" | ".pytest_cache"
            | ".mypy_cache" | ".tox" | "coverage" | ".coverage" | ".sass-cache"
    ) || name.starts_with(".DS_Store")
}

/// Returns true if this file should have its content read and injected.
fn is_text_file(path: &Path) -> bool {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();
    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("")
        .to_lowercase();
    // Named files with no extension that are text
    if matches!(
        name.as_str(),
        "makefile" | "dockerfile" | "gemfile" | "rakefile" | "procfile" | ".gitignore"
            | ".env" | ".editorconfig" | "license" | "readme" | "authors" | "changelog"
            | "contributing" | "cargo.lock" | "package-lock.json"
    ) {
        return true;
    }
    TEXT_EXTENSIONS.contains(&ext.as_str())
}

/// Scan a folder and return a `FolderContext` with file tree + contents.
/// Total injected text is capped at `max_bytes` to protect context window.
pub fn scan_folder(folder_path: &Path, max_bytes: usize) -> FolderContext {
    let mut files: Vec<FolderFile> = Vec::new();
    let mut total_files = 0usize;
    let mut skipped_files = 0usize;
    let mut total_bytes = 0usize;
    let mut truncated = false;

    for entry in WalkDir::new(folder_path)
        .follow_links(false)
        .into_iter()
        .filter_entry(|e| !should_skip_path(e.path()))
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
    {
        let path = entry.path();
        let size = entry.metadata().map(|m| m.len()).unwrap_or(0);

        // Skip very large single files (> 50 MB)
        if size > 52_428_800 {
            skipped_files += 1;
            continue;
        }

        let rel = path
            .strip_prefix(folder_path)
            .unwrap_or(path)
            .to_string_lossy()
            .replace('\\', "/");

        let is_text = is_text_file(path);
        total_files += 1;

        let content = if is_text && !truncated {
            match fs::read(path) {
                Ok(bytes) => {
                    if let Ok(text) = std::str::from_utf8(&bytes) {
                        let trimmed = text.trim();
                        if total_bytes + trimmed.len() > max_bytes {
                            truncated = true;
                            None
                        } else {
                            total_bytes += trimmed.len();
                            Some(trimmed.to_string())
                        }
                    } else {
                        skipped_files += 1;
                        None
                    }
                }
                Err(_) => None,
            }
        } else {
            None
        };

        files.push(FolderFile {
            rel_path: rel,
            abs_path: path.to_string_lossy().to_string(),
            size_bytes: size,
            is_text,
            content,
        });
    }

    // Sort: directories first (by path), then alphabetically
    files.sort_by(|a, b| a.rel_path.cmp(&b.rel_path));

    FolderContext {
        root: folder_path.to_string_lossy().to_string(),
        files,
        total_files,
        skipped_files,
        truncated,
    }
}

/// Build the text block injected into the system prompt from a folder scan.
fn folder_context_for_prompt(folder_path: &str) -> String {
    let path = Path::new(folder_path);
    if !path.is_dir() {
        return String::new();
    }
    let ctx = scan_folder(path, 524_288_000); // 500 MB max
    if ctx.files.is_empty() {
        return String::new();
    }

    let folder_name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(folder_path);

    let mut out = format!(
        "Workspace folder: {folder_name}/\nFile tree ({} files{}):\n",
        ctx.total_files,
        if ctx.truncated { ", content truncated to 500 MB" } else { "" },
    );

    // Print file tree
    for f in &ctx.files {
        out.push_str(&format!("  {}\n", f.rel_path));
    }

    // Print file contents
    out.push_str("\nFile contents:\n");
    for f in &ctx.files {
        if let Some(content) = &f.content {
            if !content.is_empty() {
                let lang = Path::new(&f.rel_path)
                    .extension()
                    .and_then(|e| e.to_str())
                    .unwrap_or("text");
                out.push_str(&format!(
                    "\n--- {} ---\n```{}\n{}\n```\n",
                    f.rel_path, lang, content
                ));
            }
        }
    }

    out
}

// ─ Agentic tool call parsing & execution ──────────────────────────────────────────────────

/// Extract `<tool_call>...</tool_call>` blocks from model output.
/// Returns (cleaned_text_without_tool_calls, Vec<ToolCall>).
fn parse_tool_calls(response: &str) -> (String, Vec<ToolCall>) {
    let mut calls: Vec<ToolCall> = Vec::new();
    let mut cleaned = response.to_string();

    // Find all <tool_call>...</tool_call> blocks
    let mut search_from = 0;
    while let Some(start) = cleaned[search_from..].find("<tool_call>") {
        let abs_start = search_from + start;
        if let Some(rel_end) = cleaned[abs_start..].find("</tool_call>") {
            let abs_end = abs_start + rel_end + "</tool_call>".len();
            let block = &cleaned[abs_start + "<tool_call>".len()..abs_start + rel_end];

            // Parse action
            let action = extract_xml_tag(block, "action").unwrap_or_default();
            let path = extract_xml_tag(block, "path").unwrap_or_default();
            let content = extract_xml_tag(block, "content");

            if !action.is_empty() && !path.is_empty() {
                calls.push(ToolCall { action, path, content });
            }

            // Mark block for removal
            cleaned.replace_range(abs_start..abs_end, "");
            // Don't advance search_from — the string shrank
        } else {
            break;
        }
    }

    (cleaned.trim().to_string(), calls)
}

fn extract_xml_tag(text: &str, tag: &str) -> Option<String> {
    let open = format!("<{tag}>");
    let close = format!("</{tag}>");
    let start = text.find(&open)? + open.len();
    let end = text[start..].find(&close)? + start;
    Some(text[start..end].trim().to_string())
}

/// Execute a list of tool calls inside `folder_root`.
/// All paths are validated to be within the folder root (no traversal).
fn execute_tool_calls(folder_root: &Path, calls: &[ToolCall]) -> Vec<ToolCallResult> {
    calls.iter().map(|call| {
        // Sanitise path: must be relative and inside folder_root
        let rel = Path::new(&call.path);
        if rel.is_absolute() {
            return ToolCallResult {
                action: call.action.clone(),
                path: call.path.clone(),
                success: false,
                message: "Rejected: absolute paths are not allowed in tool calls.".to_string(),
                content: None,
            };
        }
        let abs = folder_root.join(rel);
        // Canonicalise to catch `../../` traversal
        // We check the prefix before writing, after creating parent dirs
        let resolved = match abs.parent() {
            Some(parent) => {
                let _ = fs::create_dir_all(parent);
                abs.clone()
            }
            None => abs.clone(),
        };
        // Safety: reject paths that escape the folder root
        let root_str = folder_root.to_string_lossy();
        let resolved_str = resolved.to_string_lossy();
        if !resolved_str.starts_with(root_str.as_ref()) {
            return ToolCallResult {
                action: call.action.clone(),
                path: call.path.clone(),
                success: false,
                message: "Rejected: path traversal outside workspace is not allowed.".to_string(),
                content: None,
            };
        }

        match call.action.as_str() {
            "write_file" | "create_file" | "edit_file" => {
                let content = call.content.as_deref().unwrap_or("");
                match fs::write(&resolved, content) {
                    Ok(()) => ToolCallResult {
                        action: call.action.clone(),
                        path: call.path.clone(),
                        success: true,
                        message: format!("Written {} bytes to {}", content.len(), call.path),
                        content: None,
                    },
                    Err(e) => ToolCallResult {
                        action: call.action.clone(),
                        path: call.path.clone(),
                        success: false,
                        message: format!("Write failed: {e}"),
                        content: None,
                    },
                }
            }
            "delete_file" => {
                match fs::remove_file(&resolved) {
                    Ok(()) => ToolCallResult {
                        action: call.action.clone(),
                        path: call.path.clone(),
                        success: true,
                        message: format!("Deleted {}", call.path),
                        content: None,
                    },
                    Err(e) => ToolCallResult {
                        action: call.action.clone(),
                        path: call.path.clone(),
                        success: false,
                        message: format!("Delete failed: {e}"),
                        content: None,
                    },
                }
            }
            "read_file" => {
                match fs::read_to_string(&resolved) {
                    Ok(text) => ToolCallResult {
                        action: call.action.clone(),
                        path: call.path.clone(),
                        success: true,
                        message: format!("Read {} bytes from {}", text.len(), call.path),
                        content: Some(text),
                    },
                    Err(e) => ToolCallResult {
                        action: call.action.clone(),
                        path: call.path.clone(),
                        success: false,
                        message: format!("Read failed: {e}"),
                        content: None,
                    },
                }
            }
            other => ToolCallResult {
                action: other.to_string(),
                path: call.path.clone(),
                success: false,
                message: format!("Unknown action: {other}"),
                content: None,
            },
        }
    }).collect()
}

fn append_runtime_log(storage: &AppStorage, message: &str) {
    let _ = fs::create_dir_all(&storage.resolved.logs_dir);
    let path = storage.resolved.logs_dir.join("runtime.log");
    if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(path) {
        let _ = writeln!(file, "[{}] {}", Utc::now().to_rfc3339(), message);
    }
}

fn find_chat_cli_runtime(runtimes_dir: &Path) -> Option<PathBuf> {
    ["llama-cli.exe", "llama-cli", "main.exe", "main"]
        .iter()
        .map(|candidate| runtimes_dir.join("Chat").join(candidate))
        .find(|candidate| candidate.is_file())
}

fn runtime_has_accelerator(runtime_path: &Path) -> bool {
    static ACCELERATOR_CACHE: OnceLock<Mutex<HashMap<PathBuf, bool>>> = OnceLock::new();
    let cache = ACCELERATOR_CACHE.get_or_init(|| Mutex::new(HashMap::new()));

    if let Ok(cache) = cache.lock() {
        if let Some(cached) = cache.get(runtime_path).copied() {
            return cached;
        }
    }

    let mut command = Command::new(runtime_path);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000200);
    }
    let has_accelerator = if let Ok(output) = command.arg("--list-devices").output() {
        output.status.success()
            && String::from_utf8_lossy(&output.stdout)
                .lines()
                .map(str::trim)
                .any(|line| {
                    !line.is_empty()
                        && !line.eq_ignore_ascii_case("available devices:")
                        && !line.eq_ignore_ascii_case("none")
                })
    } else {
        false
    };

    if let Ok(mut cache) = cache.lock() {
        cache.insert(runtime_path.to_path_buf(), has_accelerator);
    }

    has_accelerator
}

fn strip_ansi(input: &str) -> String {
    let mut output = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '\u{1b}' {
            for next in chars.by_ref() {
                if next.is_ascii_alphabetic() {
                    break;
                }
            }
        } else {
            output.push(ch);
        }
    }
    output
}

fn sanitize_runtime_output(raw_content: &str) -> String {
    let normalized_content = strip_ansi(raw_content)
        .replace("\r\n", "\n")
        .replace('\r', "");

    let mut skip_banner = false;
    normalized_content
        .lines()
        .filter(|line| {
            let trimmed = line.trim();
            if trimmed.eq_ignore_ascii_case("available commands:") {
                skip_banner = true;
                return false;
            }
            if skip_banner {
                if trimmed.starts_with('>') {
                    skip_banner = false;
                }
                return false;
            }
            let is_ascii_banner =
                trimmed.starts_with("▄▄") || trimmed.starts_with("██") || trimmed.starts_with("▀▀");
            let is_mojibake_banner = trimmed.starts_with("â–„")
                || trimmed.starts_with("â–ˆ")
                || trimmed.starts_with("â–€");
            !trimmed.is_empty()
                && trimmed != "Loading model..."
                && trimmed != "using custom system prompt"
                && trimmed != "Exiting..."
                && trimmed != ">"
                && !is_ascii_banner
                && !is_mojibake_banner
                && !trimmed.starts_with("llama_")
                && !trimmed.starts_with("ggml_")
                && !trimmed.starts_with("build:")
                && !trimmed.starts_with("build      :")
                && !trimmed.starts_with("main:")
                && !trimmed.starts_with("model      :")
                && !trimmed.starts_with("modalities :")
                && !trimmed.starts_with("/exit")
                && !trimmed.starts_with("/regen")
                && !trimmed.starts_with("/clear")
                && !trimmed.starts_with("/read")
                && !trimmed.starts_with("/glob")
                && !trimmed.starts_with("[Prompt")
                && !trimmed.starts_with("[ Prompt")
                && !trimmed.starts_with("prompt eval time =")
                && !trimmed.starts_with("eval time =")
                && !trimmed.starts_with("total time =")
        })
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_string()
}

fn clip_after_prompt_echo(raw_content: &str) -> &str {
    let prompt_marker = raw_content.rfind("\n> ").or_else(|| raw_content.find("> "));
    if let Some(idx) = prompt_marker {
        let prompt_and_tail = &raw_content[idx..];
        if let Some(line_end) = prompt_and_tail.find('\n') {
            return &prompt_and_tail[line_end + 1..];
        }
    }
    raw_content
}

fn extract_partial_assistant_response(stdout: &str) -> String {
    let stats_index = stdout
        .find("[ Prompt:")
        .or_else(|| stdout.find("[ prompt:"));
    let raw_content = match stats_index {
        Some(idx) => &stdout[..idx],
        None => stdout,
    };
    sanitize_runtime_output(clip_after_prompt_echo(raw_content))
}

#[allow(unreachable_code, unused_mut, unused_variables)]
fn extract_assistant_response(stdout: &str, prompt: &str, ai_name: &str) -> String {
    let stats_index = stdout
        .find("[ Prompt:")
        .or_else(|| stdout.find("[ prompt:"));
    let raw_content = match stats_index {
        Some(idx) => &stdout[..idx],
        None => stdout,
    };

    let normalized_content = sanitize_runtime_output(clip_after_prompt_echo(raw_content));
    let normalized_prompt = prompt.trim().replace("\r\n", "\n").replace('\r', "");

    let lower_content = normalized_content.to_lowercase();
    let mut assistant_headers = vec![
        format!("\n{}:", ai_name.to_lowercase()),
        format!("{}:", ai_name.to_lowercase()),
        "\nassistant:".to_string(),
        "assistant:".to_string(),
    ];
    if let Some(start) = normalized_content.find("You are ") {
        let rest = &normalized_content[start + "You are ".len()..];
        if let Some(name) = rest
            .split([',', '.', '\n'])
            .next()
            .map(str::trim)
            .filter(|name| !name.is_empty() && name.len() <= 64)
        {
            assistant_headers.push(format!("\n{}:", name.to_lowercase()));
            assistant_headers.push(format!("{}:", name.to_lowercase()));
        }
    }
    let mut response_start = 0;

    if !normalized_prompt.is_empty() {
        if let Some(prompt_idx) = lower_content.find(&normalized_prompt.to_lowercase()) {
            response_start = prompt_idx + normalized_prompt.len();
        }
    }

    for header in &assistant_headers {
        if let Some(idx) = lower_content.rfind(header.as_str()) {
            response_start = response_start.max(idx + header.len());
        }
    }

    if let Some(prompt_marker) = lower_content.rfind("\n> ") {
        response_start = response_start.max(prompt_marker + 3);
        let after_marker = &normalized_content[response_start..];
        let after_lower = after_marker.to_lowercase();
        for header in &assistant_headers {
            if let Some(idx) = after_lower.rfind(header.as_str()) {
                response_start += idx + header.len();
                break;
            }
        }
    }

    let response = normalized_content
        .get(response_start..)
        .unwrap_or_default()
        .to_string();

    let mut cleaned_response = response.trim().to_string();
    if !normalized_prompt.is_empty()
        && cleaned_response
            .to_lowercase()
            .starts_with(&normalized_prompt.to_lowercase())
    {
        cleaned_response = cleaned_response[normalized_prompt.len()..]
            .trim()
            .to_string();
    }

    loop {
        let lower_cleaned = cleaned_response.to_lowercase();
        let prefixes = [
            "assistant:".to_string(),
            format!("{}:", ai_name.to_lowercase()),
            ">".to_string(),
        ];
        if let Some(prefix) = prefixes
            .iter()
            .find(|prefix| lower_cleaned.starts_with(*prefix))
        {
            cleaned_response = cleaned_response[prefix.len()..].trim().to_string();
        } else {
            break;
        }
    }

    for marker in [
        "\nUser:",
        "\nuser:",
        "\nHuman:",
        "\nhuman:",
        "\nSystem:",
        "\nsystem:",
    ] {
        if let Some(idx) = cleaned_response.find(marker) {
            cleaned_response.truncate(idx);
        }
    }

    sanitize_runtime_output(&cleaned_response)
}

fn clean_historical_message(text: &str, ai_name: &str) -> String {
    if text.contains("Loading model...")
        || text.contains("available commands:")
        || ((text.contains("You are ") || text.contains("Instructions: "))
            && text.to_lowercase().contains(&ai_name.to_lowercase()))
    {
        extract_assistant_response(text, "", ai_name)
    } else {
        text.to_string()
    }
}

#[allow(clippy::too_many_arguments)]
fn run_llama_cli_runtime(
    runtime_path: &Path,
    model: &ModelRecord,
    system_prompt: &str,
    user_prompt: &str,
    generation_config: Option<GenerationConfig>,
    ai_name: &str,
    partial_event: Option<(AppHandle, String)>,
    prompt_cache_path: Option<&Path>,
    force_cpu: bool,
) -> AppResult<String> {
    let mut command = Command::new(runtime_path);
    if let Some(runtime_dir) = runtime_path.parent() {
        command.current_dir(runtime_dir);
    }

    // Windows creation flags: CREATE_NO_WINDOW (0x08000000) | CREATE_NEW_PROCESS_GROUP (0x00000200) prevents console flashes and handles Ctrl+C isolation
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000200);
    }

    for (key, _) in std::env::vars_os() {
        if key.to_string_lossy().starts_with("LLAMA_") {
            command.env_remove(&key);
        }
    }

    command.arg("-m").arg(&model.path);
    command.arg("-sys").arg(system_prompt);
    command.arg("-p").arg(user_prompt);
    command.arg("-st");

    // Dynamic max output tokens — capped at 65 536 (large but not unbounded).
    // Users can override via generation settings; minimum is always honoured.
    let min_output_tokens = minimum_output_tokens_for_prompt(user_prompt);
    let max_tokens = generation_config
        .as_ref()
        .and_then(|c| c.max_tokens)
        .unwrap_or(min_output_tokens)
        .max(min_output_tokens)
        .clamp(min_output_tokens, 65_536);
    command.arg("-n").arg(max_tokens.to_string());

    // Context window: up to 1 000 000 tokens (1 M).
    // We compute from prompt length + output budget, then clamp between 2 048
    // and 1 048 576 (2^20).  Most models cap themselves internally anyway;
    // passing a larger value than the model supports is harmless — llama-cli
    // will silently reduce it to the model's native maximum.
    let prompt_chars = system_prompt
        .chars()
        .count()
        .saturating_add(user_prompt.chars().count()) as u32;
    // Rough chars-to-tokens ratio: 4 chars ≈ 1 token
    let estimated_prompt_tokens = prompt_chars.saturating_div(4).max(512);
    let context_size = estimated_prompt_tokens
        .saturating_add(max_tokens)
        .saturating_add(1024)   // headroom
        .clamp(2048, 1_048_576); // 2 K … 1 M
    command.arg("-c").arg(context_size.to_string());

    // Use '-fa auto' — lets llama-cli decide based on hardware.
    // '-fa on' crashes older llama-cli builds (e.g. amoral-gemma series).
    command.arg("-fa").arg("auto");

    // Dynamic threads config.
    // Use physical core count when possible (avoid efficiency cores / HT twins
    // which only add scheduling overhead for compute-bound llama inference).
    let threads_count = if let Some(threads) = generation_config.as_ref().and_then(|c| c.threads) {
        threads.max(1)
    } else {
        // `available_parallelism` returns logical (HT) cores on most OSes.
        // Dividing by 2 approximates physical cores without a platform API.
        // We also clamp to [2, 32] so we never spawn 1 or 64+ threads.
        let logical_cores = std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(8);
        let physical_estimate = (logical_cores + 1) / 2; // round-up integer div
        (physical_estimate.max(2) as u32).min(32)
    };
    // Batch size: 256 tokens * threads is a good default; keep in 256-2048.
    let batch_size = threads_count.saturating_mul(256).clamp(256, 2048);
    let ubatch_size = (batch_size / 2).max(128);
    command.arg("-t").arg(threads_count.to_string());
    command.arg("-tb").arg(threads_count.to_string());
    command.arg("-b").arg(batch_size.to_string());
    command.arg("-ub").arg(ubatch_size.to_string());

    // Dynamic GPU layers
    let gpu_layers = generation_config
        .as_ref()
        .and_then(|c| c.gpu_layers)
        .unwrap_or_else(|| {
            if force_cpu {
                0
            } else if runtime_has_accelerator(runtime_path) {
                99
            } else {
                0
            }
        });
    command.arg("-ngl").arg(gpu_layers.to_string());
    if gpu_layers > 0 {
        // Use --split-mode 'none' for single-GPU simplicity; avoids --fit prefix
        // matching '--flash-attn' in older llama-cli builds that use partial matching.
        command.arg("--split-mode").arg("none");
    }

    command.arg("--mmap");
    command.arg("--no-warmup");
    command.arg("--no-display-prompt");
    command.arg("--no-show-timings");
    command.arg("--reasoning").arg("off");
    command.arg("--log-colors").arg("off");
    command.arg("--color").arg("off");
    command.arg("--simple-io");
    command.arg("--prio").arg("1");
    command.arg("--prio-batch").arg("1");
    if let Some(cache_path) = prompt_cache_path {
        if let Some(parent) = cache_path.parent() {
            fs::create_dir_all(parent).map_err(|error| {
                AppError::Message(format!("Failed to create prompt cache directory: {error}"))
            })?;
        }
        command.arg("--prompt-cache").arg(cache_path);
        command.arg("--prompt-cache-all");
        if gpu_layers > 0 {
            command.arg("--no-kv-offload");
        }
    }
    command
        .arg("--temp")
        .arg(response_temperature_for_prompt(user_prompt));
    command.arg("--repeat-penalty").arg("1.1");
    command.env("OMP_NUM_THREADS", threads_count.to_string());
    let mut child = command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| {
            AppError::Message(format!("Failed to start llama-cli runtime: {error}"))
        })?;

    use std::thread;

    let mut stdout = child
        .stdout
        .take()
        .ok_or_else(|| AppError::Message("Failed to open stdout".to_string()))?;
    let mut stderr = child
        .stderr
        .take()
        .ok_or_else(|| AppError::Message("Failed to open stderr".to_string()))?;

    // ── Streaming stdout reader ───────────────────────────────────────────────
    // We read 1 byte at a time so every token is forwarded immediately.
    // A background thread sends raw bytes; the poll loop reassembles UTF-8
    // and emits partial events every 15 ms (or on newline) for low latency.
    let (stdout_tx, stdout_rx) = mpsc::channel::<u8>();
    let stdout_handle = thread::spawn(move || {
        use std::io::Read;
        let mut buf = Vec::new();
        let mut byte = [0u8; 1];
        loop {
            match stdout.read(&mut byte) {
                Ok(0) => break,
                Ok(_) => {
                    buf.push(byte[0]);
                    let _ = stdout_tx.send(byte[0]);
                }
                Err(_) => break,
            }
        }
        Ok::<Vec<u8>, std::io::Error>(buf)
    });

    let stderr_handle = thread::spawn(move || {
        let mut buf = Vec::new();
        stderr.read_to_end(&mut buf).map(|_| buf)
    });

    // Dynamic timeout: allow 10 s per 1 K context tokens, min 120 s, max 3 600 s (1 h).
    let timeout_secs = ((context_size as u64).saturating_mul(10) / 1000)
        .clamp(120, 3_600);

    let started_at = std::time::Instant::now();
    let mut exited = false;
    let mut exit_status = None;
    let mut streamed_stdout = Vec::new();
    let mut last_emitted = String::new();
    let mut last_emit_at: Option<std::time::Instant> = None;

    while started_at.elapsed() < Duration::from_secs(timeout_secs) {
        // Drain all bytes that arrived since last poll
        while let Ok(byte) = stdout_rx.try_recv() {
            streamed_stdout.push(byte);
        }
        if let Some((app, chat_id)) = partial_event.as_ref() {
            let partial_stdout = String::from_utf8_lossy(&streamed_stdout);
            let cleaned = extract_partial_assistant_response(&partial_stdout);
            let new_chars = cleaned.len().saturating_sub(last_emitted.len());
            // Emit if: ≥1 new char AND (≥15 ms since last emit OR newline arrived)
            let emit_ready = last_emit_at
                .map(|t| t.elapsed() >= Duration::from_millis(15))
                .unwrap_or(true);
            let ended_line = cleaned.ends_with('\n') && new_chars > 0;
            if new_chars >= 1 && (emit_ready || ended_line) {
                let _ = app.emit(
                    "chat-generation-partial",
                    GenerationPartialEvent {
                        chat_id: chat_id.clone(),
                        text: cleaned.clone(),
                        elapsed_ms: started_at.elapsed().as_millis() as u64,
                    },
                );
                last_emitted = cleaned;
                last_emit_at = Some(std::time::Instant::now());
            }
        }
        if let Some(status) = child.try_wait().map_err(|error| {
            AppError::Message(format!("Failed to poll llama-cli runtime: {error}"))
        })? {
            exit_status = Some(status);
            exited = true;
            break;
        }
        thread::sleep(Duration::from_millis(8));
    }

    // Drain any remaining bytes after process exit
    while let Ok(byte) = stdout_rx.try_recv() {
        streamed_stdout.push(byte);
    }

    if !exited {
        let _ = child.kill();
        let _ = child.wait();
        return Err(AppError::Message(format!(
            "llama-cli timed out after {timeout_secs}s. \
             Try a smaller GGUF, fewer GPU layers, or reduce context length."
        )));
    }

    let status = exit_status
        .ok_or_else(|| AppError::Message("Could not retrieve exit status".to_string()))?;

    let stdout_bytes = stdout_handle
        .join()
        .map_err(|_| AppError::Message("Stdout thread panicked".to_string()))?
        .map_err(|error| AppError::Message(format!("Failed to read stdout: {error}")))?;

    let stderr_bytes = stderr_handle
        .join()
        .map_err(|_| AppError::Message("Stderr thread panicked".to_string()))?
        .map_err(|error| AppError::Message(format!("Failed to read stderr: {error}")))?;

    let stdout_str = String::from_utf8_lossy(&stdout_bytes);
    let stderr_str = String::from_utf8_lossy(&stderr_bytes);

    if !status.success() {
        let details = stderr_str.trim();
        let details = if details.is_empty() {
            "No stderr was returned. Verify llama-cli can load this model from the Models folder."
        } else {
            details
        };
        return Err(AppError::Message(format!(
            "llama-cli exited with status {status}.\n{details}"
        )));
    }

    let cleaned = extract_assistant_response(&stdout_str, user_prompt, ai_name);
    if !cleaned.is_empty() {
        if let Some((app, chat_id)) = partial_event.as_ref() {
            let _ = app.emit(
                "chat-generation-partial",
                GenerationPartialEvent {
                    chat_id: chat_id.clone(),
                    text: cleaned.clone(),
                    elapsed_ms: started_at.elapsed().as_millis() as u64,
                },
            );
        }
    }

    if cleaned.is_empty() {
        Err(AppError::Message(format!(
            "llama-cli returned no text.\n{}",
            stderr_str.trim()
        )))
    } else {
        Ok(cleaned)
    }
}

#[allow(clippy::too_many_arguments)]
fn run_local_chat_generation(
    runtime_path: &Path,
    storage: &AppStorage,
    model: &ModelRecord,
    system_prompt: &str,
    user_prompt: &str,
    generation_config: Option<GenerationConfig>,
    ai_name: &str,
    partial_event: Option<(AppHandle, String)>,
    chat_id: &str,
) -> AppResult<String> {
    let _generation_guard = local_generation_lock()
        .lock()
        .map_err(|_| AppError::Message("Failed to acquire local generation lock".to_string()))?;
    let prompt_cache_path = prompt_cache_path_for_chat(storage, chat_id, model);
    let cache_path_ref = prompt_cache_path.as_path();
    let attempts: [(Option<&Path>, bool); 3] =
        [(Some(cache_path_ref), false), (None, false), (None, true)];

    let mut last_error: Option<AppError> = None;
    for (attempt_index, (prompt_cache, force_cpu)) in attempts.into_iter().enumerate() {
        append_runtime_log(
            storage,
            &format!(
                "Chat runtime attempt {} for {} (cache={}, cpu_only={})",
                attempt_index + 1,
                model.name,
                prompt_cache.is_some(),
                force_cpu
            ),
        );
        let result = run_llama_cli_runtime(
            runtime_path,
            model,
            system_prompt,
            user_prompt,
            generation_config.clone(),
            ai_name,
            partial_event.clone(),
            prompt_cache,
            force_cpu,
        );
        match result {
            Ok(response) => return Ok(response),
            Err(error) => {
                append_runtime_log(
                    storage,
                    &format!("Chat runtime attempt {} failed: {error}", attempt_index + 1),
                );
                if error.to_string().to_lowercase().contains("timed out") {
                    return Err(error);
                }
                last_error = Some(error);
            }
        }
    }

    Err(last_error.unwrap_or_else(|| AppError::Message("Local GGUF generation failed".to_string())))
}

#[tauri::command]
fn get_app_state(app: AppHandle, state: State<'_, AppSession>) -> Result<AppSnapshot, String> {
    let guard = session(&state).map_err(|error| error.to_string())?;
    read_snapshot(&app, &guard).map_err(|error| error.to_string())
}

#[tauri::command]
fn initialize_setup(
    app: AppHandle,
    state: State<'_, AppSession>,
    payload: SetupPayload,
) -> Result<AppSnapshot, String> {
    let (_, storage) = app_storage(&app).map_err(|error| error.to_string())?;
    let mut guard = session(&state).map_err(|error| error.to_string())?;
    let username = payload.username.trim().to_string();
    let master_key = storage
        .save_security(&username, &payload.password)
        .map_err(|error| error.to_string())?;
    let now = Utc::now().to_rfc3339();
    let profile_avatar = payload
        .profile_avatar_base64
        .as_deref()
        .map(|value| storage.import_avatar(value, "profile"))
        .transpose()
        .map_err(|error| error.to_string())?;
    let character_avatar = payload
        .character_avatar_base64
        .as_deref()
        .map(|value| storage.import_avatar(value, "character"))
        .transpose()
        .map_err(|error| error.to_string())?;

    let profile = ProfileRecord {
        id: Uuid::new_v4().to_string(),
        display_name: payload.display_name,
        avatar_path: profile_avatar,
        created_at: now.clone(),
        updated_at: now.clone(),
    };
    let character = CharacterRecord {
        id: Uuid::new_v4().to_string(),
        name: payload.character_name.clone(),
        description: payload.character_description,
        style_notes: payload.character_style_notes,
        avatar_path: character_avatar,
        default_model_id: None,
        created_at: now.clone(),
        updated_at: now.clone(),
    };
    storage
        .save_profile(&username, &profile)
        .map_err(|error| error.to_string())?;
    storage
        .save_character(&username, &character)
        .map_err(|error| error.to_string())?;
    let initial_chat = storage
        .create_initial_chat(&username, &master_key, &payload.character_name)
        .map_err(|error| error.to_string())?;
    guard.master_key = Some(master_key);
    guard.username = Some(username);
    guard.active_chat_id = Some(initial_chat.id);
    read_snapshot(&app, &guard).map_err(|error| error.to_string())
}

#[tauri::command]
fn unlock_with_password(
    app: AppHandle,
    state: State<'_, AppSession>,
    username: String,
    password: String,
) -> Result<AppSnapshot, String> {
    let (_, storage) = app_storage(&app).map_err(|error| error.to_string())?;
    let master_key = storage
        .unlock_security(&username, &password)
        .map_err(|error| error.to_string())?;
    let mut guard = session(&state).map_err(|error| error.to_string())?;
    let saved_chat_id = guard.active_chat_id.clone();
    guard.master_key = Some(master_key.clone());
    guard.username = Some(username.clone());

    // Validate saved chat id still exists and isn't archived; if not, use startup logic
    let chats = storage
        .load_chats(&username, &master_key)
        .map_err(|e| e.to_string())?;
    let saved_valid = saved_chat_id
        .as_deref()
        .map(|id| chats.iter().any(|c| c.id == id && !c.archived))
        .unwrap_or(false);

    if !saved_valid {
        guard.active_chat_id = Some(
            select_startup_chat(&storage, &username, &master_key)
                .map_err(|error| error.to_string())?,
        );
    }
    read_snapshot(&app, &guard).map_err(|error| error.to_string())
}

#[tauri::command]
fn lock_app(app: AppHandle, state: State<'_, AppSession>) -> Result<AppSnapshot, String> {
    let mut guard = session(&state).map_err(|error| error.to_string())?;
    // Preserve active_chat_id across lock so unlock restores the same chat
    guard.master_key = None;
    guard.username = None;
    read_snapshot(&app, &guard).map_err(|error| error.to_string())
}

#[tauri::command]
fn save_profile(
    app: AppHandle,
    state: State<'_, AppSession>,
    payload: SaveProfilePayload,
) -> Result<AppSnapshot, String> {
    let guard = require_unlocked(&state).map_err(|error| error.to_string())?;
    let (_, storage) = app_storage(&app).map_err(|error| error.to_string())?;
    let username = guard
        .username
        .as_deref()
        .ok_or_else(|| "No active account".to_string())?;
    let mut profile = storage
        .load_profile(Some(username))
        .map_err(|error| error.to_string())?
        .ok_or_else(|| "Profile not found".to_string())?;
    profile.display_name = payload.display_name;
    profile.updated_at = Utc::now().to_rfc3339();
    if let Some(base64) = payload.avatar_base64.as_deref() {
        profile.avatar_path = Some(
            storage
                .import_avatar(base64, "profile")
                .map_err(|error| error.to_string())?,
        );
    }
    storage
        .save_profile(username, &profile)
        .map_err(|error| error.to_string())?;
    read_snapshot(&app, &guard).map_err(|error| error.to_string())
}

#[tauri::command]
fn save_character(
    app: AppHandle,
    state: State<'_, AppSession>,
    payload: SaveCharacterPayload,
) -> Result<AppSnapshot, String> {
    let guard = require_unlocked(&state).map_err(|error| error.to_string())?;
    let (_, storage) = app_storage(&app).map_err(|error| error.to_string())?;
    let username = guard
        .username
        .as_deref()
        .ok_or_else(|| "No active account".to_string())?;
    let mut character = storage
        .load_character(Some(username))
        .map_err(|error| error.to_string())?
        .ok_or_else(|| "Character not found".to_string())?;
    character.name = payload.name;
    character.description = payload.description;
    character.style_notes = payload.style_notes;
    character.updated_at = Utc::now().to_rfc3339();
    if let Some(base64) = payload.avatar_base64.as_deref() {
        character.avatar_path = Some(
            storage
                .import_avatar(base64, "character")
                .map_err(|error| error.to_string())?,
        );
    }
    storage
        .save_character(username, &character)
        .map_err(|error| error.to_string())?;
    read_snapshot(&app, &guard).map_err(|error| error.to_string())
}

#[tauri::command]
fn create_chat(
    app: AppHandle,
    state: State<'_, AppSession>,
    title: Option<String>,
) -> Result<AppSnapshot, String> {
    let mut guard = require_unlocked(&state).map_err(|error| error.to_string())?;
    let (_, storage) = app_storage(&app).map_err(|error| error.to_string())?;
    let master_key = guard
        .master_key
        .clone()
        .ok_or_else(|| "App is locked".to_string())?;
    let username = guard
        .username
        .clone()
        .ok_or_else(|| "No active account".to_string())?;
    let chat = create_blank_chat(
        &storage,
        &username,
        &master_key,
        &title.unwrap_or_else(|| "New chat".to_string()),
    )
    .map_err(|error| error.to_string())?;
    guard.active_chat_id = Some(chat.id);
    read_snapshot(&app, &guard).map_err(|error| error.to_string())
}

#[tauri::command]
fn select_chat(
    app: AppHandle,
    state: State<'_, AppSession>,
    chat_id: String,
) -> Result<AppSnapshot, String> {
    let mut guard = require_unlocked(&state).map_err(|error| error.to_string())?;
    guard.active_chat_id = Some(chat_id);
    read_snapshot(&app, &guard).map_err(|error| error.to_string())
}

struct ProviderChatGenerationContext<'a> {
    app: &'a AppHandle,
    chat_id: &'a str,
    secrets: &'a ProviderSecrets,
    system_prompt: &'a str,
    user_prompt: &'a str,
    max_tokens: u32,
    temperature: f32,
}

async fn run_provider_chat_generation(
    provider: ProviderKind,
    ctx: ProviderChatGenerationContext<'_>,
) -> Result<String, String> {
    let partial_app = ctx.app.clone();
    let partial_chat_id = ctx.chat_id.to_string();
    let started = std::time::Instant::now();
    let streaming = generate_provider_reply_streaming(
        provider,
        ctx.secrets,
        ctx.system_prompt,
        ctx.user_prompt,
        ctx.max_tokens,
        ctx.temperature,
        move |partial| {
            if partial.trim().is_empty() {
                return;
            }
            let _ = partial_app.emit(
                "chat-generation-partial",
                GenerationPartialEvent {
                    chat_id: partial_chat_id.clone(),
                    text: partial,
                    elapsed_ms: started.elapsed().as_millis() as u64,
                },
            );
        },
    )
    .await;

    match streaming {
        Ok(response) => Ok(response),
        Err(stream_error) => generate_provider_reply(
            provider,
            ctx.secrets,
            ctx.system_prompt,
            ctx.user_prompt,
            ctx.max_tokens,
            ctx.temperature,
        )
        .await
        .map_err(|retry_error| {
            format!(
                "Streaming provider request failed: {stream_error}. Non-streaming retry failed: {retry_error}"
            )
        }),
    }
}

#[allow(clippy::too_many_arguments)]
async fn generate_assistant_text(
    app: AppHandle,
    storage: AppStorage,
    snapshot: AppSnapshot,
    active_chat: Option<ChatRecord>,
    character: Option<CharacterRecord>,
    chat_id: String,
    text: String,
    provider_secrets: ProviderSecrets,
) -> String {
    let file_context_count = active_chat
        .as_ref()
        .map(|chat| {
            chat.messages
                .iter()
                .map(|message| message.attachments.len())
                .sum::<usize>()
        })
        .unwrap_or_default();
    let selected_model = resolve_chat_model(&snapshot, active_chat.as_ref()).cloned();
    let prompt = build_chat_prompt_parts(active_chat.as_ref(), character.as_ref(), &text);
    let worker_storage = AppStorage::new(storage.resolved.clone());
    let worker_storage_for_blocking = AppStorage::new(storage.resolved.clone());
    let prompt_system_prompt = prompt.system_prompt.clone();
    let prompt_user_prompt = prompt.user_prompt.clone();
    let max_tokens = snapshot
        .config
        .generation
        .as_ref()
        .and_then(|generation| generation.max_tokens)
        .unwrap_or_else(|| minimum_output_tokens_for_prompt(&text));
    let temperature = response_temperature_for_prompt(&text)
        .parse::<f32>()
        .unwrap_or(0.7);
    let fallback_provider = if snapshot.config.privacy.remote_providers
        && !selected_model
            .as_ref()
            .map(|model| model.id.starts_with("provider:"))
            .unwrap_or(false)
    {
        preferred_provider_kind(&provider_secrets)
    } else {
        None
    };

    if let Some(model) = selected_model {
        let model_name = model.name.clone();
        if let Some(provider) = provider_from_model_id(&model.id) {
            append_runtime_log(
                &worker_storage,
                &format!(
                    "Using provider model: {} ({})",
                    provider.label(),
                    provider.default_model()
                ),
            );
            match run_provider_chat_generation(
                provider,
                ProviderChatGenerationContext {
                    app: &app,
                    chat_id: &chat_id,
                    secrets: &provider_secrets,
                    system_prompt: &prompt.system_prompt,
                    user_prompt: &prompt.user_prompt,
                    max_tokens,
                    temperature,
                },
            )
            .await
            {
                Ok(response) => {
                    append_runtime_log(
                        &worker_storage,
                        &format!(
                            "Provider generation completed: {} characters",
                            response.len()
                        ),
                    );
                    response
                }
                Err(error) => {
                    append_runtime_log(
                        &worker_storage,
                        &format!("Provider generation failed: {error}"),
                    );
                    format!(
                        "{} could not generate a response.\n\n{}\n\nModel: {}\nContext items available: {}",
                        provider.label(),
                        error,
                        model_name,
                        file_context_count
                    )
                }
            }
        } else {
            let runtime_path = find_chat_cli_runtime(&storage.resolved.runtimes_dir);
            let gen_config = snapshot.config.generation.clone();
            let partial_app = app.clone();
            let partial_chat_id = chat_id.clone();
            let chat_id_for_generation = chat_id.clone();
            let ai_name = active_chat
                .as_ref()
                .and_then(|chat| chat.settings.ai_name.clone())
                .or_else(|| character.as_ref().map(|value| value.name.clone()))
                .unwrap_or_else(|| "Assistant".to_string());
            let ai_name_clone = ai_name.clone();
            let generation_result = match tauri::async_runtime::spawn_blocking(move || {
                let result = if let Some(runtime_path) = runtime_path {
                    append_runtime_log(
                        &worker_storage_for_blocking,
                        &format!("Using chat runtime: {}", runtime_path.to_string_lossy()),
                    );
                    run_local_chat_generation(
                        &runtime_path,
                        &worker_storage_for_blocking,
                        &model,
                        &prompt_system_prompt,
                        &prompt_user_prompt,
                        gen_config,
                        &ai_name_clone,
                        Some((partial_app, partial_chat_id)),
                        &chat_id_for_generation,
                    )
                } else {
                    let message = "No isolated chat runtime was found. Put llama-cli.exe in Runtimes/Chat to run GGUF models safely. The in-process embedded runtime is disabled because native GGUF crashes can close the whole app.";
                    append_runtime_log(&worker_storage_for_blocking, message);
                    Err(AppError::Message(message.to_string()))
                };

                if let Err(error) = &result {
                    append_runtime_log(
                        &worker_storage_for_blocking,
                        &format!("Chat generation failed: {error}"),
                    );
                }
                if let Ok(response) = &result {
                    append_runtime_log(
                        &worker_storage_for_blocking,
                        &format!("Chat generation completed: {} characters", response.len()),
                    );
                }
                result.map_err(|error| error.to_string())
            })
            .await
            {
                Ok(result) => result,
                Err(error) => Err(format!("Local GGUF generation worker failed: {error}")),
            };

            match generation_result {
                Ok(response) => response,
                Err(error) => {
                    format!(
                        "The selected GGUF model was found, but the local runtime could not generate a response.\n\n{}\n\nModel: {}\nContext items available: {}\nRuntime log: Data/Logs/runtime.log",
                        error,
                        model_name,
                        file_context_count
                    )
                }
            }
        }
    } else if let Some(model_id) = active_chat
        .as_ref()
        .and_then(|chat| chat.settings.model_id.as_deref())
    {
        if let Some(provider) = fallback_provider {
            match run_provider_chat_generation(
                provider,
                ProviderChatGenerationContext {
                    app: &app,
                    chat_id: &chat_id,
                    secrets: &provider_secrets,
                    system_prompt: &prompt.system_prompt,
                    user_prompt: &prompt.user_prompt,
                    max_tokens,
                    temperature,
                },
            )
            .await
            {
                Ok(response) => response,
                Err(error) => format!(
                    "{} could not generate a response.\n\n{}\n\nModel: {}\nContext items available: {}",
                    provider.label(),
                    error,
                    model_id,
                    file_context_count
                ),
            }
        } else {
            format!(
                "The selected model ({}) is not currently available. Re-enable the saved provider key or choose a different model.",
                model_id
            )
        }
    } else {
        if let Some(provider) = fallback_provider {
            match run_provider_chat_generation(
                provider,
                ProviderChatGenerationContext {
                    app: &app,
                    chat_id: &chat_id,
                    secrets: &provider_secrets,
                    system_prompt: &prompt.system_prompt,
                    user_prompt: &prompt.user_prompt,
                    max_tokens,
                    temperature,
                },
            )
            .await
            {
                Ok(response) => response,
                Err(error) => format!(
                    "{} could not generate a response.\n\n{}\n\nContext items available: {}",
                    provider.label(),
                    error,
                    file_context_count
                ),
            }
        } else {
            format!(
                "No local GGUF chat model is selected or detected. Add one or more .gguf files to Data/Models or to a Models folder beside the app executable, rescan, and select the model in this chat. No cloud fallback was used.\n\nContext items available: {}",
                file_context_count
            )
        }
    }
}

#[tauri::command]
async fn send_chat_message(
    app: AppHandle,
    state: State<'_, AppSession>,
    chat_id: String,
    text: String,
) -> Result<AppSnapshot, String> {
    let text = text.trim().to_string();
    if text.is_empty() {
        return Err("Message is empty".to_string());
    }

    let (master_key, username, active_chat_id) = {
        let guard = require_unlocked(&state).map_err(|error| error.to_string())?;
        (
            guard
                .master_key
                .clone()
                .ok_or_else(|| "App is locked".to_string())?,
            guard
                .username
                .clone()
                .ok_or_else(|| "No active account".to_string())?,
            guard.active_chat_id.clone(),
        )
    };
    let session_for_read = SessionState {
        master_key: Some(master_key.clone()),
        username: Some(username.clone()),
        active_chat_id: active_chat_id.clone(),
    };
    let (_, storage) = app_storage(&app).map_err(|error| error.to_string())?;
    let snapshot = read_snapshot(&app, &session_for_read).map_err(|error| error.to_string())?;
    let character = snapshot.character.clone();
    let active_chat = snapshot
        .chats
        .iter()
        .find(|chat| chat.id == chat_id)
        .cloned();
    let provider_secrets = storage
        .load_provider_secrets(&username, &master_key)
        .map_err(|error| error.to_string())?;

    let session_for_write = SessionState {
        master_key: Some(master_key),
        username: Some(username),
        active_chat_id,
    };
    persist_chat_update(&app, &session_for_write, &chat_id, |chat| {
        chat.messages.push(ChatMessage {
            id: Uuid::new_v4().to_string(),
            role: "user".to_string(),
            text: text.clone(),
            created_at: Utc::now().to_rfc3339(),
            pinned: false,
            attachments: Vec::new(),
        });
        chat.updated_at = Utc::now().to_rfc3339();
    })
    .map_err(|error| error.to_string())?;

    let assistant_text = generate_assistant_text(
        app.clone(),
        storage,
        snapshot,
        active_chat,
        character,
        chat_id.clone(),
        text,
        provider_secrets,
    )
    .await;

    persist_chat_update(&app, &session_for_write, &chat_id, |chat| {
        chat.messages.push(ChatMessage {
            id: Uuid::new_v4().to_string(),
            role: "assistant".to_string(),
            text: assistant_text,
            created_at: Utc::now().to_rfc3339(),
            pinned: false,
            attachments: Vec::new(),
        });
        chat.updated_at = Utc::now().to_rfc3339();
    })
    .map_err(|error| error.to_string())?;

    let current_active_chat_id = {
        if let Ok(guard) = session(&state) {
            guard.active_chat_id.clone()
        } else {
            session_for_write.active_chat_id.clone()
        }
    };
    let mut final_session = session_for_write;
    final_session.active_chat_id = current_active_chat_id;
    read_snapshot(&app, &final_session).map_err(|error| error.to_string())
}


#[tauri::command]
async fn send_incognito_chat_message(
    app: AppHandle,
    state: State<'_, AppSession>,
    chat_id: String,
    text: String,
) -> Result<ChatMessage, String> {
    let text = text.trim().to_string();
    if text.is_empty() {
        return Err("Message is empty".to_string());
    }

    let (master_key, username, active_chat_id) = {
        let guard = require_unlocked(&state).map_err(|error| error.to_string())?;
        (
            guard
                .master_key
                .clone()
                .ok_or_else(|| "App is locked".to_string())?,
            guard
                .username
                .clone()
                .ok_or_else(|| "No active account".to_string())?,
            guard.active_chat_id.clone(),
        )
    };
    let session_for_read = SessionState {
        master_key: Some(master_key),
        username: Some(username),
        active_chat_id,
    };
    let (_, storage) = app_storage(&app).map_err(|error| error.to_string())?;
    let snapshot = read_snapshot(&app, &session_for_read).map_err(|error| error.to_string())?;
    let character = snapshot.character.clone();
    let active_chat = snapshot
        .chats
        .iter()
        .find(|chat| chat.id == chat_id)
        .cloned();
    let provider_secrets = storage
        .load_provider_secrets(
            session_for_read
                .username
                .as_deref()
                .ok_or_else(|| "No active account".to_string())?,
            session_for_read
                .master_key
                .as_deref()
                .ok_or_else(|| "App is locked".to_string())?,
        )
        .map_err(|error| error.to_string())?;
    let assistant_text = generate_assistant_text(
        app,
        storage,
        snapshot,
        active_chat,
        character,
        chat_id,
        text,
        provider_secrets,
    )
    .await;

    Ok(ChatMessage {
        id: format!("incognito-assistant-{}", Uuid::new_v4()),
        role: "assistant".to_string(),
        text: assistant_text,
        created_at: Utc::now().to_rfc3339(),
        pinned: false,
        attachments: Vec::new(),
    })
}

#[tauri::command]
fn rename_chat(
    app: AppHandle,
    state: State<'_, AppSession>,
    chat_id: String,
    title: String,
) -> Result<AppSnapshot, String> {
    let guard = require_unlocked(&state).map_err(|error| error.to_string())?;
    persist_chat_update(&app, &guard, &chat_id, |chat| {
        chat.title = title;
        chat.updated_at = Utc::now().to_rfc3339();
    })
    .map_err(|error| error.to_string())?;
    read_snapshot(&app, &guard).map_err(|error| error.to_string())
}

#[tauri::command]
fn toggle_chat_flag(
    app: AppHandle,
    state: State<'_, AppSession>,
    chat_id: String,
    field: String,
) -> Result<AppSnapshot, String> {
    let guard = require_unlocked(&state).map_err(|error| error.to_string())?;
    persist_chat_update(&app, &guard, &chat_id, |chat| {
        match field.as_str() {
            "favorite" => chat.favorite = !chat.favorite,
            "archived" => chat.archived = !chat.archived,
            _ => {}
        }
        chat.updated_at = Utc::now().to_rfc3339();
    })
    .map_err(|error| error.to_string())?;
    read_snapshot(&app, &guard).map_err(|error| error.to_string())
}

#[tauri::command]
fn clear_chat_messages(
    app: AppHandle,
    state: State<'_, AppSession>,
    chat_id: String,
) -> Result<AppSnapshot, String> {
    let guard = require_unlocked(&state).map_err(|error| error.to_string())?;
    persist_chat_update(&app, &guard, &chat_id, |chat| {
        chat.messages.clear();
        chat.updated_at = Utc::now().to_rfc3339();
    })
    .map_err(|error| error.to_string())?;
    read_snapshot(&app, &guard).map_err(|error| error.to_string())
}

#[tauri::command]
fn delete_chat(
    app: AppHandle,
    state: State<'_, AppSession>,
    chat_id: String,
) -> Result<AppSnapshot, String> {
    let mut guard = require_unlocked(&state).map_err(|error| error.to_string())?;
    let (_, storage) = app_storage(&app).map_err(|error| error.to_string())?;
    let master_key = guard
        .master_key
        .clone()
        .ok_or_else(|| "App is locked".to_string())?;
    let username = guard
        .username
        .clone()
        .ok_or_else(|| "No active account".to_string())?;
    storage
        .delete_chat(&username, &chat_id)
        .map_err(|error| error.to_string())?;

    let remaining = storage
        .load_chats(&username, &master_key)
        .map_err(|error| error.to_string())?;
    if let Some(next_chat) = remaining.first() {
        guard.active_chat_id = Some(next_chat.id.clone());
    } else {
        let chat = create_blank_chat(&storage, &username, &master_key, "New chat")
            .map_err(|error| error.to_string())?;
        guard.active_chat_id = Some(chat.id);
    }

    read_snapshot(&app, &guard).map_err(|error| error.to_string())
}

#[tauri::command]
fn set_chat_settings(
    app: AppHandle,
    state: State<'_, AppSession>,
    payload: ChatSettingsUpdatePayload,
) -> Result<AppSnapshot, String> {
    let guard = require_unlocked(&state).map_err(|error| error.to_string())?;
    let (_, storage) = app_storage(&app).map_err(|error| error.to_string())?;

    let user_avatar = if let Some(base64) = payload.user_avatar_base64.as_ref() {
        Some(
            storage
                .import_avatar(base64, "chat_user")
                .map_err(|error| error.to_string())?,
        )
    } else {
        payload.user_avatar_path.clone()
    };

    let ai_avatar = if let Some(base64) = payload.ai_avatar_base64.as_ref() {
        Some(
            storage
                .import_avatar(base64, "chat_ai")
                .map_err(|error| error.to_string())?,
        )
    } else {
        payload.ai_avatar_path.clone()
    };

    persist_chat_update(&app, &guard, &payload.chat_id, |chat| {
        chat.settings.model_id = payload.model_id.clone();
        chat.settings.image_model_id = payload.image_model_id.clone();
        chat.settings.video_model_id = payload.video_model_id.clone();
        chat.settings.profile_override_id = payload.profile_override_id.clone();
        chat.settings.user_name = payload.user_name.clone();
        chat.settings.ai_name = payload.ai_name.clone();
        chat.settings.ai_instructions = payload.ai_instructions.clone();
        chat.settings.user_avatar = user_avatar;
        chat.settings.ai_avatar = ai_avatar;
        // Only update folder path if explicitly provided (None = don't touch)
        if payload.context_folder_path.is_some() {
            chat.settings.context_folder_path = payload.context_folder_path.clone();
        }
        chat.updated_at = Utc::now().to_rfc3339();
    })
    .map_err(|error| error.to_string())?;
    read_snapshot(&app, &guard).map_err(|error| error.to_string())
}

#[tauri::command]
fn set_chat_folder(
    app: AppHandle,
    state: State<'_, AppSession>,
    chat_id: String,
    folder_path: String,
) -> Result<AppSnapshot, String> {
    let guard = require_unlocked(&state).map_err(|error| error.to_string())?;
    // Validate the path is a real directory
    let path = Path::new(&folder_path);
    if !path.is_dir() {
        return Err(format!("Path is not a directory: {folder_path}"));
    }
    persist_chat_update(&app, &guard, &chat_id, |chat| {
        chat.settings.context_folder_path = Some(folder_path.clone());
        chat.updated_at = Utc::now().to_rfc3339();
    })
    .map_err(|error| error.to_string())?;
    read_snapshot(&app, &guard).map_err(|error| error.to_string())
}

#[tauri::command]
fn clear_chat_folder(
    app: AppHandle,
    state: State<'_, AppSession>,
    chat_id: String,
) -> Result<AppSnapshot, String> {
    let guard = require_unlocked(&state).map_err(|error| error.to_string())?;
    persist_chat_update(&app, &guard, &chat_id, |chat| {
        chat.settings.context_folder_path = None;
        chat.updated_at = Utc::now().to_rfc3339();
    })
    .map_err(|error| error.to_string())?;
    read_snapshot(&app, &guard).map_err(|error| error.to_string())
}

#[tauri::command]
fn read_chat_folder(
    state: State<'_, AppSession>,
    folder_path: String,
) -> Result<FolderContext, String> {
    let _guard = require_unlocked(&state).map_err(|error| error.to_string())?;
    let path = Path::new(&folder_path);
    if !path.is_dir() {
        return Err(format!("Not a directory: {folder_path}"));
    }
    Ok(scan_folder(path, 524_288_000))
}

#[tauri::command]
fn apply_chat_tools(
    app: AppHandle,
    state: State<'_, AppSession>,
    chat_id: String,
    message_id: String,
) -> Result<AppSnapshot, String> {
    let guard = require_unlocked(&state).map_err(|error| error.to_string())?;

    // We must read the snapshot to find the message text and the folder path
    let snapshot = read_snapshot(&app, &guard).map_err(|error| error.to_string())?;
    let active_chat = snapshot
        .chats
        .iter()
        .find(|chat| chat.id == chat_id)
        .ok_or_else(|| "Chat not found".to_string())?;
    
    let message = active_chat
        .messages
        .iter()
        .find(|m| m.id == message_id)
        .ok_or_else(|| "Message not found".to_string())?;

    let folder_root_opt = active_chat.settings.context_folder_path.clone();
    
    let (_, tool_calls) = parse_tool_calls(&message.text);
    if tool_calls.is_empty() {
        return Err("No tool calls found in message".to_string());
    }

    let tool_results: Vec<ToolCallResult> = if let Some(ref folder_root) = folder_root_opt {
        execute_tool_calls(Path::new(folder_root), &tool_calls)
    } else {
        tool_calls.iter().map(|tc| ToolCallResult {
            action: tc.action.clone(),
            path: tc.path.clone(),
            success: false,
            message: "No workspace folder is attached to this chat.".to_string(),
            content: None,
        }).collect()
    };

    persist_chat_update(&app, &guard, &chat_id, |chat| {
        let result_lines: Vec<String> = tool_results.iter().map(|r| {
            let icon = if r.success { "✅" } else { "❌" };
            let action_label = match r.action.as_str() {
                "write_file" | "create_file" | "edit_file" => "Wrote",
                "delete_file" => "Deleted",
                "read_file" => "Read",
                other => other,
            };
            format!("{icon} **{action_label}** `{}` — {}", r.path, r.message)
        }).collect();
        chat.messages.push(ChatMessage {
            id: Uuid::new_v4().to_string(),
            role: "tool".to_string(),
            text: result_lines.join("\n"),
            created_at: Utc::now().to_rfc3339(),
            pinned: false,
            attachments: Vec::new(),
        });
        chat.updated_at = Utc::now().to_rfc3339();
    })
    .map_err(|error| error.to_string())?;

    read_snapshot(&app, &guard).map_err(|error| error.to_string())
}

#[tauri::command]
fn delete_message(
    app: AppHandle,
    state: State<'_, AppSession>,
    chat_id: String,
    message_id: String,
) -> Result<AppSnapshot, String> {
    let guard = require_unlocked(&state).map_err(|error| error.to_string())?;
    persist_chat_update(&app, &guard, &chat_id, |chat| {
        chat.messages.retain(|message| message.id != message_id);
        chat.updated_at = Utc::now().to_rfc3339();
    })
    .map_err(|error| error.to_string())?;
    read_snapshot(&app, &guard).map_err(|error| error.to_string())
}

#[tauri::command]
fn toggle_pinned_message(
    app: AppHandle,
    state: State<'_, AppSession>,
    chat_id: String,
    message_id: String,
) -> Result<AppSnapshot, String> {
    let guard = require_unlocked(&state).map_err(|error| error.to_string())?;
    persist_chat_update(&app, &guard, &chat_id, |chat| {
        if let Some(message) = chat
            .messages
            .iter_mut()
            .find(|message| message.id == message_id)
        {
            message.pinned = !message.pinned;
        }
        chat.updated_at = Utc::now().to_rfc3339();
    })
    .map_err(|error| error.to_string())?;
    read_snapshot(&app, &guard).map_err(|error| error.to_string())
}

#[tauri::command]
fn update_message(
    app: AppHandle,
    state: State<'_, AppSession>,
    chat_id: String,
    message_id: String,
    text: String,
) -> Result<AppSnapshot, String> {
    let guard = require_unlocked(&state).map_err(|error| error.to_string())?;
    persist_chat_update(&app, &guard, &chat_id, |chat| {
        if let Some(message) = chat
            .messages
            .iter_mut()
            .find(|message| message.id == message_id)
        {
            message.text = text;
        }
        chat.updated_at = Utc::now().to_rfc3339();
    })
    .map_err(|error| error.to_string())?;
    read_snapshot(&app, &guard).map_err(|error| error.to_string())
}

#[tauri::command]
fn import_media_attachment(
    app: AppHandle,
    state: State<'_, AppSession>,
    chat_id: String,
    file_name: String,
    base64: String,
    kind: String,
) -> Result<AppSnapshot, String> {
    let guard = require_unlocked(&state).map_err(|error| error.to_string())?;
    let (_, storage) = app_storage(&app).map_err(|error| error.to_string())?;
    let imported = storage
        .import_media_attachment(&base64, &file_name, &kind)
        .map_err(|error| error.to_string())?;
    let content_preview = storage.build_attachment_preview(&imported, &file_name, &kind);
    persist_chat_update(&app, &guard, &chat_id, |chat| {
        chat.messages.push(ChatMessage {
            id: Uuid::new_v4().to_string(),
            role: "media".to_string(),
            text: format!("Attached local {kind}: {file_name}"),
            created_at: Utc::now().to_rfc3339(),
            pinned: false,
            attachments: vec![MediaAsset {
                id: Uuid::new_v4().to_string(),
                kind: kind.clone(),
                path: imported.clone(),
                thumbnail_path: None,
                content_preview: content_preview.clone(),
                prompt: None,
                status: Some("ready".to_string()),
                model_used: None,
                seed: None,
                source_label: Some(file_name.clone()),
            }],
        });
        chat.updated_at = Utc::now().to_rfc3339();
    })
    .map_err(|error| error.to_string())?;
    read_snapshot(&app, &guard).map_err(|error| error.to_string())
}

#[tauri::command]
fn build_media_prompt_from_chat(
    app: AppHandle,
    state: State<'_, AppSession>,
    chat_id: String,
    user_prompt: String,
) -> Result<MediaPromptPreview, String> {
    let guard = require_unlocked(&state).map_err(|error| error.to_string())?;
    let (_, storage) = app_storage(&app).map_err(|error| error.to_string())?;
    let character = storage
        .load_character(guard.username.as_deref())
        .map_err(|error| error.to_string())?;
    let master_key = guard
        .master_key
        .as_ref()
        .ok_or_else(|| "App is locked".to_string())?;
    let username = guard
        .username
        .as_deref()
        .ok_or_else(|| "No active account".to_string())?;
    let chats = storage
        .load_chats(username, master_key)
        .map_err(|error| error.to_string())?;
    let chat = chats
        .iter()
        .find(|chat| chat.id == chat_id)
        .ok_or_else(|| "Chat not found".to_string())?;
    Ok(storage.build_media_prompt(chat, character.as_ref(), &user_prompt))
}

#[tauri::command]
async fn generate_image_from_chat(
    app: AppHandle,
    state: State<'_, AppSession>,
    chat_id: String,
    prompt: String,
) -> Result<AppSnapshot, String> {
    generate_media_from_chat(app, state, chat_id, prompt, "image").await
}

#[tauri::command]
async fn generate_video_from_chat(
    app: AppHandle,
    state: State<'_, AppSession>,
    chat_id: String,
    prompt: String,
) -> Result<AppSnapshot, String> {
    generate_media_from_chat(app, state, chat_id, prompt, "video").await
}

struct ProviderMediaGenerationContext<'a> {
    app: &'a AppHandle,
    worker_storage: &'a AppStorage,
    secrets: &'a ProviderSecrets,
    explicit_request: &'a str,
    kind: &'static str,
    chat_id: &'a str,
    output_path: &'a Path,
}

async fn run_provider_media_generation(
    provider: ProviderKind,
    ctx: ProviderMediaGenerationContext<'_>,
) -> Result<(PathBuf, String, String, Option<u64>), String> {
    if ctx.kind == "image" {
        append_runtime_log(
            ctx.worker_storage,
            &format!(
                "Using provider image generator: {} (gpt-image-1)",
                provider.label()
            ),
        );
        let image_bytes =
            generate_provider_image(provider, ctx.secrets, ctx.explicit_request, 1024, 1024)
                .await
                .map_err(|error| error.to_string())?;
        fs::write(ctx.output_path, image_bytes).map_err(|error| error.to_string())?;
        append_runtime_log(
            ctx.worker_storage,
            &format!(
                "Provider image generation completed: {}",
                ctx.output_path.to_string_lossy()
            ),
        );
        Ok((
            ctx.output_path.to_path_buf(),
            format!("{} GPT Image", provider.label()),
            format!("provider:{}:gpt-image-1", provider.id()),
            None,
        ))
    } else {
        append_runtime_log(
            ctx.worker_storage,
            &format!(
                "Using provider video generator: {} (sora-2)",
                provider.label()
            ),
        );
        let progress_started = std::time::Instant::now();
        let progress_app = ctx.app.clone();
        let progress_chat_id = ctx.chat_id.to_string();
        let progress_kind = ctx.kind.to_string();
        let progress_provider = provider.label().to_string();
        let video_bytes = generate_provider_video(
            provider,
            ctx.secrets,
            ctx.explicit_request,
            move |status, progress| {
                let _ = progress_app.emit(
                    "media-generation-progress",
                    MediaGenerationProgressEvent {
                        chat_id: progress_chat_id.clone(),
                        kind: progress_kind.clone(),
                        provider: progress_provider.clone(),
                        status,
                        progress,
                        elapsed_ms: progress_started.elapsed().as_millis() as u64,
                    },
                );
            },
        )
        .await
        .map_err(|error| error.to_string())?;
        fs::write(ctx.output_path, video_bytes).map_err(|error| error.to_string())?;
        append_runtime_log(
            ctx.worker_storage,
            &format!(
                "Provider video generation completed: {}",
                ctx.output_path.to_string_lossy()
            ),
        );
        Ok((
            ctx.output_path.to_path_buf(),
            format!("{} Sora", provider.label()),
            format!("provider:{}:sora-2", provider.id()),
            None,
        ))
    }
}

async fn generate_media_from_chat(
    app: AppHandle,
    state: State<'_, AppSession>,
    chat_id: String,
    prompt: String,
    kind: &'static str,
) -> Result<AppSnapshot, String> {
    let (master_key, username, active_chat_id) = {
        let guard = require_unlocked(&state).map_err(|error| error.to_string())?;
        (
            guard
                .master_key
                .clone()
                .ok_or_else(|| "App is locked".to_string())?,
            guard
                .username
                .clone()
                .ok_or_else(|| "No active account".to_string())?,
            guard.active_chat_id.clone(),
        )
    };
    let session_for_read = SessionState {
        master_key: Some(master_key.clone()),
        username: Some(username.clone()),
        active_chat_id: active_chat_id.clone(),
    };
    let (_, storage) = app_storage(&app).map_err(|error| error.to_string())?;
    let provider_secrets = storage
        .load_provider_secrets(&username, &master_key)
        .map_err(|error| error.to_string())?;
    let session_for_write = SessionState {
        master_key: Some(master_key),
        username: Some(username),
        active_chat_id,
    };
    let snapshot = read_snapshot(&app, &session_for_read).map_err(|error| error.to_string())?;
    let character = snapshot.character.clone();
    let active_chat = snapshot
        .chats
        .iter()
        .find(|chat| chat.id == chat_id)
        .cloned()
        .ok_or_else(|| "Chat not found".to_string())?;

    let selected_model = if kind == "video" {
        resolve_media_model(
            &snapshot.video_models,
            active_chat.settings.video_model_id.as_deref(),
        )
        .cloned()
    } else {
        resolve_media_model(
            &snapshot.image_models,
            active_chat.settings.image_model_id.as_deref(),
        )
        .cloned()
    };

    let runtime_dir =
        storage
            .resolved
            .runtimes_dir
            .join(if kind == "video" { "Video" } else { "Image" });
    let runtime_path = find_runtime_executable(kind, &runtime_dir);

    let explicit_request = extract_media_request(&prompt);
    let media_prompt =
        storage.build_media_prompt(&active_chat, character.as_ref(), &explicit_request);
    let now = Utc::now();
    let job_id = Uuid::new_v4().to_string();
    let media_jobs_dir = storage.resolved.logs_dir.join("MediaJobs").join(kind);
    fs::create_dir_all(&media_jobs_dir).map_err(|error| error.to_string())?;
    let job_path = media_jobs_dir.join(format!("{job_id}.json"));
    let result_path = media_jobs_dir.join(format!("{job_id}.result.json"));
    let output_dir = if kind == "video" {
        storage.resolved.videos_dir.clone()
    } else {
        storage.resolved.images_dir.clone()
    };
    fs::create_dir_all(&output_dir).map_err(|error| error.to_string())?;
    let local_output_extension = if kind == "video" { "webm" } else { "png" };
    let provider_output_extension = if kind == "video" { "mp4" } else { "png" };
    let local_output_path_hint = output_dir.join(format!(
        "generated-{kind}-{job_id}.{local_output_extension}"
    ));
    let provider_output_path_hint = output_dir.join(format!(
        "generated-{kind}-{job_id}.{provider_output_extension}"
    ));
    let seed = rand::random::<u64>();

    let worker_storage = AppStorage::new(storage.resolved.clone());
    let local_generation = selected_model.clone().zip(runtime_path.clone());
    let provider_generation = if snapshot.config.privacy.remote_providers {
        if kind == "image" {
            preferred_image_provider(&provider_secrets)
        } else if kind == "video" {
            preferred_video_provider(&provider_secrets)
        } else {
            None
        }
    } else {
        None
    };

    let (generated_path, generated_label, model_used, seed_used) = if let Some((
        selected_model,
        runtime_path,
    )) = local_generation
    {
        let job = MediaGenerationJob {
            id: job_id.clone(),
            kind: kind.to_string(),
            chat_id: chat_id.clone(),
            prompt: explicit_request.clone(),
            model_id: selected_model.id.clone(),
            model_name: selected_model.name.clone(),
            model_path: selected_model.path.clone(),
            output_dir: output_dir.to_string_lossy().to_string(),
            output_path_hint: local_output_path_hint.to_string_lossy().to_string(),
            result_path_hint: result_path.to_string_lossy().to_string(),
            seed,
            width: if kind == "video" { 960 } else { 1024 },
            height: if kind == "video" { 540 } else { 1024 },
            frames: if kind == "video" { Some(48) } else { None },
            fps: if kind == "video" { Some(12) } else { None },
            created_at: now.to_rfc3339(),
            sources: media_prompt.sources.clone(),
        };
        write_json(&job_path, &job).map_err(|error| error.to_string())?;

        let worker_storage_for_blocking = AppStorage::new(storage.resolved.clone());
        let runtime_path_for_blocking = runtime_path.clone();
        let job_path_for_blocking = job_path.clone();
        let output_dir_for_blocking = output_dir.clone();
        let job_for_runtime = job.clone();
        let generation_result = match tauri::async_runtime::spawn_blocking(move || {
            let _guard = local_generation_lock()
                .lock()
                .map_err(|_| "Failed to acquire local generation lock".to_string())?;
            append_runtime_log(
                &worker_storage_for_blocking,
                &format!(
                    "Using {kind} runtime: {}",
                    runtime_path_for_blocking.to_string_lossy()
                ),
            );
            run_media_runtime(
                &runtime_path_for_blocking,
                &job_for_runtime,
                &job_path_for_blocking,
                &output_dir_for_blocking,
                kind,
            )
            .map_err(|error| error.to_string())
        })
        .await
        {
            Ok(result) => result,
            Err(error) => Err(format!("Local {kind} generation worker failed: {error}")),
        };

        match generation_result {
            Ok(generated_path) => (
                generated_path,
                selected_model.name.clone(),
                selected_model.id.clone(),
                Some(seed),
            ),
            Err(local_error) => {
                if let Some(provider) = provider_generation {
                    append_runtime_log(
                        &worker_storage,
                        &format!(
                            "Local {kind} generation failed; falling back to {} provider: {local_error}",
                            provider.label()
                        ),
                    );
                    run_provider_media_generation(
                        provider,
                        ProviderMediaGenerationContext {
                            app: &app,
                            worker_storage: &worker_storage,
                            secrets: &provider_secrets,
                            explicit_request: &explicit_request,
                            kind,
                            chat_id: &chat_id,
                            output_path: &provider_output_path_hint,
                        },
                    )
                    .await
                    .map_err(|provider_error| {
                        format!(
                            "Local {kind} generation failed: {local_error}\n\nProvider fallback also failed: {provider_error}"
                        )
                    })?
                } else {
                    return Err(format!("Local {kind} generation failed: {local_error}"));
                }
            }
        }
    } else if let Some(provider) = provider_generation {
        run_provider_media_generation(
            provider,
            ProviderMediaGenerationContext {
                app: &app,
                worker_storage: &worker_storage,
                secrets: &provider_secrets,
                explicit_request: &explicit_request,
                kind,
                chat_id: &chat_id,
                output_path: &provider_output_path_hint,
            },
        )
        .await?
    } else {
        if selected_model.is_none() {
            return Err(format!(
                "No {} model is selected or available. Add a compatible model and rescan.",
                kind
            ));
        }
        Err(format!(
                "No local {kind} runtime was found in {}. Add a compatible executable that can read AI_CHAT_JOB_PATH and write the generated file into AI_CHAT_OUTPUT_DIR, then rescan.",
                runtime_dir.to_string_lossy()
            ))?
    };
    let file_name = generated_path
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| format!("Generated {kind} file name is invalid"))?
        .to_string();
    let imported_path = worker_storage
        .import_generated_media_file(&generated_path, kind, "generated")
        .map_err(|error| error.to_string())?;

    let media_text = format!(
        "Generated {kind} with {}.\n\nPrompt:\n{explicit_request}",
        generated_label
    );
    let imported_path_for_message = imported_path.clone();
    let prompt_for_attachment = explicit_request.clone();
    persist_chat_update(&app, &session_for_write, &chat_id, |chat| {
        chat.messages.push(ChatMessage {
            id: Uuid::new_v4().to_string(),
            role: "media".to_string(),
            text: media_text.clone(),
            created_at: Utc::now().to_rfc3339(),
            pinned: false,
            attachments: vec![MediaAsset {
                id: Uuid::new_v4().to_string(),
                kind: kind.to_string(),
                path: imported_path_for_message.clone(),
                thumbnail_path: if kind == "image" {
                    Some(imported_path_for_message.clone())
                } else {
                    None
                },
                content_preview: Some(prompt_for_attachment.clone()),
                prompt: Some(explicit_request.clone()),
                status: Some("ready".to_string()),
                model_used: Some(model_used.clone()),
                seed: seed_used,
                source_label: Some(file_name.clone()),
            }],
        });
        chat.updated_at = Utc::now().to_rfc3339();
    })
    .map_err(|error| error.to_string())?;

    read_snapshot(&app, &session_for_write).map_err(|error| error.to_string())
}

fn resolve_media_result_path(value: &serde_json::Value, output_dir: &Path) -> Option<PathBuf> {
    match value {
        serde_json::Value::String(text) => {
            let candidate = PathBuf::from(text);
            let candidate = if candidate.is_absolute() {
                candidate
            } else {
                output_dir.join(candidate)
            };
            candidate.exists().then_some(candidate)
        }
        serde_json::Value::Array(items) => items
            .iter()
            .find_map(|item| resolve_media_result_path(item, output_dir)),
        serde_json::Value::Object(map) => {
            for key in [
                "outputPath",
                "output_path",
                "path",
                "file",
                "filePath",
                "file_path",
                "resultPath",
                "result_path",
            ] {
                if let Some(candidate) = map
                    .get(key)
                    .and_then(|value| resolve_media_result_path(value, output_dir))
                {
                    return Some(candidate);
                }
            }
            for key in [
                "outputPaths",
                "output_paths",
                "paths",
                "files",
                "images",
                "videos",
                "outputs",
            ] {
                if let Some(candidate) = map
                    .get(key)
                    .and_then(|value| resolve_media_result_path(value, output_dir))
                {
                    return Some(candidate);
                }
            }
            None
        }
        _ => None,
    }
}

fn run_media_runtime(
    runtime_path: &Path,
    job: &MediaGenerationJob,
    job_path: &Path,
    output_dir: &Path,
    kind: &str,
) -> AppResult<PathBuf> {
    let mut command = Command::new(runtime_path);
    if let Some(runtime_dir) = runtime_path.parent() {
        command.current_dir(runtime_dir);
    }

    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000200);
    }

    command.env("AI_CHAT_JOB_PATH", job_path);
    command.env("AI_CHAT_KIND", kind);
    command.env("AI_CHAT_CHAT_ID", &job.chat_id);
    command.env("AI_CHAT_MODEL_ID", &job.model_id);
    command.env("AI_CHAT_MODEL_NAME", &job.model_name);
    command.env("AI_CHAT_MODEL_PATH", &job.model_path);
    command.env("AI_CHAT_PROMPT", &job.prompt);
    command.env("AI_CHAT_OUTPUT_DIR", output_dir);
    command.env("AI_CHAT_OUTPUT_PATH_HINT", &job.output_path_hint);
    command.env("AI_CHAT_RESULT_PATH_HINT", &job.result_path_hint);
    command.env("AI_CHAT_SEED", job.seed.to_string());
    command.env("AI_CHAT_WIDTH", job.width.to_string());
    command.env("AI_CHAT_HEIGHT", job.height.to_string());
    if let Some(frames) = job.frames {
        command.env("AI_CHAT_FRAMES", frames.to_string());
    }
    if let Some(fps) = job.fps {
        command.env("AI_CHAT_FPS", fps.to_string());
    }

    command.arg("--job").arg(job_path);
    command.arg("--kind").arg(kind);
    command.arg("--model").arg(&job.model_path);
    command.arg("--prompt").arg(&job.prompt);
    command.arg("--output-dir").arg(output_dir);
    command.arg("--output").arg(&job.output_path_hint);
    command.arg("--result").arg(&job.result_path_hint);
    command.arg("--seed").arg(job.seed.to_string());
    command.arg("--width").arg(job.width.to_string());
    command.arg("--height").arg(job.height.to_string());
    if let Some(frames) = job.frames {
        command.arg("--frames").arg(frames.to_string());
    }
    if let Some(fps) = job.fps {
        command.arg("--fps").arg(fps.to_string());
    }

    let mut child = command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| AppError::Message(format!("Failed to start {kind} runtime: {error}")))?;

    use std::io::Read;
    use std::thread;

    let mut stdout = child
        .stdout
        .take()
        .ok_or_else(|| AppError::Message("Failed to open stdout".to_string()))?;
    let mut stderr = child
        .stderr
        .take()
        .ok_or_else(|| AppError::Message("Failed to open stderr".to_string()))?;

    let stdout_handle = thread::spawn(move || {
        let mut buf = Vec::new();
        stdout.read_to_end(&mut buf).map(|_| buf)
    });
    let stderr_handle = thread::spawn(move || {
        let mut buf = Vec::new();
        stderr.read_to_end(&mut buf).map(|_| buf)
    });

    let started_at = std::time::Instant::now();
    let started_wallclock = std::time::SystemTime::now();
    let mut exited = false;
    let mut exit_status = None;
    while started_at.elapsed() < Duration::from_secs(900) {
        if let Some(status) = child
            .try_wait()
            .map_err(|error| AppError::Message(format!("Failed to poll {kind} runtime: {error}")))?
        {
            exit_status = Some(status);
            exited = true;
            break;
        }
        thread::sleep(Duration::from_millis(100));
    }

    if !exited {
        let _ = child.kill();
        let _ = child.wait();
        return Err(AppError::Message(format!(
            "{kind} runtime timed out after 15 minutes. Check the runtime logs and try a smaller model or resolution."
        )));
    }

    let status = exit_status
        .ok_or_else(|| AppError::Message("Could not retrieve exit status".to_string()))?;

    let stdout_bytes = stdout_handle
        .join()
        .map_err(|_| AppError::Message("Stdout thread panicked".to_string()))?
        .map_err(|error| AppError::Message(format!("Failed to read stdout: {error}")))?;
    let stderr_bytes = stderr_handle
        .join()
        .map_err(|_| AppError::Message("Stderr thread panicked".to_string()))?
        .map_err(|error| AppError::Message(format!("Failed to read stderr: {error}")))?;

    let stdout_str = String::from_utf8_lossy(&stdout_bytes);
    let stderr_str = String::from_utf8_lossy(&stderr_bytes);

    if !status.success() {
        let details = stderr_str.trim();
        let details = if details.is_empty() {
            "No stderr was returned. Verify the runtime can load the selected model and write to the output directory."
        } else {
            details
        };
        return Err(AppError::Message(format!(
            "{kind} runtime exited with status {status}.\n{details}"
        )));
    }

    let output_hint = PathBuf::from(&job.output_path_hint);
    if output_hint.exists() {
        return Ok(output_hint);
    }

    let result_hint = PathBuf::from(&job.result_path_hint);
    if result_hint.exists() {
        if let Ok(raw) = fs::read(&result_hint) {
            if let Ok(value) = serde_json::from_slice::<serde_json::Value>(&raw) {
                if let Some(candidate) = resolve_media_result_path(&value, output_dir) {
                    return Ok(candidate);
                }
            }
        }
    }

    if let Some(candidate) = resolve_media_result_path(
        &serde_json::Value::String(stdout_str.trim().to_string()),
        output_dir,
    ) {
        return Ok(candidate);
    }

    if let Some(candidate) = detect_latest_output_file(output_dir, kind, started_wallclock) {
        return Ok(candidate);
    }

    Err(AppError::Message(format!(
        "{kind} runtime completed but no output file was found in {}.\nExpected a result JSON at {} or a media file in the output directory.",
        output_dir.to_string_lossy(),
        result_hint.to_string_lossy()
    )))
}

fn backup_record_from_dir(path: &Path) -> AppResult<BackupRecord> {
    let id = path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("backup")
        .to_string();
    let manifest_path = path.join("backup-manifest.json");
    let manifest = if manifest_path.exists() {
        read_json::<BackupRecord>(&manifest_path).ok()
    } else {
        None
    };
    let metadata = fs::metadata(path)?;
    let created_at = manifest
        .as_ref()
        .map(|record| record.created_at.clone())
        .or_else(|| to_iso_time(&metadata))
        .unwrap_or_else(|| Utc::now().to_rfc3339());
    let files_copied = WalkDir::new(path)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|entry| entry.path().is_file())
        .count() as u64;

    Ok(BackupRecord {
        id,
        path: path.to_string_lossy().to_string(),
        created_at,
        size_bytes: folder_size(path),
        files_copied,
    })
}

fn list_backup_records(backups_dir: &Path) -> AppResult<Vec<BackupRecord>> {
    fs::create_dir_all(backups_dir)?;
    let mut backups = Vec::new();
    for entry in fs::read_dir(backups_dir)? {
        let entry = entry?;
        if entry.path().is_dir() {
            backups.push(backup_record_from_dir(&entry.path())?);
        }
    }
    backups.sort_by(|left, right| right.created_at.cmp(&left.created_at));
    Ok(backups)
}

fn copy_tree(source: &Path, target: &Path) -> AppResult<u64> {
    if !source.exists() {
        return Ok(0);
    }

    let mut copied = 0;
    for entry in WalkDir::new(source) {
        let entry = entry.map_err(|error| {
            AppError::Message(format!(
                "Failed to walk {}: {error}",
                source.to_string_lossy()
            ))
        })?;
        let path = entry.path();
        let relative = path.strip_prefix(source).map_err(|error| {
            AppError::Message(format!(
                "Failed to build backup path for {}: {error}",
                path.to_string_lossy()
            ))
        })?;
        if relative.as_os_str().is_empty() {
            continue;
        }
        let destination = target.join(relative);
        if entry.file_type().is_dir() {
            fs::create_dir_all(&destination)?;
        } else if entry.file_type().is_file() {
            if let Some(parent) = destination.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::copy(path, &destination)?;
            copied += 1;
        }
    }
    Ok(copied)
}

fn create_data_backup(storage: &AppStorage) -> AppResult<BackupRecord> {
    fs::create_dir_all(&storage.resolved.backups_dir)?;
    let created_at = Utc::now();
    let backup_id = format!("backup-{}", created_at.format("%Y%m%d-%H%M%S"));
    let backup_dir = storage.resolved.backups_dir.join(&backup_id);
    fs::create_dir_all(&backup_dir)?;

    let sources = [
        ("Config", &storage.resolved.config_dir),
        ("Profiles", &storage.resolved.profiles_dir),
        ("Characters", &storage.resolved.characters_dir),
        ("Chats", &storage.resolved.chats_dir),
        ("Media", &storage.resolved.media_dir),
        ("Presets", &storage.resolved.presets_dir),
    ];
    let mut files_copied = 0;
    for (name, source) in sources {
        files_copied += copy_tree(source, &backup_dir.join(name))?;
    }

    let mut record = BackupRecord {
        id: backup_id,
        path: backup_dir.to_string_lossy().to_string(),
        created_at: created_at.to_rfc3339(),
        size_bytes: 0,
        files_copied,
    };
    record.size_bytes = folder_size(&backup_dir);
    write_json(&backup_dir.join("backup-manifest.json"), &record)?;
    Ok(record)
}

#[tauri::command]
fn list_backups(app: AppHandle, state: State<'_, AppSession>) -> Result<Vec<BackupRecord>, String> {
    let _guard = session(&state).map_err(|error| error.to_string())?;
    let (_, storage) = app_storage(&app).map_err(|error| error.to_string())?;
    list_backup_records(&storage.resolved.backups_dir).map_err(|error| error.to_string())
}

#[tauri::command]
fn create_backup(app: AppHandle, state: State<'_, AppSession>) -> Result<BackupResult, String> {
    let guard = require_unlocked(&state).map_err(|error| error.to_string())?;
    let (_, storage) = app_storage(&app).map_err(|error| error.to_string())?;
    let backup = create_data_backup(&storage).map_err(|error| error.to_string())?;
    let snapshot = read_snapshot(&app, &guard).map_err(|error| error.to_string())?;
    Ok(BackupResult { snapshot, backup })
}

#[tauri::command]
fn rescan_models(app: AppHandle, state: State<'_, AppSession>) -> Result<AppSnapshot, String> {
    let guard = session(&state).map_err(|error| error.to_string())?;
    read_snapshot(&app, &guard).map_err(|error| error.to_string())
}

#[tauri::command]
fn save_settings(
    app: AppHandle,
    state: State<'_, AppSession>,
    payload: SettingsUpdatePayload,
) -> Result<AppSnapshot, String> {
    let guard = require_unlocked(&state).map_err(|error| error.to_string())?;
    let app_root = detect_app_root(&app).map_err(|error| error.to_string())?;
    let mut config = load_or_create_config(&app_root).map_err(|error| error.to_string())?;
    let username = guard
        .username
        .as_deref()
        .ok_or_else(|| "No active account".to_string())?;
    let master_key = guard
        .master_key
        .as_deref()
        .ok_or_else(|| "App is locked".to_string())?;
    let old_resolved = resolve_paths(&config, &app_root);
    let old_storage = AppStorage::new(old_resolved);
    let mut provider_secrets = old_storage
        .load_provider_secrets(username, master_key)
        .unwrap_or_default();
    config.paths = payload.paths;
    config.appearance.theme_id = payload.appearance.theme_id;
    config.appearance.theme_group = payload.appearance.theme_group;
    config.appearance.smooth = payload.appearance.smooth;
    config.appearance.compact = payload.appearance.compact;
    config.appearance.auto_scroll = payload.appearance.auto_scroll;
    config.appearance.typewriter = payload.appearance.typewriter;
    config.appearance.show_performance = payload.appearance.show_performance;
    config.appearance.wide_messages = payload.appearance.wide_messages;
    config.appearance.technical_mode = payload.appearance.technical_mode;
    config.appearance.scale = payload.appearance.scale.clamp(85, 120);
    config.appearance.message_style = match payload.appearance.message_style.as_str() {
        "bubbles" | "minimal" => payload.appearance.message_style,
        _ => "cards".to_string(),
    };
    config.appearance.sidebar_collapsed = payload.appearance.sidebar_collapsed;
    config.security.auto_lock_minutes = payload.auto_lock_minutes.max(1);
    config.privacy.remote_providers = payload.remote_providers;
    config.generation = payload.generation.map(|mut generation| {
        generation.threads = generation.threads.map(|threads| threads.clamp(1, 128));
        generation.gpu_layers = generation.gpu_layers.map(|layers| layers.min(999));
        generation.max_tokens = generation.max_tokens.map(|tokens| tokens.clamp(512, 8192));
        generation
    });
    let resolved = resolve_paths(&config, &app_root);
    resolved
        .ensure_directories()
        .map_err(|error| error.to_string())?;
    write_json(&resolved.config_dir.join("app-config.json"), &config)
        .map_err(|error| error.to_string())?;
    if let Some(provider_updates) = payload.provider_keys {
        if let Some(openai_key) = provider_updates.openai_key {
            let trimmed = openai_key.trim().to_string();
            provider_secrets.set_key(
                ProviderKind::OpenAI,
                if trimmed.is_empty() {
                    None
                } else {
                    Some(trimmed)
                },
            );
        }
        if let Some(openrouter_key) = provider_updates.openrouter_key {
            let trimmed = openrouter_key.trim().to_string();
            provider_secrets.set_key(
                ProviderKind::OpenRouter,
                if trimmed.is_empty() {
                    None
                } else {
                    Some(trimmed)
                },
            );
        }
        if let Some(gemini_key) = provider_updates.gemini_key {
            let trimmed = gemini_key.trim().to_string();
            provider_secrets.set_key(
                ProviderKind::Gemini,
                if trimmed.is_empty() {
                    None
                } else {
                    Some(trimmed)
                },
            );
        }
        if let Some(claude_key) = provider_updates.claude_key {
            let trimmed = claude_key.trim().to_string();
            provider_secrets.set_key(
                ProviderKind::Claude,
                if trimmed.is_empty() {
                    None
                } else {
                    Some(trimmed)
                },
            );
        }
        if let Some(mistral_key) = provider_updates.mistral_key {
            let trimmed = mistral_key.trim().to_string();
            provider_secrets.set_key(
                ProviderKind::Mistral,
                if trimmed.is_empty() {
                    None
                } else {
                    Some(trimmed)
                },
            );
        }
    }
    let storage = AppStorage::new(resolved.clone());
    storage
        .save_provider_secrets(username, master_key, &provider_secrets)
        .map_err(|error| error.to_string())?;
    read_snapshot(&app, &guard).map_err(|error| error.to_string())
}

#[tauri::command]
fn get_diagnostics(
    app: AppHandle,
    state: State<'_, AppSession>,
) -> Result<DiagnosticsReport, String> {
    let _guard = session(&state).map_err(|error| error.to_string())?;
    let (config, storage) = app_storage(&app).map_err(|error| error.to_string())?;
    let (chat_models, image_models, video_models) =
        storage.scan_models().map_err(|error| error.to_string())?;
    let mut all_models = Vec::new();
    all_models.extend(chat_models);
    all_models.extend(image_models);
    all_models.extend(video_models);
    let runtimes = storage.scan_runtimes().map_err(|error| error.to_string())?;
    Ok(storage.diagnostics(&all_models, &runtimes, config.limits.max_repo_file_size_mb))
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(AppSession(Mutex::new(SessionState::default())))
        .invoke_handler(tauri::generate_handler![
            get_app_state,
            initialize_setup,
            unlock_with_password,
            lock_app,
            save_profile,
            save_character,
            create_chat,
            select_chat,
            send_chat_message,
            send_incognito_chat_message,
            rename_chat,
            set_chat_settings,
            toggle_chat_flag,
            clear_chat_messages,
            delete_chat,
            delete_message,
            toggle_pinned_message,
            update_message,
            import_media_attachment,
            build_media_prompt_from_chat,
            generate_image_from_chat,
            generate_video_from_chat,
            list_backups,
            create_backup,
            rescan_models,
            save_settings,
            get_diagnostics,
            set_chat_folder,
            clear_chat_folder,
            read_chat_folder,
            apply_chat_tools
        ])

        .run(tauri::generate_context!())
        .expect("error while running AI Chat application");
}
