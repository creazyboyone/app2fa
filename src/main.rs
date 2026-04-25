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

/// 从 protobuf 解码的迁移账户（secret 直接为 Base32 编码）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MigrationAccount {
    pub name: String,
    pub issuer: Option<String>,
    /// Base32-encoded secret (可直接用于 TOTP)
    pub secret_b32: String,
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

    for c in cleaned.chars() {
        if !(c.is_ascii_uppercase() || ('2'..='7').contains(&c)) {
            return Err(format!("密钥包含无效字符：{}。Base32 只允许 A-Z 和 2-7", c));
        }
    }

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
    Ok(accounts.into_iter().filter(|a| {
        !a.secret.is_empty() && a.secret.len() >= 8
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
    validate_base32_strict(&cleaned_secret)?;

    let Some(totp) = otpauth::TOTP::from_base32(&cleaned_secret) else {
        return Err("无效的 Base32 密钥格式".to_string());
    };

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|e| format!("时间错误：{:?}", e))?
        .as_secs();

    let code = totp.generate(30, now);
    // Ensure 6-digit code with leading zeros
    let code_str = format!("{:06}", code);

    Ok(TOTPResult {
        code: code_str,
        remaining_seconds: (30 - now % 30) as u32,
    })
}

fn add_account_impl(name: String, issuer: Option<String>, secret: String) -> Result<Account, String> {
    let id = uuid::Uuid::new_v4().to_string();
    let cleaned_secret = secret.replace([' ', '-'], "").to_uppercase();
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

// ==================== Google Authenticator 迁移 payload 解码 ====================

/// 将 raw secret bytes 编码为 Base32（无 padding）
fn encode_secret_base32(secret: &[u8]) -> String {
    base32::encode(base32::Alphabet::Rfc4648 { padding: false }, secret)
        .to_uppercase()
}

/// 解码 Google Authenticator 迁移 payload（base64-encoded protobuf）
fn decode_migration_payload_impl(b64_data: String) -> Result<Vec<MigrationAccount>, String> {
    let b64_data = url_decode(&b64_data);

    #[allow(deprecated)]
    let raw = base64::decode(&b64_data).map_err(|e| format!("Base64 解码失败：{:?}", e))?;

    let mut offset = 0usize;
    let mut accounts = Vec::new();

    while offset < raw.len() {
        let (tag, new_off) = decode_varint(&raw, offset)?;
        offset = new_off;

        let field_num = (tag >> 3) as u32;
        let wire_type = (tag & 7) as u8;

        // Skip non-OtpParameters fields (version_number=2, batch_size=3, batch_index=4, batch_id=5)
        if field_num != 1 {
            match wire_type {
                0 => {
                    let (_, o) = decode_varint(&raw, offset)?;
                    offset = o;
                    continue;
                }
                2 => {
                    let (len, o) = decode_varint(&raw, offset)?;
                    offset = o + len as usize;
                    continue;
                }
                _ => return Err(format!("未知 wire type={}，字段={}", wire_type, field_num)),
            }
        }

        let (msg_len, new_offset) = decode_varint(&raw, offset)?;
        offset = new_offset;

        if offset + msg_len as usize > raw.len() {
            return Err("protobuf 数据截断".to_string());
        }

        let msg_start = offset;
        let otp = parse_otp_parameters(&raw[msg_start..msg_start + msg_len as usize])?;
        offset = msg_start + msg_len as usize;

        let secret_b32 = encode_secret_base32(&otp.secret);

        accounts.push(MigrationAccount {
            name: otp.name,
            issuer: if otp.issuer.is_empty() { None } else { Some(otp.issuer) },
            secret_b32,
        });
    }

    if accounts.is_empty() {
        return Err("payload 中未找到任何账户".to_string());
    }

    Ok(accounts)
}

/// URL-decode the base64 data from QR code (handles %xx escaping)
fn url_decode(s: &str) -> String {
    let mut result = Vec::new();
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Ok(byte) = u8::from_str_radix(&s[i+1..i+3], 16) {
                result.push(byte);
                i += 3;
                continue;
            }
        }
        result.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&result).to_string()
}

/// 解码 varint，返回 (值, 新偏移量)
fn decode_varint(data: &[u8], offset: usize) -> Result<(u64, usize), String> {
    let mut result: u64 = 0;
    let mut shift: u32 = 0;
    let mut i = offset;

    while i < data.len() {
        let byte = data[i];
        result |= ((byte & 0x7F) as u64) << shift;
        i += 1;
        if (byte & 0x80) == 0 {
            return Ok((result, i));
        }
        shift += 7;
        if shift > 63 {
            return Err("varint 溢出".to_string());
        }
    }

    Err("varint 未完整".to_string())
}

/// OtpParameters 的简化解析结果
struct SimpleOtpParams {
    secret: Vec<u8>,
    name: String,
    issuer: String,
}

/// 解析 OtpParameters 嵌套消息
fn parse_otp_parameters(data: &[u8]) -> Result<SimpleOtpParams, String> {
    let mut offset = 0usize;
    let mut secret = Vec::new();
    let mut name = String::new();
    let mut issuer = String::new();

    while offset < data.len() {
        let (tag, new_offset) = decode_varint(data, offset)?;
        offset = new_offset;

        let field_num = (tag >> 3) as u32;
        if field_num == 0 { return Err("无效字段号".to_string()); }

        let wire_type = (tag & 7) as u8;

        match field_num {
            1 => { // bytes secret
                if wire_type != 2 { return Err("secret 期望 length-delimited".to_string()); }
                let (len, o) = decode_varint(data, offset)?;
                offset = o;
                if offset + len as usize > data.len() {
                    return Err("数据截断".to_string());
                }
                secret.extend_from_slice(&data[offset..offset + len as usize]);
                offset += len as usize;
            }
            2 => { // string name
                if wire_type != 2 { return Err("name 期望 length-delimited".to_string()); }
                let (len, o) = decode_varint(data, offset)?;
                offset = o;
                if offset + len as usize > data.len() {
                    return Err("数据截断".to_string());
                }
                name.push_str(
                    std::str::from_utf8(&data[offset..offset + len as usize])
                        .map_err(|e| format!("name 解码失败：{:?}", e))?,
                );
                offset += len as usize;
            }
            3 => { // string issuer
                if wire_type != 2 { return Err("issuer 期望 length-delimited".to_string()); }
                let (len, o) = decode_varint(data, offset)?;
                offset = o;
                if offset + len as usize > data.len() {
                    return Err("数据截断".to_string());
                }
                issuer.push_str(
                    std::str::from_utf8(&data[offset..offset + len as usize])
                        .map_err(|e| format!("issuer 解码失败：{:?}", e))?,
                );
                offset += len as usize;
            }
            _ => {
                match wire_type {
                    0 => {
                        let (_, o) = decode_varint(data, offset)?;
                        offset = o;
                    }
                    2 => {
                        let (len, o) = decode_varint(data, offset)?;
                        offset = o + len as usize;
                    }
                    _ => return Err(format!("未知 wire type={}，字段={}", wire_type, field_num)),
                }
            }
        }
    }

    Ok(SimpleOtpParams { secret, name, issuer })
}

fn parse_qr_image_impl(data_url: String) -> Result<String, String> {
    let b64 = if let Some(pos) = data_url.find(",") {
        &data_url[pos + 1..]
    } else {
        &data_url
    };

    #[allow(deprecated)]
    let img_bytes = base64::decode(b64).map_err(|e| format!("Base64 解码失败：{:?}", e))?;

    let img = image::load_from_memory(&img_bytes).map_err(|e| format!("图片解析失败：{:?}", e))?;

    let mut reader = rxing::qrcode::QRCodeReader::default();
    let result = rxing::Reader::decode(
        &mut reader,
        &mut rxing::BinaryBitmap::new(rxing::common::HybridBinarizer::new(
            rxing::BufferedImageLuminanceSource::new(img),
        )),
    )
    .map_err(|e| format!("QR 码识别失败：{:?}", e))?;

    let text = result.getText();
    if text.is_empty() {
        return Err("QR 码内容为空".to_string());
    }

    Ok(text.to_string())
}

// ==================== Tauri 应用入口 ====================

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .invoke_handler(tauri::generate_handler![
            load_accounts, save_accounts, generate_totp, parse_qr_image,
            add_account, delete_account, parse_otpauth_uri, verify_windows_hello,
            decode_migration_payload, copy_to_clipboard,
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
fn parse_qr_image(data_url: String) -> Result<String, String> {
    parse_qr_image_impl(data_url)
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
    Ok(true)
}

/// 解码 Google Authenticator 迁移 payload（base64-encoded protobuf）
#[tauri::command]
fn decode_migration_payload(b64_data: String) -> Result<Vec<MigrationAccount>, String> {
    decode_migration_payload_impl(b64_data)
}

#[tauri::command]
async fn copy_to_clipboard(app: tauri::AppHandle, text: String) -> Result<(), String> {
    use tauri_plugin_clipboard_manager::ClipboardExt;
    app.clipboard().write_text(&text).map_err(|e| format!("复制失败：{:?}", e))
}
