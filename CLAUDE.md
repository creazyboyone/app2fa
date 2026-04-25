# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

**TOTP Manager** — a Windows-only 2FA TOTP key manager built with Tauri 2 (Rust backend + vanilla JS frontend). Data is stored as JSON in `%APPDATA%/totp-manager/keys.bin`.

## Architecture

- **`src/main.rs`** — Single-file Rust backend. Contains all data models (`Account`, `TOTPResult`), business logic functions (`*_impl`), and Tauri command wrappers (`#[tauri::command]`). No modular separation currently.
- **`dist/`** — Frontend: `index.html` (UI + jsQR CDN), `app.js` (vanilla JS state management, Tauri invoke calls, DOM rendering), `styles.css`.
- **`src/tauri/capabilities/default.json`** — Tauri permissions for the main window.

Key dependencies: `otpauth` (TOTP generation), `rxing` (Windows Hello), `image` (QR parsing, stub), `zeroize` (secret zeroization).

## Commands

```bash
# Build and run the app in development mode
npx tauri dev

# Build for production (creates NSIS installer + APPX)
npx tauri build
```

No test suite exists. No linter or formatter is configured beyond Rust's built-in `cargo check`.

## Pending / TODOs

- **手动导入准确性验证** — 待验证：手动输入密钥后生成的 TOTP 码是否与官方 authenticator app（Google Authenticator、Microsoft Authenticator）一致。需要对比测试多个常见 issuer（GitHub、Google、Microsoft 等）。
- **图片导入功能** — 下一步实现。当前 `parse_qr_image` 命令为 stub，返回"开发中"提示。

## Key Implementation Details

- All Tauri commands are in `src/main.rs` — both the `*_impl` functions and their `#[tauri::command]` wrappers.
- Frontend uses a custom `invoke` binding to `window.__TAURI__.core.invoke` (Tauri 2 API). Commands called: `load_accounts`, `save_accounts`, `generate_totp`, `add_account`, `delete_account`, `parse_otpauth_uri`, `verify_windows_hello`, `parse_qr_image`.
- TOTP codes refresh every 1 second on the frontend via `setInterval`, calling `generate_totp` backend command.
- QR scanning uses jsQR (loaded from CDN) to scan camera frames — no backend involvement.
- The `skip-windows-hello` feature flag exists but Windows Hello verification is currently stubbed out (`verify_windows_hello` always returns `true`).
