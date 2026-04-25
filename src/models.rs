use serde::{Deserialize, Serialize};

/// 用户账户
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Account {
    pub id: String,
    pub name: String,
    pub issuer: Option<String>,
    pub secret: String,
}

/// TOTP 生成结果
#[derive(Debug, Serialize, Deserialize)]
pub struct TotpResult {
    pub code: String,
    pub remaining_seconds: u32,
}

/// 从 Google Authenticator 迁移数据解析出的账户
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MigrationAccount {
    pub name: String,
    pub issuer: Option<String>,
    /// Base32 编码的密钥
    pub secret_b32: String,
}

/// OtpParameters 的简化解析结果
pub struct OtpParams {
    pub secret: Vec<u8>,
    pub name: String,
    pub issuer: String,
}
