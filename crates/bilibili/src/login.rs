// Adapted from LiliaBilibili, Copyright (c) 2026 Sena, MIT; see LICENSE-LILIA.
use serde::Deserialize;

use crate::ApiError;
use crate::Client;
use crate::http::parse_envelope;

/// 护照接口源站。
const PASSPORT_ORIGIN: &str = "https://passport.bilibili.com";

/// 扫码登录二维码信息。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QrLogin {
    /// 二维码内容(登录页 url),交由 UI 的 `QrCode` 组件渲染。
    pub url: String,
    /// 扫码登录秘钥(轮询用,180 秒过期)。
    pub qrcode_key: String,
}

/// 扫码轮询状态。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QrStatus {
    /// 未扫码(86101)。
    Waiting,
    /// 已扫码未确认(86090)。
    Scanned,
    /// 二维码已失效,需重新生成(86038)。
    Expired,
    /// 登录成功,凭据已写入客户端(0)。
    Confirmed,
}

/// 一次扫码轮询的结果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QrPoll {
    /// 扫码状态。
    pub status: QrStatus,
    /// 服务端状态信息。
    pub message: String,
}

/// 轮询响应 `data` 中与状态相关的字段。
#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct QrPollData {
    code: i64,
    message: String,
}

/// 业务码到扫码状态的映射;未知码为 `None`。
fn status_from_code(code: i64) -> Option<QrStatus> {
    match code {
        86101 => Some(QrStatus::Waiting),
        86090 => Some(QrStatus::Scanned),
        86038 => Some(QrStatus::Expired),
        0 => Some(QrStatus::Confirmed),
        _ => None,
    }
}

impl Client {
    /// 申请扫码登录二维码。
    pub fn qr_generate(&self) -> Result<QrLogin, ApiError> {
        self.ensure_buvid()?;
        let url = format!("{PASSPORT_ORIGIN}/x/passport-login/web/qrcode/generate");
        let data = self.get_json(&url)?.into_data()?;
        Ok(QrLogin {
            url: data["url"]
                .as_str()
                .ok_or_else(|| ApiError::Parse("qrcode/generate 缺少 url".into()))?
                .to_string(),
            qrcode_key: data["qrcode_key"]
                .as_str()
                .ok_or_else(|| ApiError::Parse("qrcode/generate 缺少 qrcode_key".into()))?
                .to_string(),
        })
    }

    /// 轮询扫码状态;状态为 `Confirmed` 时凭据已存入客户端内存。
    pub fn qr_poll(&self, qrcode_key: &str) -> Result<QrPoll, ApiError> {
        let url = format!(
            "{PASSPORT_ORIGIN}/x/passport-login/web/qrcode/poll?qrcode_key={}",
            crate::wbi::wbi_encode(qrcode_key)
        );
        let raw = self.send(&url)?;
        let data = parse_envelope(&raw.body)?.into_data()?;
        let poll: QrPollData = serde_json::from_value(data)
            .map_err(|_| ApiError::Parse("missing QR status".into()))?;
        let status = status_from_code(poll.code).ok_or_else(|| ApiError::Business {
            code: poll.code,
            message: poll.message.clone(),
        })?;
        if status == QrStatus::Confirmed {
            let mut jar = crate::cookie::CookieJar::new();
            for set_cookie in &raw.set_cookies {
                jar.absorb(set_cookie);
            }
            if !self.replace_login(jar) {
                return Err(ApiError::Parse("扫码确认未包含完整登录凭据".into()));
            }
        }
        Ok(QrPoll {
            status,
            message: poll.message,
        })
    }

    /// Restore the cookie header stored by the account service.
    /// 返回身份替换是否生效；持久化由账号服务单独处理。
    pub(crate) fn import_cookies(&self, text: &str) -> bool {
        let mut jar = crate::cookie::CookieJar::new();
        jar.absorb(text);
        self.replace_login(jar)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn invalid_import_cannot_mix_accounts() {
        let client = Client::in_memory();
        client.jar().set("buvid3", "device");
        assert!(client.import_cookies("SESSDATA=old; bili_jct=old; DedeUserID=7"));
        assert!(!client.import_cookies("SESSDATA=new"));
        assert_eq!(client.mid(), Some(7));
        let request = client.snapshot();
        assert!(client.import_cookies("SESSDATA=new; bili_jct=new; DedeUserID=8"));
        assert_eq!(request.mid(), Some(7));
        assert_eq!(client.mid(), Some(8));
        assert_eq!(client.jar().get("buvid3"), Some("device"));
    }
    #[test]
    fn qr_lifecycle_distinguishes_expiry_and_confirmation() {
        assert_eq!(status_from_code(86101), Some(QrStatus::Waiting));
        assert_eq!(status_from_code(86090), Some(QrStatus::Scanned));
        assert_eq!(status_from_code(86038), Some(QrStatus::Expired));
        assert_eq!(status_from_code(0), Some(QrStatus::Confirmed));
        assert_eq!(status_from_code(-1), None);
    }

    #[test]
    #[ignore = "live endpoint smoke test; creates an unconfirmed QR login attempt"]
    fn live_qr_generation_and_polling() {
        let client = Client::in_memory();
        let qr = client.qr_generate().expect("QR endpoint must respond");
        assert!(qr.url.starts_with("https://"));
        assert!(!qr.qrcode_key.is_empty());
        let poll = client
            .qr_poll(&qr.qrcode_key)
            .expect("QR polling must respond");
        assert_eq!(poll.status, QrStatus::Waiting);
        assert!(!client.is_logged_in());
    }
}
