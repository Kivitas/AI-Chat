use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PathConfig {
    pub app_root: String,
    pub data_dir: String,
    pub models_dir: String,
    pub image_models_dir: String,
    pub video_models_dir: String,
    pub characters_dir: String,
    pub chats_dir: String,
    pub media_dir: String,
    pub images_dir: String,
    pub videos_dir: String,
    pub thumbnails_dir: String,
    pub config_dir: String,
    pub profiles_dir: String,
    pub presets_dir: String,
    pub backups_dir: String,
    pub logs_dir: String,
    pub runtimes_dir: String,
    #[serde(default)]
    pub workspace_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LimitsConfig {
    pub max_repo_file_size_mb: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppearanceConfig {
    pub theme_id: String,
    pub theme_group: String,
    pub smooth: bool,
    pub compact: bool,
    #[serde(default = "default_true")]
    pub auto_scroll: bool,
    #[serde(default = "default_true")]
    pub typewriter: bool,
    #[serde(default = "default_true")]
    pub show_performance: bool,
    #[serde(default)]
    pub wide_messages: bool,
    #[serde(default)]
    pub technical_mode: bool,
    pub scale: u32,
    pub message_style: String,
    pub sidebar_collapsed: bool,
}

fn default_true() -> bool {
    true
}

pub fn default_appearance_config() -> AppearanceConfig {
    AppearanceConfig {
        theme_id: "charcoal".to_string(),
        theme_group: "dark".to_string(),
        smooth: true,
        compact: false,
        auto_scroll: true,
        typewriter: true,
        show_performance: true,
        wide_messages: false,
        technical_mode: false,
        scale: 100,
        message_style: "cards".to_string(),
        sidebar_collapsed: false,
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PrivacyConfig {
    pub local_only: bool,
    pub telemetry: bool,
    pub analytics: bool,
    pub cloud_sync: bool,
    pub remote_providers: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SecurityConfig {
    pub lock_on_startup: bool,
    pub auto_lock_minutes: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GenerationConfig {
    pub threads: Option<u32>,
    pub gpu_layers: Option<u32>,
    pub max_tokens: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppConfig {
    pub app_name: String,
    pub portable: bool,
    pub paths: PathConfig,
    pub limits: LimitsConfig,
    #[serde(default = "default_appearance_config")]
    pub appearance: AppearanceConfig,
    pub privacy: PrivacyConfig,
    pub security: SecurityConfig,
    pub generation: Option<GenerationConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthCapabilities {
    pub password: bool,
    pub passkey: bool,
    pub biometric: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileRecord {
    pub id: String,
    pub display_name: String,
    pub avatar_path: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CharacterRecord {
    pub id: String,
    pub name: String,
    pub description: String,
    pub style_notes: String,
    pub avatar_path: Option<String>,
    pub default_model_id: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelRecord {
    pub id: String,
    pub name: String,
    pub path: String,
    pub kind: String,
    pub size_bytes: u64,
    pub last_modified: Option<String>,
    pub available: bool,
    pub favorite: bool,
    pub missing: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeRecord {
    pub id: String,
    pub kind: String,
    pub name: String,
    pub path: String,
    pub available: bool,
    pub note: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderStatusRecord {
    pub id: String,
    pub label: String,
    pub default_model_id: String,
    pub has_key: bool,
    pub available: bool,
    pub note: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaAsset {
    pub id: String,
    pub kind: String,
    pub path: String,
    pub thumbnail_path: Option<String>,
    pub content_preview: Option<String>,
    pub prompt: Option<String>,
    pub status: Option<String>,
    pub model_used: Option<String>,
    pub seed: Option<u64>,
    pub source_label: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatMessage {
    pub id: String,
    pub role: String,
    pub text: String,
    pub created_at: String,
    pub pinned: bool,
    pub attachments: Vec<MediaAsset>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatSettings {
    pub model_id: Option<String>,
    pub image_model_id: Option<String>,
    pub video_model_id: Option<String>,
    pub profile_override_id: Option<String>,
    pub user_name: Option<String>,
    pub ai_name: Option<String>,
    pub ai_instructions: Option<String>,
    pub user_avatar: Option<String>,
    pub ai_avatar: Option<String>,
    /// Absolute path to a folder the user imported as workspace context for this chat.
    #[serde(default)]
    pub context_folder_path: Option<String>,
}

// ── Folder context types ──────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FolderFile {
    pub rel_path: String,
    pub abs_path: String,
    pub size_bytes: u64,
    pub is_text: bool,
    pub content: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FolderContext {
    pub root: String,
    pub files: Vec<FolderFile>,
    pub total_files: usize,
    pub skipped_files: usize,
    pub truncated: bool,
}

// ── Agentic tool-call types ───────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolCall {
    pub action: String,
    pub path: String,
    pub content: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolCallResult {
    pub action: String,
    pub path: String,
    pub success: bool,
    pub message: String,
    pub content: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatRecord {
    pub id: String,
    pub title: String,
    pub created_at: String,
    pub updated_at: String,
    pub favorite: bool,
    pub archived: bool,
    pub settings: ChatSettings,
    pub messages: Vec<ChatMessage>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppSnapshot {
    pub app_name: String,
    pub setup_complete: bool,
    pub locked: bool,
    pub offline_notice: String,
    pub config: AppConfig,
    pub directories: PathConfig,
    pub auth_capabilities: AuthCapabilities,
    pub profile: Option<ProfileRecord>,
    pub character: Option<CharacterRecord>,
    pub chats: Vec<ChatRecord>,
    pub active_chat_id: Option<String>,
    pub chat_models: Vec<ModelRecord>,
    pub image_models: Vec<ModelRecord>,
    pub video_models: Vec<ModelRecord>,
    pub providers: Vec<ProviderStatusRecord>,
    pub runtimes: Vec<RuntimeRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiagnosticsReport {
    pub app_root: String,
    pub data_dir: String,
    pub runtimes_dir: String,
    pub detected_models: Vec<ModelRecord>,
    pub missing_model_paths: Vec<String>,
    pub runtime_availability: Vec<RuntimeRecord>,
    pub disk_usage_bytes: u64,
    pub repo_oversize_warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupRecord {
    pub id: String,
    pub path: String,
    pub created_at: String,
    pub size_bytes: u64,
    pub files_copied: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupResult {
    pub snapshot: AppSnapshot,
    pub backup: BackupRecord,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SetupPayload {
    pub username: String,
    pub password: String,
    pub display_name: String,
    pub profile_avatar_base64: Option<String>,
    pub character_name: String,
    pub character_description: String,
    pub character_style_notes: String,
    pub character_avatar_base64: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaPromptPreview {
    pub prompt: String,
    pub sources: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveProfilePayload {
    pub display_name: String,
    pub avatar_base64: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveCharacterPayload {
    pub name: String,
    pub description: String,
    pub style_notes: String,
    pub avatar_base64: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingsUpdatePayload {
    pub paths: PathConfig,
    pub auto_lock_minutes: u32,
    pub appearance: AppearanceConfig,
    pub generation: Option<GenerationConfig>,
    pub remote_providers: bool,
    pub provider_keys: Option<ProviderSecretsUpdatePayload>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderSecretsUpdatePayload {
    pub openai_key: Option<String>,
    pub openrouter_key: Option<String>,
    pub gemini_key: Option<String>,
    pub claude_key: Option<String>,
    pub mistral_key: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatSettingsUpdatePayload {
    pub chat_id: String,
    pub model_id: Option<String>,
    pub image_model_id: Option<String>,
    pub video_model_id: Option<String>,
    pub profile_override_id: Option<String>,
    pub user_name: Option<String>,
    pub ai_name: Option<String>,
    pub ai_instructions: Option<String>,
    pub user_avatar_base64: Option<String>,
    pub ai_avatar_base64: Option<String>,
    pub user_avatar_path: Option<String>,
    pub ai_avatar_path: Option<String>,
    pub context_folder_path: Option<String>,
}
