use std::{
    collections::HashMap,
    fs,
    path::{Component, Path, PathBuf},
    time::UNIX_EPOCH,
};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tauri::AppHandle;
use walkdir::WalkDir;

use crate::{
    error::{AppError, AppResult},
    models::{
        default_appearance_config, AppConfig, GenerationConfig, LimitsConfig, PathConfig,
        PrivacyConfig, SecurityConfig,
    },
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResolvedPaths {
    pub app_root: PathBuf,
    pub data_dir: PathBuf,
    pub models_dir: PathBuf,
    pub image_models_dir: PathBuf,
    pub video_models_dir: PathBuf,
    pub characters_dir: PathBuf,
    pub chats_dir: PathBuf,
    pub media_dir: PathBuf,
    pub images_dir: PathBuf,
    pub videos_dir: PathBuf,
    pub thumbnails_dir: PathBuf,
    pub config_dir: PathBuf,
    pub profiles_dir: PathBuf,
    pub presets_dir: PathBuf,
    pub backups_dir: PathBuf,
    pub logs_dir: PathBuf,
    pub runtimes_dir: PathBuf,
}

impl ResolvedPaths {
    pub fn to_path_config(&self) -> PathConfig {
        PathConfig {
            app_root: self.app_root.to_string_lossy().to_string(),
            data_dir: self.data_dir.to_string_lossy().to_string(),
            models_dir: self.models_dir.to_string_lossy().to_string(),
            image_models_dir: self.image_models_dir.to_string_lossy().to_string(),
            video_models_dir: self.video_models_dir.to_string_lossy().to_string(),
            characters_dir: self.characters_dir.to_string_lossy().to_string(),
            chats_dir: self.chats_dir.to_string_lossy().to_string(),
            media_dir: self.media_dir.to_string_lossy().to_string(),
            images_dir: self.images_dir.to_string_lossy().to_string(),
            videos_dir: self.videos_dir.to_string_lossy().to_string(),
            thumbnails_dir: self.thumbnails_dir.to_string_lossy().to_string(),
            config_dir: self.config_dir.to_string_lossy().to_string(),
            profiles_dir: self.profiles_dir.to_string_lossy().to_string(),
            presets_dir: self.presets_dir.to_string_lossy().to_string(),
            backups_dir: self.backups_dir.to_string_lossy().to_string(),
            logs_dir: self.logs_dir.to_string_lossy().to_string(),
            runtimes_dir: self.runtimes_dir.to_string_lossy().to_string(),
        }
    }

    pub fn ensure_directories(&self) -> AppResult<()> {
        for path in [
            &self.data_dir,
            &self.models_dir,
            &self.image_models_dir,
            &self.video_models_dir,
            &self.characters_dir,
            &self.chats_dir,
            &self.media_dir,
            &self.images_dir,
            &self.videos_dir,
            &self.thumbnails_dir,
            &self.config_dir,
            &self.profiles_dir,
            &self.presets_dir,
            &self.backups_dir,
            &self.logs_dir,
            &self.runtimes_dir,
            &self.runtimes_dir.join("Chat"),
            &self.runtimes_dir.join("Image"),
            &self.runtimes_dir.join("Video"),
        ] {
            fs::create_dir_all(path)?;
        }
        Ok(())
    }
}

pub fn default_config() -> AppConfig {
    AppConfig {
        app_name: "AI Chat".to_string(),
        portable: true,
        paths: PathConfig {
            app_root: "${APP_ROOT}".to_string(),
            data_dir: "${APP_ROOT}/Data".to_string(),
            models_dir: "${DATA_DIR}/Models".to_string(),
            image_models_dir: "${DATA_DIR}/ImageModels".to_string(),
            video_models_dir: "${DATA_DIR}/VideoModels".to_string(),
            characters_dir: "${DATA_DIR}/Characters".to_string(),
            chats_dir: "${DATA_DIR}/Chats".to_string(),
            media_dir: "${DATA_DIR}/Media".to_string(),
            images_dir: "${MEDIA_DIR}/Images".to_string(),
            videos_dir: "${MEDIA_DIR}/Videos".to_string(),
            thumbnails_dir: "${MEDIA_DIR}/Thumbnails".to_string(),
            config_dir: "${DATA_DIR}/Config".to_string(),
            profiles_dir: "${DATA_DIR}/Profiles".to_string(),
            presets_dir: "${DATA_DIR}/Presets".to_string(),
            backups_dir: "${DATA_DIR}/Backups".to_string(),
            logs_dir: "${DATA_DIR}/Logs".to_string(),
            runtimes_dir: "${APP_ROOT}/Runtimes".to_string(),
        },
        limits: LimitsConfig {
            max_repo_file_size_mb: 100,
        },
        appearance: default_appearance_config(),
        privacy: PrivacyConfig {
            local_only: true,
            telemetry: false,
            analytics: false,
            cloud_sync: false,
            remote_providers: false,
        },
        security: SecurityConfig {
            lock_on_startup: true,
            auto_lock_minutes: 5,
        },
        generation: Some(GenerationConfig {
            threads: None,
            gpu_layers: None,
            max_tokens: Some(2048),
        }),
    }
}

pub fn detect_app_root(_app: &AppHandle) -> AppResult<PathBuf> {
    if cfg!(debug_assertions) {
        let src_tauri_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let repo_root = src_tauri_dir
            .parent()
            .ok_or_else(|| AppError::Message("Unable to determine repository root".to_string()))?;
        Ok(repo_root.to_path_buf())
    } else {
        let exe_path = std::env::current_exe()?;
        let parent = exe_path.parent().ok_or_else(|| {
            AppError::Message("Unable to determine executable directory".to_string())
        })?;
        Ok(parent.to_path_buf())
    }
}

pub fn load_or_create_config(app_root: &Path) -> AppResult<AppConfig> {
    let config_dir = app_root.join("Data").join("Config");
    fs::create_dir_all(&config_dir)?;
    let config_path = config_dir.join("app-config.json");

    if config_path.exists() {
        let raw = fs::read_to_string(config_path)?;
        Ok(serde_json::from_str(&raw)?)
    } else {
        let config = default_config();
        fs::write(&config_path, serde_json::to_string_pretty(&config)?)?;
        Ok(config)
    }
}

pub fn resolve_paths(config: &AppConfig, app_root: &Path) -> ResolvedPaths {
    let app_root_string = app_root.to_string_lossy().replace('\\', "/");
    let data_dir = expand_path(
        &config.paths.data_dir,
        app_root,
        &HashMap::from([("APP_ROOT".to_string(), app_root_string.clone())]),
    );
    let data_dir_string = data_dir.to_string_lossy().replace('\\', "/");
    let media_dir = expand_path(
        &config.paths.media_dir,
        app_root,
        &HashMap::from([
            ("APP_ROOT".to_string(), app_root_string.clone()),
            ("DATA_DIR".to_string(), data_dir_string.clone()),
        ]),
    );
    let media_dir_string = media_dir.to_string_lossy().replace('\\', "/");

    let vars = HashMap::from([
        ("APP_ROOT".to_string(), app_root_string.clone()),
        ("DATA_DIR".to_string(), data_dir_string.clone()),
        ("MEDIA_DIR".to_string(), media_dir_string),
    ]);

    ResolvedPaths {
        app_root: app_root.to_path_buf(),
        data_dir,
        models_dir: expand_path(&config.paths.models_dir, app_root, &vars),
        image_models_dir: expand_path(&config.paths.image_models_dir, app_root, &vars),
        video_models_dir: expand_path(&config.paths.video_models_dir, app_root, &vars),
        characters_dir: expand_path(&config.paths.characters_dir, app_root, &vars),
        chats_dir: expand_path(&config.paths.chats_dir, app_root, &vars),
        media_dir: expand_path(&config.paths.media_dir, app_root, &vars),
        images_dir: expand_path(&config.paths.images_dir, app_root, &vars),
        videos_dir: expand_path(&config.paths.videos_dir, app_root, &vars),
        thumbnails_dir: expand_path(&config.paths.thumbnails_dir, app_root, &vars),
        config_dir: expand_path(&config.paths.config_dir, app_root, &vars),
        profiles_dir: expand_path(&config.paths.profiles_dir, app_root, &vars),
        presets_dir: expand_path(&config.paths.presets_dir, app_root, &vars),
        backups_dir: expand_path(&config.paths.backups_dir, app_root, &vars),
        logs_dir: expand_path(&config.paths.logs_dir, app_root, &vars),
        runtimes_dir: expand_path(&config.paths.runtimes_dir, app_root, &vars),
    }
}

fn expand_path(raw: &str, app_root: &Path, vars: &HashMap<String, String>) -> PathBuf {
    let mut expanded = raw.replace('\\', "/");
    for _ in 0..8 {
        let previous = expanded.clone();
        for (key, value) in vars {
            expanded = expanded.replace(&format!("${{{key}}}"), value);
        }
        if expanded == previous {
            break;
        }
    }

    let candidate = PathBuf::from(expanded);
    if candidate.is_absolute() {
        normalize_path(candidate)
    } else {
        normalize_path(app_root.join(candidate))
    }
}

fn normalize_path(path: PathBuf) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Prefix(prefix) => normalized.push(prefix.as_os_str()),
            Component::RootDir => normalized.push(Path::new(std::path::MAIN_SEPARATOR_STR)),
            Component::CurDir => {}
            Component::ParentDir => {
                normalized.pop();
            }
            Component::Normal(part) => normalized.push(part),
        }
    }
    normalized
}

pub fn write_json<T: Serialize>(path: &Path, value: &T) -> AppResult<()> {
    fs::write(path, serde_json::to_string_pretty(value)?)?;
    Ok(())
}

pub fn read_json<T: for<'de> Deserialize<'de>>(path: &Path) -> AppResult<T> {
    Ok(serde_json::from_str(&fs::read_to_string(path)?)?)
}

pub fn to_iso_time(metadata: &fs::Metadata) -> Option<String> {
    metadata
        .modified()
        .ok()
        .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
        .and_then(|duration| DateTime::<Utc>::from_timestamp(duration.as_secs() as i64, 0))
        .map(|timestamp| timestamp.to_rfc3339())
}

pub fn folder_size(path: &Path) -> u64 {
    WalkDir::new(path)
        .into_iter()
        .filter_map(Result::ok)
        .filter_map(|entry| entry.metadata().ok())
        .filter(|metadata| metadata.is_file())
        .map(|metadata| metadata.len())
        .sum()
}
