//! TOTP Manager - Windows 2FA Key Manager

mod commands;
mod crypto;
mod migration;
mod models;
mod storage;
mod totp;

use tauri::Manager;

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_notification::init())
        .invoke_handler(tauri::generate_handler![
            commands::load_accounts,
            commands::save_accounts,
            commands::generate_totp,
            commands::parse_qr_image,
            commands::add_account,
            commands::delete_account,
            commands::parse_otpauth_uri,
            commands::verify_windows_hello,
            commands::decode_migration_payload,
            commands::copy_to_clipboard,
            commands::minimize_to_tray,
            commands::show_notification,
        ])
        .setup(|app| {
            // 创建系统托盘
            let tray = tauri::tray::TrayIconBuilder::new()
                .icon(app.default_window_icon().unwrap().clone())
                .tooltip("2FA Manager - 点击打开")
                .on_tray_icon_event(|tray, event| {
                    if let tauri::tray::TrayIconEvent::Click { .. } = event {
                        let app = tray.app_handle();
                        if let Some(window) = app.get_webview_window("main") {
                            let _ = window.show();
                            let _ = window.set_focus();
                        }
                    }
                })
                .build(app);

            if let Err(e) = tray {
                eprintln!("创建系统托盘失败: {:?}", e);
            }

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("运行 Tauri 应用程序失败");
}
