use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;
use tauri::{
    menu::{Menu, MenuItem},
    tray::TrayIconBuilder,
    Manager, State,
};

#[derive(Serialize, Deserialize, Clone)]
struct AppSettings {
    password_hash: Option<String>,
    auto_hide: bool,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            password_hash: None,
            auto_hide: false,
        }
    }
}

struct AppState {
    settings: Mutex<AppSettings>,
}

fn get_settings_path() -> PathBuf {
    let mut path = std::env::current_exe()
        .unwrap_or_else(|_| PathBuf::from("."))
        .parent()
        .unwrap_or_else(|| std::path::Path::new("."))
        .to_path_buf();
    path.push("settings.json");
    path
}

fn load_settings() -> AppSettings {
    let path = get_settings_path();
    if let Ok(content) = fs::read_to_string(&path) {
        if let Ok(settings) = serde_json::from_str(&content) {
            return settings;
        }
    }
    AppSettings::default()
}

fn save_settings(settings: &AppSettings) -> Result<(), String> {
    let path = get_settings_path();
    let content = serde_json::to_string_pretty(settings)
        .map_err(|e| format!("序列化失败: {}", e))?;
    fs::write(&path, content)
        .map_err(|e| format!("写入失败: {}", e))?;
    Ok(())
}

fn hash_password(password: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(password.as_bytes());
    let result = hasher.finalize();
    hex::encode(result)
}

#[tauri::command]
fn set_password(
    password: String,
    old_password: Option<String>,
    state: State<AppState>,
) -> Result<(), String> {
    let mut settings = state.settings.lock().unwrap();

    // 如果已设置密码，需要验证原密码
    if let Some(ref existing_hash) = settings.password_hash {
        let provided_old = old_password.ok_or("需要输入原密码")?;
        let provided_hash = hash_password(&provided_old);
        if &provided_hash != existing_hash {
            return Err("原密码错误".to_string());
        }
    }

    let hash = hash_password(&password);
    settings.password_hash = Some(hash);
    save_settings(&settings)?;
    Ok(())
}

#[tauri::command]
fn verify_password(password: String, state: State<AppState>) -> Result<bool, String> {
    let settings = state.settings.lock().unwrap();
    if let Some(ref hash) = settings.password_hash {
        let input_hash = hash_password(&password);
        Ok(&input_hash == hash)
    } else {
        Ok(false)
    }
}

#[tauri::command]
fn has_password(state: State<AppState>) -> Result<bool, String> {
    let settings = state.settings.lock().unwrap();
    Ok(settings.password_hash.is_some())
}

#[tauri::command]
fn get_settings(state: State<AppState>) -> Result<AppSettings, String> {
    let settings = state.settings.lock().unwrap();
    Ok(settings.clone())
}

#[tauri::command]
fn update_setting(key: String, value: bool, state: State<AppState>) -> Result<(), String> {
    let mut settings = state.settings.lock().unwrap();
    match key.as_str() {
        "auto_hide" => settings.auto_hide = value,
        _ => return Err(format!("未知设置项: {}", key)),
    }
    save_settings(&settings)?;
    Ok(())
}

fn setup_tray(app: &tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    let show_item = MenuItem::with_id(app, "show", "显示窗口", true, None::<&str>)?;
    let quit_item = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&show_item, &quit_item])?;

    let _tray = TrayIconBuilder::new()
        .icon(app.default_window_icon().unwrap().clone())
        .menu(&menu)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "show" => toggle_window_visibility(app),
            "quit" => app.exit(0),
            _ => {}
        })
        .build(app)?;

    Ok(())
}

fn toggle_window_visibility(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let is_visible = window.is_visible().unwrap_or(true);
        if is_visible {
            let _ = window.hide();
        } else {
            let _ = window.show();
            let _ = window.set_focus();
        }
    }
}

fn setup_window_events(app: &tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    if let Some(window) = app.get_webview_window("main") {
        let window_clone = window.clone();
        window.on_window_event(move |event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = window_clone.hide();
            }
        });
    }
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let settings = load_settings();
    let app_state = AppState {
        settings: Mutex::new(settings),
    };

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(app_state)
        .invoke_handler(tauri::generate_handler![
            set_password,
            verify_password,
            has_password,
            get_settings,
            update_setting
        ])
        .setup(|app| {
            setup_tray(app)?;
            setup_window_events(app)?;
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
