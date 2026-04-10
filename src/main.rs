// ==================== 数据结构定义 ====================

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Account {
    pub id: String,
    pub name: String,
    pub issuer: Option<String>,
    pub secret: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TOTPResult {
    pub code: String,
    pub remaining_seconds: u32,
}

// ==================== 业务逻辑（无宏）=====================

fn get_data_path() -> std::path::PathBuf {
    let appdata = std::env::var("APPDATA").unwrap_or_else(|_| String::from("%APPDATA%"));
    let mut path = std::path::PathBuf::from(appdata);
    path.push("totp-manager");
    path.push("keys.bin");
    path
}

fn load_accounts_impl() -> Result<Vec<Account>, String> {
    let path = get_data_path();
    if !path.exists() { return Ok(Vec::new()); }
    let data = std::fs::read(&path).map_err(|e| format!("读取数据文件失败：{:?}", e))?;
    serde_json::from_slice(&data).map_err(|e| format!("解析数据失败：{:?}", e))
}

fn save_accounts_impl(accounts: Vec<Account>) -> Result<(), String> {
    let path = get_data_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("创建目录失败：{:?}", e))?;
    }
    let json = serde_json::to_vec(&accounts).map_err(|e| format!("数据序列化失败：{:?}", e))?;
    std::fs::write(&path, &json).map_err(|e| format!("写入数据文件失败：{:?}", e))
}

fn generate_totp_impl(secret: String) -> Result<TOTPResult, String> {
    let cleaned_secret = secret.replace([' ', '-'], "").to_uppercase();
    let totp = totp_rs::TOTP::new(
        totp_rs::Algorithm::SHA1, 6, 0, 0,
        Vec::from(cleaned_secret), None, String::new(),
    ).map_err(|e| format!("密钥格式错误：{:?}", e))?;
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH).map_err(|e| format!("时间错误：{:?}", e))?
        .as_secs();
    let code = totp.generate(now);
    Ok(TOTPResult {
        code,
        remaining_seconds: (30 - now % 30) as u32,
    })
}

fn add_account_impl(name: String, issuer: Option<String>, secret: String) -> Account {
    let id = uuid::Uuid::new_v4().to_string();
    let cleaned_secret = secret.replace([' ', '-'], "").to_uppercase();
    Account { id, name, issuer, secret: cleaned_secret }
}

fn delete_account_impl(id: String) -> Result<(), String> {
    let mut accounts = load_accounts_impl()?;
    let initial_len = accounts.len();
    accounts.retain(|a| a.id != id);
    if accounts.len() == initial_len { return Err(format!("账户不存在：{}", id)); }
    save_accounts_impl(accounts)?;
    Ok(())
}

fn parse_otpauth_uri_impl(uri: String) -> Result<Account, String> {
    if !uri.starts_with("otpauth://totp/") {
        return Err("不支持的 URI 格式：必须是 otpauth://totp/".to_string());
    }
    let uri = uri.trim_start_matches("otpauth://totp/");
    let mut parts = uri.split('?');
    let label_part: &str = parts.next().unwrap_or("");
    let params_part: &str = parts.next().unwrap_or("");
    let decoded_label = percent_decode(label_part)?;
    let at_pos = decoded_label.rfind('@');
    let (name, issuer): (&str, Option<&str>) = if let Some(pos) = at_pos {
        (&decoded_label[..pos], Some(&decoded_label[pos + 1..]))
    } else { (&decoded_label, None) };
    let mut secret = String::new();
    for param in params_part.split('&') {
        if let Some(kv) = param.strip_prefix("secret=") { secret = percent_decode(kv)?; }
    }
    if secret.is_empty() { return Err("URI 中缺少 secret 参数".to_string()); }
    totp_rs::TOTP::new(totp_rs::Algorithm::SHA1, 6, 0, 0,
        Vec::from(secret.to_uppercase()), None, String::new())
        .map_err(|_| "无效的 secret 格式")?;
    let id = uuid::Uuid::new_v4().to_string();
    Ok(Account {
        id, name: name.to_string(), issuer: issuer.map(|s| s.to_string()),
        secret: secret.to_uppercase(),
    })
}

fn percent_decode(s: &str) -> Result<String, String> {
    let mut result = String::new();
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '%' {
            let hex: String = chars.by_ref().take(2).collect();
            if hex.len() == 2 {
                if let Ok(byte) = u8::from_str_radix(&hex, 16) {
                    result.push(byte as char); continue;
                }
            }
            return Err("URL 解码失败".to_string());
        } else if c == '+' { result.push(' '); continue; }
        result.push(c);
    }
    Ok(result)
}

// ==================== Tauri 应用入口 ====================

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            load_accounts, save_accounts, generate_totp, parse_qr_image,
            add_account, delete_account, parse_otpauth_uri, verify_windows_hello,
        ])
        .setup(|_app| {
            println!("TOTP Manager 启动");
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("运行 Tauri 应用程序失败");
}

// ==================== Tauri 命令（内联定义）=====================

#[tauri::command]
fn load_accounts() -> Result<Vec<Account>, String> {
    load_accounts_impl()
}

#[tauri::command]
fn save_accounts(accounts: Vec<Account>) -> Result<(), String> {
    save_accounts_impl(accounts)
}

#[tauri::command]
fn generate_totp(secret: String) -> Result<TOTPResult, String> {
    generate_totp_impl(secret)
}

#[tauri::command]
fn parse_qr_image(_path: String) -> Result<String, String> {
    Err("QR 图片导入功能正在开发中，请使用手动输入或摄像头扫描".to_string())
}

#[tauri::command]
fn add_account(name: String, issuer: Option<String>, secret: String) -> Result<Account, String> {
    Ok(add_account_impl(name, issuer, secret))
}

#[tauri::command]
fn delete_account(id: String) -> Result<(), String> {
    delete_account_impl(id)
}

#[tauri::command]
fn parse_otpauth_uri(uri: String) -> Result<Account, String> {
    parse_otpauth_uri_impl(uri)
}

#[tauri::command]
fn verify_windows_hello() -> Result<bool, String> {
    #[cfg(any(feature = "skip-windows-hello", not(windows)))]
    { return Ok(true); }
    match windows::Security::Credentials::UI::UserConsentVerifier::IUserConsentVerifierStatics(|_| Ok(())) {
        Ok(_) => Ok(true),
        Err(e) => Err(format!("Windows Hello 不可用：{:?}。\n请确保已配置 Windows Hello PIN、指纹或面部识别后重试。", e)),
    }
}
