//! Windows DPAPI 加密支持

#[cfg(windows)]
use windows::Win32::Security::Cryptography::*;

/// 获取数据文件路径
pub fn get_data_path() -> std::path::PathBuf {
    let appdata = std::env::var("APPDATA").unwrap_or_else(|_| String::from("%APPDATA%"));
    let mut path = std::path::PathBuf::from(appdata);
    path.push("totp-manager");
    path.push("keys.bin");
    path
}

/// 使用 Windows DPAPI 加密数据
#[cfg(windows)]
pub fn encrypt(data: &[u8]) -> Result<Vec<u8>, String> {
    unsafe {
        let input = CRYPT_INTEGER_BLOB {
            cbData: data.len() as u32,
            pbData: data.as_ptr() as *mut u8,
        };
        let mut output = CRYPT_INTEGER_BLOB::default();
        if CryptProtectData(&input, None, None, None, None, CRYPTPROTECT_UI_FORBIDDEN, &mut output).is_ok() {
            let result = std::slice::from_raw_parts(output.pbData, output.cbData as usize).to_vec();
            let _ = windows::Win32::Foundation::LocalFree(windows::Win32::Foundation::HLOCAL(output.pbData as _));
            Ok(result)
        } else {
            Err("加密失败".to_string())
        }
    }
}

/// 使用 Windows DPAPI 解密数据
#[cfg(windows)]
pub fn decrypt(data: &[u8]) -> Result<Vec<u8>, String> {
    unsafe {
        let input = CRYPT_INTEGER_BLOB {
            cbData: data.len() as u32,
            pbData: data.as_ptr() as *mut u8,
        };
        let mut output = CRYPT_INTEGER_BLOB::default();
        if CryptUnprotectData(&input, None, None, None, None, CRYPTPROTECT_UI_FORBIDDEN, &mut output).is_ok() {
            let result = std::slice::from_raw_parts(output.pbData, output.cbData as usize).to_vec();
            let _ = windows::Win32::Foundation::LocalFree(windows::Win32::Foundation::HLOCAL(output.pbData as _));
            Ok(result)
        } else {
            Err("解密失败".to_string())
        }
    }
}

/// 非 Windows 平台的空实现
#[cfg(not(windows))]
pub fn encrypt(_data: &[u8]) -> Result<Vec<u8>, String> {
    Err("仅支持 Windows 平台".to_string())
}

#[cfg(not(windows))]
pub fn decrypt(_data: &[u8]) -> Result<Vec<u8>, String> {
    Err("仅支持 Windows 平台".to_string())
}

#[cfg(not(windows))]
pub fn get_data_path() -> std::path::PathBuf {
    let mut path = std::env::temp_dir();
    path.push("totp-manager");
    path.push("keys.bin");
    path
}
