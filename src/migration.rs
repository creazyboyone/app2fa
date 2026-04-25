//! Google Authenticator 迁移数据解析
//!
//! 解析 otpauth-migration://offline?data=... 格式的二维码

use crate::models::{Account, MigrationAccount, OtpParams};

/// 解码 Google Authenticator 迁移 payload
pub fn decode_migration_payload(b64_data: &str) -> Result<Vec<MigrationAccount>, String> {
    let b64_data = url_decode(b64_data);

    #[allow(deprecated)]
    let raw = base64::decode(&b64_data).map_err(|e| format!("Base64 解码失败：{:?}", e))?;

    let mut offset = 0usize;
    let mut accounts = Vec::new();

    while offset < raw.len() {
        let (tag, new_off) = decode_varint(&raw, offset)?;
        offset = new_off;

        let field_num = (tag >> 3) as u32;
        let wire_type = (tag & 7) as u8;

        // 跳过非 OtpParameters 字段
        if field_num != 1 {
            offset = skip_field(&raw, offset, wire_type, field_num)?;
            continue;
        }

        let (msg_len, new_offset) = decode_varint(&raw, offset)?;
        offset = new_offset;

        if offset + msg_len as usize > raw.len() {
            return Err("protobuf 数据截断".to_string());
        }

        let otp = parse_otp_parameters(&raw[offset..offset + msg_len as usize])?;
        offset += msg_len as usize;

        let secret_b32 = base32::encode(base32::Alphabet::Rfc4648 { padding: false }, &otp.secret)
            .to_uppercase();

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

/// 解析 otpauth:// URI
pub fn parse_otpauth_uri(uri: &str) -> Result<Account, String> {
    if !uri.starts_with("otpauth://totp/") {
        return Err("不支持的 URI 格式：必须是 otpauth://totp/".to_string());
    }

    let uri = uri.trim_start_matches("otpauth://totp/");
    let mut parts = uri.split('?');
    let label_part = parts.next().unwrap_or("");
    let params_part = parts.next().unwrap_or("");

    let decoded_label = percent_decode(label_part)?;
    let at_pos = decoded_label.rfind('@');
    let (name, issuer) = match at_pos {
        Some(pos) => (&decoded_label[..pos], Some(decoded_label[pos + 1..].to_string())),
        None => (decoded_label.as_str(), None),
    };

    let mut secret = String::new();
    for param in params_part.split('&') {
        if let Some(kv) = param.strip_prefix("secret=") {
            secret = percent_decode(kv)?;
        }
    }

    if secret.is_empty() {
        return Err("URI 中缺少 secret 参数".to_string());
    }

    let cleaned_secret = secret.replace([' ', '-'], "").to_uppercase();
    crate::totp::validate_base32(&cleaned_secret)?;

    Ok(Account {
        id: uuid::Uuid::new_v4().to_string(),
        name: name.to_string(),
        issuer,
        secret: cleaned_secret,
    })
}

/// 解析 QR 码图片
pub fn parse_qr_image(data_url: &str) -> Result<String, String> {
    let b64 = match data_url.find(",") {
        Some(pos) => &data_url[pos + 1..],
        None => data_url,
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

// ==================== 内部辅助函数 ====================

fn url_decode(s: &str) -> String {
    let mut result = Vec::new();
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Ok(byte) = u8::from_str_radix(&s[i + 1..i + 3], 16) {
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

fn percent_decode(s: &str) -> Result<String, String> {
    let mut result = String::new();
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '%' => {
                let hex: String = chars.by_ref().take(2).collect();
                if hex.len() == 2 {
                    if let Ok(byte) = u8::from_str_radix(&hex, 16) {
                        result.push(byte as char);
                        continue;
                    }
                }
                return Err("URL 解码失败".to_string());
            }
            '+' => result.push(' '),
            _ => result.push(c),
        }
    }
    Ok(result)
}

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

fn skip_field(data: &[u8], offset: usize, wire_type: u8, field_num: u32) -> Result<usize, String> {
    match wire_type {
        0 => {
            let (_, o) = decode_varint(data, offset)?;
            Ok(o)
        }
        2 => {
            let (len, o) = decode_varint(data, offset)?;
            Ok(o + len as usize)
        }
        _ => Err(format!("未知 wire type={}，字段={}", wire_type, field_num)),
    }
}

fn parse_otp_parameters(data: &[u8]) -> Result<OtpParams, String> {
    let mut offset = 0usize;
    let mut secret = Vec::new();
    let mut name = String::new();
    let mut issuer = String::new();

    while offset < data.len() {
        let (tag, new_offset) = decode_varint(data, offset)?;
        offset = new_offset;

        let field_num = (tag >> 3) as u32;
        if field_num == 0 {
            return Err("无效字段号".to_string());
        }

        let wire_type = (tag & 7) as u8;

        match field_num {
            1 => {
                if wire_type != 2 {
                    return Err("secret 期望 length-delimited".to_string());
                }
                let (len, o) = decode_varint(data, offset)?;
                offset = o;
                if offset + len as usize > data.len() {
                    return Err("数据截断".to_string());
                }
                secret.extend_from_slice(&data[offset..offset + len as usize]);
                offset += len as usize;
            }
            2 => {
                if wire_type != 2 {
                    return Err("name 期望 length-delimited".to_string());
                }
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
            3 => {
                if wire_type != 2 {
                    return Err("issuer 期望 length-delimited".to_string());
                }
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
                offset = skip_field(data, offset, wire_type, field_num)?;
            }
        }
    }

    Ok(OtpParams { secret, name, issuer })
}
