# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

**TOTP Manager** — a Windows-only 2FA TOTP key manager built with Tauri 2 (Rust backend + vanilla JS frontend). Data is encrypted with Windows DPAPI and stored in `%APPDATA%/totp-manager/keys.bin`.

## Architecture

### Backend (Rust)

```
src/
├── main.rs        # 应用入口、系统托盘初始化
├── models.rs      # 数据结构：Account, TotpResult, MigrationAccount
├── crypto.rs      # Windows DPAPI 加解密
├── totp.rs        # TOTP 生成与 Base32 验证
├── storage.rs     # 账户存储（加载/保存/删除）
├── migration.rs   # Google Authenticator 迁移解析、otpauth URI 解析、QR 图片解析
└── commands.rs    # Tauri 命令封装
```

### Frontend (vanilla JS)

- `dist/index.html` — UI 结构，jsQR CDN 引入
- `dist/app.js` — 状态管理、Tauri invoke 调用、DOM 渲染
- `dist/styles.css` — Tokyo Night 主题样式

### Tauri 配置

- `src/tauri/capabilities/default.json` — 窗口权限配置
- `tauri.conf.json` — 应用配置（窗口尺寸 560px，禁止最大化）

## Commands

```bash
# 开发模式
cargo tauri dev

# 生产构建（生成 NSIS 安装包 + APPX）
cargo tauri build
```

## Key Dependencies

- `otpauth` — TOTP 算法实现
- `rxing` — QR 码识别（后端图片解析）
- `image` — 图片加载
- `base32` — Base32 编码
- `windows` — Windows DPAPI 加密

## Implementation Details

- **数据加密**：使用 Windows DPAPI 的 `CryptProtectData`/`CryptUnprotectData`，绑定当前 Windows 用户
- **TOTP 刷新**：前端每秒通过 `setInterval` 调用 `generate_totp`
- **QR 扫描**：
  - 摄像头：前端 jsQR 库
  - 图片：后端 rxing 库
- **Google Authenticator 迁移**：手动解析 protobuf 格式的 `otpauth-migration://` URI
- **系统托盘**：复制验证码后最小化，点击托盘图标恢复窗口
