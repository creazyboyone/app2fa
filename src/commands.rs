//! Tauri 命令

use crate::migration;
use crate::models::{Account, MigrationAccount, TotpResult};
use crate::storage;
use crate::totp;
use tauri::Manager;

#[tauri::command]
pub fn load_accounts() -> Result<Vec<Account>, String> {
    storage::load_accounts()
}

#[tauri::command]
pub fn save_accounts(accounts: Vec<Account>) -> Result<(), String> {
    storage::save_accounts(accounts)
}

#[tauri::command]
pub fn generate_totp(secret: String) -> Result<TotpResult, String> {
    totp::generate(&secret)
}

#[tauri::command]
pub fn add_account(name: String, issuer: Option<String>, secret: String) -> Result<Account, String> {
    totp::create_account(name, issuer, secret)
}

#[tauri::command]
pub fn delete_account(id: String) -> Result<(), String> {
    storage::delete_account(&id)
}

#[tauri::command]
pub fn parse_otpauth_uri(uri: String) -> Result<Account, String> {
    migration::parse_otpauth_uri(&uri)
}

#[tauri::command]
pub fn decode_migration_payload(b64_data: String) -> Result<Vec<MigrationAccount>, String> {
    migration::decode_migration_payload(&b64_data)
}

#[tauri::command]
pub fn parse_qr_image(data_url: String) -> Result<String, String> {
    migration::parse_qr_image(&data_url)
}

#[tauri::command]
pub fn verify_windows_hello() -> Result<bool, String> {
    // TODO: 实现 Windows Hello 验证
    Ok(true)
}

#[tauri::command]
pub async fn copy_to_clipboard(app: tauri::AppHandle, text: String) -> Result<(), String> {
    use tauri_plugin_clipboard_manager::ClipboardExt;
    app.clipboard()
        .write_text(&text)
        .map_err(|e| format!("复制失败：{:?}", e))
}

#[tauri::command]
pub async fn minimize_to_tray(app: tauri::AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("main") {
        window.hide().map_err(|e| format!("隐藏窗口失败：{:?}", e))?;
    }
    Ok(())
}

#[tauri::command]
pub async fn show_notification(
    app: tauri::AppHandle,
    title: String,
    body: String,
) -> Result<(), String> {
    use tauri_plugin_notification::NotificationExt;
    app.notification()
        .builder()
        .title(&title)
        .body(&body)
        .show()
        .map_err(|e| format!("通知失败：{:?}", e))
}
