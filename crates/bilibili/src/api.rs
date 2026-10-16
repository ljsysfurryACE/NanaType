use crate::{
    ApiError, Client,
    live_ws::{DanmakuInfo, LiveHost, PROTOVER_BROTLI},
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
const LIVE: &str = "https://api.live.bilibili.com";
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Profile {
    pub uid: u64,
    pub name: String,
    pub avatar_url: String,
    pub room_id: Option<u64>,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Room {
    pub id: u64,
    pub owner: u64,
    pub title: String,
    pub max_chars: usize,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SendReceipt {
    pub id: Option<String>,
}
impl Client {
    pub fn profile(&self) -> Result<Profile, ApiError> {
        self.require_login()?;
        let d = self
            .get_json("https://api.bilibili.com/x/web-interface/nav")?
            .into_data()?;
        if d["isLogin"] != true {
            return Err(ApiError::LoginRequired);
        }
        let uid = d["mid"]
            .as_u64()
            .ok_or_else(|| ApiError::Parse("missing account identity".into()))?;
        let room = self
            .get_json(&format!("{LIVE}/live_user/v1/Master/info?uid={uid}"))
            .and_then(|v| v.into_data())
            .ok();
        let room_id = room
            .and_then(|d| d["room_id"].as_u64())
            .filter(|id| *id > 0);
        Ok(Profile {
            uid,
            name: d["uname"].as_str().unwrap_or_default().into(),
            avatar_url: d["face"].as_str().unwrap_or_default().into(),
            room_id,
        })
    }
    pub fn room(&self, id: u64) -> Result<Room, ApiError> {
        let d = self
            .get_json(&format!("{LIVE}/room/v1/Room/get_info?room_id={id}"))?
            .into_data()?;
        let id = d["room_id"]
            .as_u64()
            .filter(|i| *i > 0)
            .ok_or_else(|| ApiError::Parse("invalid room".into()))?;
        let length = self
            .get_json(&format!(
                "{LIVE}/xlive/web-room/v1/index/getInfoByUser?room_id={id}"
            ))
            .and_then(|v| v.into_data())
            .ok()
            .and_then(|v| v["property"]["danmu"]["length"].as_u64())
            .filter(|n| *n > 0 && *n <= 1000)
            .unwrap_or(20) as usize;
        Ok(Room {
            id,
            owner: d["uid"].as_u64().unwrap_or_default(),
            title: d["title"].as_str().unwrap_or_default().into(),
            max_chars: length,
        })
    }
    pub(crate) fn danmu_info(&self, room: u64) -> Result<DanmakuInfo, ApiError> {
        self.require_login()?;
        let d = self.signed_get(
            &format!("{LIVE}/xlive/web-room/v1/index/getDanmuInfo"),
            vec![("id".into(), room.to_string()), ("type".into(), "0".into())],
        )?;
        let hosts = d["host_list"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|v| {
                Some(LiveHost {
                    host: v["host"].as_str()?.into(),
                    wss_port: u16::try_from(v["wss_port"].as_u64()?).ok()?,
                })
            })
            .collect();
        Ok(DanmakuInfo {
            room_id: room as i64,
            uid: self.mid().unwrap_or_default(),
            token: d["token"]
                .as_str()
                .ok_or_else(|| ApiError::Parse("missing chat token".into()))?
                .into(),
            buvid: self.jar().get("buvid3").map(str::to_owned),
            hosts,
            protover: PROTOVER_BROTLI,
        })
    }
    pub fn send_danmaku(&self, room: u64, text: &str) -> Result<SendReceipt, ApiError> {
        self.require_login()?;
        if room == 0 || text.trim().is_empty() {
            return Err(ApiError::InvalidInput("empty danmaku".into()));
        }
        let csrf = self.jar().get("bili_jct").unwrap_or_default().to_owned();
        let mut fields = vec![
            ("roomid".into(), room.to_string()),
            ("msg".into(), text.into()),
            ("csrf".into(), csrf.clone()),
            ("csrf_token".into(), csrf),
            (
                "rnd".into(),
                (nanatype_model::unix_millis() / 1000).to_string(),
            ),
        ];
        for (k, v) in [
            ("mode", "1"),
            ("color", "16777215"),
            ("fontsize", "25"),
            ("bubble", "0"),
        ] {
            fields.push((k.into(), v.into()));
        }
        let raw = self.send_form(
            &format!("{LIVE}/msg/send"),
            &crate::wbi::encode_pairs(&fields),
        )?;
        parse_receipt(&raw.body)
    }
}
pub(crate) fn parse_receipt(body: &[u8]) -> Result<SendReceipt, ApiError> {
    let envelope = crate::http::parse_envelope(body)?;
    // Some rejected submissions use code=0 with a nonempty message/msg.
    let message = if envelope.msg.is_empty() {
        &envelope.message
    } else {
        &envelope.msg
    };
    if envelope.code == 0 && !matches!(message.as_str(), "" | "0" | "ok") {
        return Err(ApiError::Business {
            code: 0,
            message: message.clone(),
        });
    }
    let d = envelope.into_data()?;
    let extra: Value = d["mode_info"]["extra"]
        .as_str()
        .and_then(|s| serde_json::from_str(s).ok())
        .unwrap_or_default();
    let id = extra["id_str"]
        .as_str()
        .or_else(|| d["dmid_str"].as_str())
        .filter(|s| !s.is_empty())
        .map(str::to_owned);
    Ok(SendReceipt { id })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn receipts_preserve_server_identity_and_rejections() {
        let receipt = parse_receipt(
            br#"{"code":0,"data":{"mode_info":{"extra":"{\"id_str\":\"echo-1\"}"}},"message":""}"#,
        )
        .unwrap();
        assert_eq!(receipt.id.as_deref(), Some("echo-1"));
        assert_eq!(parse_receipt(br#"{"code":0,"data":{}}"#).unwrap().id, None);
        assert!(matches!(
            parse_receipt(br#"{"code":10031,"message":"too fast"}"#),
            Err(ApiError::Business { code: 10031, .. })
        ));
        assert!(matches!(
            parse_receipt(br#"{"code":0,"msg":"rejected"}"#),
            Err(ApiError::Business { .. })
        ));
        assert!(parse_receipt(b"interrupted").is_err());
        assert!(parse_receipt(b"{}").is_err());
    }
}
