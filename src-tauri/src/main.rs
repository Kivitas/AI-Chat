// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::{
    env, fs,
    path::{Path, PathBuf},
};

fn main() {
    sanitize_llama_environment();
    configure_portable_environment();
    ai_chat_lib::run();
}

fn configure_portable_environment() {
    if let Ok(exe_path) = env::current_exe() {
        if let Some(app_root) = exe_path.parent() {
            cleanup_startup_junk(app_root);

            let app_dir = app_root.join("App");
            let data_dir = app_root.join("Data");
            let webview_dir = app_dir.join("WebView2Profile");

            let _ = fs::create_dir_all(&app_dir);
            let _ = fs::create_dir_all(&data_dir);
            let _ = fs::create_dir_all(&webview_dir);

            env::set_var("AI_CHAT_PORTABLE", "1");
            env::set_var("AI_CHAT_APP_ROOT", app_root);
            env::set_var("AI_CHAT_DATA_DIR", &data_dir);
            env::set_var("WEBVIEW2_USER_DATA_FOLDER", webview_dir);
        }
    }
}

fn sanitize_llama_environment() {
    let keys: Vec<_> = env::vars_os()
        .map(|(key, _)| key)
        .filter(|key| key.to_string_lossy().starts_with("LLAMA_"))
        .collect();
    for key in keys {
        env::remove_var(key);
    }
}

fn cleanup_startup_junk(app_root: &Path) {
    remove_path_if_exists(&app_root.join("src-tauri").join("target"));
    remove_path_if_exists(&app_root.join("dist"));

    let webview_profile = app_root.join("App").join("WebView2Profile");
    for relative in [
        PathBuf::from("Cache"),
        PathBuf::from("Code Cache"),
        PathBuf::from("GPUCache"),
        PathBuf::from("DawnCache"),
        PathBuf::from("GrShaderCache"),
        PathBuf::from("GraphiteDawnCache"),
        PathBuf::from("Temp"),
        PathBuf::from("Crashpad"),
        PathBuf::from("Service Worker").join("CacheStorage"),
    ] {
        remove_path_if_exists(&webview_profile.join(relative));
    }

    trim_runtime_log(
        &app_root.join("Data").join("Logs").join("runtime.log"),
        8 * 1024 * 1024,
    );
}

fn remove_path_if_exists(path: &Path) {
    if !path.exists() {
        return;
    }

    let _ = if path.is_dir() {
        fs::remove_dir_all(path)
    } else {
        fs::remove_file(path)
    };
}

fn trim_runtime_log(path: &Path, max_bytes: u64) {
    let Ok(metadata) = fs::metadata(path) else {
        return;
    };
    if metadata.len() <= max_bytes {
        return;
    }

    let Ok(contents) = fs::read(path) else {
        return;
    };
    let keep_from = contents.len().saturating_sub((max_bytes / 2) as usize);
    let trimmed = &contents[keep_from..];
    let _ = fs::write(path, trimmed);
}
