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

/// 严格验证 Base32 字符串格式
fn validate_base32_strict(s: &str) -> Result<(), String> {
    let cleaned = s.replace([' ', '-'], "").to_uppercase();
    if cleaned.is_empty() || cleaned.len() < 8 {
        return Err("密钥无效：长度不足".to_string());
    }

    // Check each character is valid Base32 (A-Z and 2-7 only)
    for c in cleaned.chars() {
        if !(c.is_ascii_uppercase() || ('2'..='7').contains(&c)) {
            return Err(format!("密钥包含无效字符：{}。Base32 只允许 A-Z 和 2-7", c));
        }
    }

    // Try to create TOTP from base32 - this will fail gracefully if invalid (no panic!)
    let Some(_totp) = otpauth::TOTP::from_base32(&cleaned) else {
        return Err("无效的 Base32 密钥格式".to_string());
    };

    Ok(())
}

fn load_accounts_impl() -> Result<Vec<Account>, String> {
    let path = get_data_path();
    if !path.exists() { return Ok(Vec::new()); }
    let data = std::fs::read(&path).map_err(|e| format!("读取数据文件失败：{:?}", e))?;
    let accounts: Vec<Account> = serde_json::from_slice(&data).map_err(|e| format!("解析数据失败：{:?}", e))?;
    // Filter out invalid accounts (empty or short secrets)
    Ok(accounts.into_iter().filter(|a| {
        let valid = !a.secret.is_empty() && a.secret.len() >= 8;
        if !valid { println!("[WARN] Filtering out account with invalid secret: {}", a.name); }
        valid
    }).collect())
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
    println!("[DEBUG] generate_totp called with secret: {} (len={})", cleaned_secret, cleaned_secret.len());

    // Validate base32 format first
    validate_base32_strict(&cleaned_secret)?;

    // Create TOTP from base32 - returns Option<TOTP>, no panic!
    let Some(totp) = otpauth::TOTP::from_base32(&cleaned_secret) else {
        return Err("无效的 Base32 密钥格式".to_string());
    };

    // Generate TOTP code - uses standard period=30, returns u32 (never panics)
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|e| format!("时间错误：{:?}", e))?
        .as_secs();

    let code = totp.generate(30, now);

    Ok(TOTPResult {
        code: code.to_string(),
        remaining_seconds: (30 - now % 30) as u32,
    })
}

fn add_account_impl(name: String, issuer: Option<String>, secret: String) -> Result<Account, String> {
    let id = uuid::Uuid::new_v4().to_string();
    let cleaned_secret = secret.replace([' ', '-'], "").to_uppercase();

    // Validate base32 format and that TOTP can be created
    validate_base32_strict(&cleaned_secret)?;

    Ok(Account { id, name, issuer, secret: cleaned_secret })
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

    // Validate base32 format
    validate_base32_strict(&secret.to_uppercase())?;

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
    add_account_impl(name, issuer, secret)
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
    // Always return true - skip Windows Hello verification for now
    Ok(true)
}
