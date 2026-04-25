//! TOTP 相关功能

use crate::models::{Account, TotpResult};

/// 严格验证 Base32 字符串格式
pub fn validate_base32(s: &str) -> Result<(), String> {
    let cleaned = s.replace([' ', '-'], "").to_uppercase();
    if cleaned.is_empty() || cleaned.len() < 8 {
        return Err("密钥无效：长度不足".to_string());
    }

    for c in cleaned.chars() {
        if !(c.is_ascii_uppercase() || ('2'..='7').contains(&c)) {
            return Err(format!("密钥包含无效字符：{}。Base32 只允许 A-Z 和 2-7", c));
        }
    }

    if otpauth::TOTP::from_base32(&cleaned).is_none() {
        return Err("无效的 Base32 密钥格式".to_string());
    }

    Ok(())
}

/// 生成 TOTP 验证码
pub fn generate(secret: &str) -> Result<TotpResult, String> {
    let cleaned_secret = secret.replace([' ', '-'], "").to_uppercase();
    validate_base32(&cleaned_secret)?;

    let Some(totp) = otpauth::TOTP::from_base32(&cleaned_secret) else {
        return Err("无效的 Base32 密钥格式".to_string());
    };

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|e| format!("时间错误：{:?}", e))?
        .as_secs();

    let code = totp.generate(30, now);

    Ok(TotpResult {
        code: format!("{:06}", code),
        remaining_seconds: (30 - now % 30) as u32,
    })
}

/// 创建新账户
pub fn create_account(name: String, issuer: Option<String>, secret: String) -> Result<Account, String> {
    let cleaned_secret = secret.replace([' ', '-'], "").to_uppercase();
    validate_base32(&cleaned_secret)?;
    Ok(Account {
        id: uuid::Uuid::new_v4().to_string(),
        name,
        issuer,
        secret: cleaned_secret,
    })
}
