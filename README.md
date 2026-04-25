# TOTP Manager

A Windows-only 2FA TOTP key manager built with Tauri 2 (Rust backend + vanilla JS frontend).

## Features

- **TOTP Code Generation** - Generate 6-digit time-based one-time passwords
- **Multiple Import Methods**:
  - Manual key entry (Base32 format)
  - QR code image import (supports multi-select)
  - Camera scanning
- **Google Authenticator Migration** - Import accounts from Google Authenticator export QR codes
- **Secure Storage** - Data encrypted with Windows DPAPI
- **System Tray** - Minimize to tray after copying, click tray icon to restore
- **System Notifications** - Get notified when codes are copied
- **Account Deduplication** - Automatic detection of duplicate accounts

## Build Requirements

- Rust 1.75+
- Node.js 18+
- Windows 10/11

## Development

```bash
# Install dependencies
npm install

# Run in development mode
cargo tauri dev

# Build for production (creates NSIS installer + APPX)
cargo tauri build
```

## Data Storage

Account data is stored at:
```
%APPDATA%\totp-manager\keys.bin
```

Data is encrypted using Windows DPAPI (Data Protection API), which ties the encryption to the current Windows user account.

## Tech Stack

- **Backend**: Rust + Tauri 2
- **Frontend**: Vanilla JavaScript + HTML/CSS
- **TOTP**: [otpauth](https://crates.io/crates/otpauth) crate
- **QR Scanning**: [jsQR](https://github.com/cozmo/jsQR) (frontend) + [rxing](https://crates.io/crates/rxing) (backend)
- **Encryption**: Windows DPAPI

## Project Structure

```
src/
├── main.rs        # 应用入口、系统托盘
├── models.rs      # 数据结构定义
├── crypto.rs      # Windows DPAPI 加解密
├── totp.rs        # TOTP 生成与验证
├── storage.rs     # 账户存储
├── migration.rs   # Google Authenticator 迁移解析
├── commands.rs    # Tauri 命令
└── tauri/         # Tauri 配置
    └── capabilities/
        └── default.json
```

## License

MIT
