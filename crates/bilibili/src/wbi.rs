// Adapted from LiliaBilibili, Copyright (c) 2026 Sena, MIT; see LICENSE-LILIA.
use md5::{Digest, Md5};

/// WBI 签名密钥对,来自 nav 接口 `wbi_img.img_url/sub_url` 的文件名。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WbiKeys {
    /// `img_url` 文件名。
    pub img_key: String,
    /// `sub_url` 文件名。
    pub sub_key: String,
}

/// 重排映射表,来自 bilibili-API-collect 的 WBI 签名文档。
const MIXIN_KEY_ENC_TAB: [usize; 64] = [
    46, 47, 18, 2, 53, 8, 23, 32, 15, 50, 10, 31, 58, 3, 45, 35, 27, 43, 5, 49, 33, 9, 42, 19, 29,
    28, 14, 39, 12, 38, 41, 13, 37, 48, 7, 16, 24, 55, 40, 61, 26, 17, 0, 1, 60, 51, 30, 4, 22, 25,
    54, 21, 56, 59, 6, 63, 57, 62, 11, 36, 20, 34, 44, 52,
];

/// 从 nav 接口的两个 url 提取密钥:取文件名(去掉扩展名)。
pub(crate) fn keys_from_nav(img_url: &str, sub_url: &str) -> Option<WbiKeys> {
    Some(WbiKeys {
        img_key: key_from_url(img_url)?,
        sub_key: key_from_url(sub_url)?,
    })
}

fn key_from_url(url: &str) -> Option<String> {
    let file = url.rsplit('/').next()?;
    let (key, _) = file.split_once('.')?;
    Some(key.to_string())
}

/// 拼接 img_key + sub_key 后按置换表重排,取前 32 位。
pub fn mixin_key(img_key: &str, sub_key: &str) -> String {
    let mut raw = Vec::with_capacity(img_key.len() + sub_key.len());
    raw.extend_from_slice(img_key.as_bytes());
    raw.extend_from_slice(sub_key.as_bytes());
    MIXIN_KEY_ENC_TAB
        .iter()
        .filter_map(|&index| raw.get(index).copied())
        .map(char::from)
        .take(32)
        .collect()
}

/// 按 wbi 规则编码键/值:`encodeURIComponent` 语义,十六进制大写,空格编码为 `%20`。
pub fn wbi_encode(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for &byte in value.as_bytes() {
        match byte {
            b'A'..=b'Z'
            | b'a'..=b'z'
            | b'0'..=b'9'
            | b'-'
            | b'_'
            | b'.'
            | b'!'
            | b'~'
            | b'*'
            | b'\''
            | b'('
            | b')' => out.push(char::from(byte)),
            other => out.push_str(&format!("%{other:02X}")),
        }
    }
    out
}

/// 过滤签名值中的 `!'()*` 字符(文档规定的预处理)。
fn filter_value(value: &str) -> String {
    value
        .chars()
        .filter(|c| !matches!(c, '!' | '\'' | '(' | ')' | '*'))
        .collect()
}

/// 对请求参数做 WBI 签名:值剔除 `!'()*`、追加 `wts`、按键升序、
/// `encodeURIComponent` 编码后拼接 mixin key,取 MD5 得 `w_rid`。
///
/// `wts` 与 `w_rid` 会追加进 `params`(保持原顺序),返回 `w_rid`。
pub fn sign(params: &mut Vec<(String, String)>, keys: &WbiKeys, wts: u64) -> String {
    let mut signed: Vec<(String, String)> = params
        .iter()
        .map(|(key, value)| (key.clone(), filter_value(value)))
        .chain(std::iter::once(("wts".to_string(), wts.to_string())))
        .collect();
    signed.sort_by(|a, b| a.0.cmp(&b.0));
    let query = signed
        .iter()
        .map(|(key, value)| format!("{}={}", wbi_encode(key), wbi_encode(value)))
        .collect::<Vec<_>>()
        .join("&");
    let mixin = mixin_key(&keys.img_key, &keys.sub_key);
    let w_rid = md5_hex(format!("{query}{mixin}").as_bytes());
    params.push(("wts".to_string(), wts.to_string()));
    params.push(("w_rid".to_string(), w_rid.clone()));
    w_rid
}

/// 普通查询串/表单编码(空格编码为 `%20`,十六进制大写)。
pub(crate) fn encode_pairs(pairs: &[(String, String)]) -> String {
    pairs
        .iter()
        .map(|(key, value)| format!("{}={}", wbi_encode(key), wbi_encode(value)))
        .collect::<Vec<_>>()
        .join("&")
}

pub(crate) fn md5_hex(data: &[u8]) -> String {
    let mut hasher = Md5::new();
    hasher.update(data);
    hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}
