//! 账户存储

use crate::crypto;
use crate::models::Account;

/// 加载账户列表
pub fn load_accounts() -> Result<Vec<Account>, String> {
    let path = crypto::get_data_path();
    if !path.exists() {
        return Ok(Vec::new());
    }

    let encrypted_data = std::fs::read(&path).map_err(|e| format!("读取数据文件失败：{:?}", e))?;

    // 尝试解密，失败则作为明文 JSON 读取（兼容旧版本）
    let json_data = crypto::decrypt(&encrypted_data).unwrap_or_else(|_| encrypted_data.clone());

    let accounts: Vec<Account> =
        serde_json::from_slice(&json_data).map_err(|e| format!("解析数据失败：{:?}", e))?;

    Ok(accounts
        .into_iter()
        .filter(|a| !a.secret.is_empty() && a.secret.len() >= 8)
        .collect())
}

/// 保存账户列表
pub fn save_accounts(accounts: Vec<Account>) -> Result<(), String> {
    let path = crypto::get_data_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("创建目录失败：{:?}", e))?;
    }

    let json =
        serde_json::to_vec(&accounts).map_err(|e| format!("数据序列化失败：{:?}", e))?;

    let encrypted = crypto::encrypt(&json)?;
    std::fs::write(&path, &encrypted).map_err(|e| format!("写入数据文件失败：{:?}", e))
}

/// 删除账户
pub fn delete_account(id: &str) -> Result<(), String> {
    let mut accounts = load_accounts()?;
    let initial_len = accounts.len();
    accounts.retain(|a| a.id != id);
    if accounts.len() == initial_len {
        return Err(format!("账户不存在：{}", id));
    }
    save_accounts(accounts)
}

/// 更新账户使用记录
pub fn update_usage(id: &str) -> Result<(), String> {
    let mut accounts = load_accounts()?;
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|e| format!("获取时间失败：{:?}", e))?
        .as_secs();

    if let Some(account) = accounts.iter_mut().find(|a| a.id == id) {
        account.usage_count = account.usage_count.saturating_add(1);
        account.last_used_at = Some(now);
        save_accounts(accounts)
    } else {
        Err(format!("账户不存在：{}", id))
    }
}

/// 切换置顶状态
pub fn toggle_pin(id: &str) -> Result<(), String> {
    let mut accounts = load_accounts()?;
    if let Some(account) = accounts.iter_mut().find(|a| a.id == id) {
        account.pinned = !account.pinned;
        save_accounts(accounts)
    } else {
        Err(format!("账户不存在：{}", id))
    }
}
