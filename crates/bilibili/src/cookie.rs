// Copyright (c) 2026 Sena. MIT; see ../../../LICENSE-MIT.
use std::collections::BTreeMap;

/// Cookie 保留的属性名(出现在 `Set-Cookie` 行尾),永不作为真实 cookie 名吸收。
const ATTRIBUTE_NAMES: [&str; 11] = [
    "path",
    "domain",
    "expires",
    "max-age",
    "secure",
    "httponly",
    "samesite",
    "partitioned",
    "priority",
    "comment",
    "version",
];

/// 构成有效登录态所需的三个 cookie 名。
const LOGIN_COOKIES: [&str; 3] = ["SESSDATA", "bili_jct", "DedeUserID"];

/// 进程内 Cookie 容器:仅保存名值对,渲染为 `Cookie` 请求头。
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct CookieJar {
    cookies: BTreeMap<String, String>,
}

impl CookieJar {
    /// 空容器。
    pub fn new() -> Self {
        Self::default()
    }

    /// 写入一个 cookie。
    pub fn set(&mut self, name: impl Into<String>, value: impl Into<String>) {
        self.cookies.insert(name.into(), value.into());
    }

    /// 读取一个 cookie。
    pub fn get(&self, name: &str) -> Option<&str> {
        self.cookies.get(name).map(String::as_str)
    }

    /// 清空。
    pub(crate) fn clear(&mut self) {
        self.cookies.clear();
    }

    /// Read persisted Cookie headers or one Set-Cookie value from the login response.
    pub fn absorb(&mut self, text: &str) {
        for segment in text.split([';', '\n', '\r']) {
            let Some((name, value)) = segment.split_once('=') else {
                continue;
            };
            let name = name.trim();
            if !name.is_empty() && !ATTRIBUTE_NAMES.contains(&name.to_ascii_lowercase().as_str()) {
                self.set(name, value.trim());
            }
        }
    }

    /// 渲染 `Cookie` 请求头;容器为空时返回 `None`。
    pub fn header_value(&self) -> Option<String> {
        if self.cookies.is_empty() {
            return None;
        }
        let pairs: Vec<String> = self
            .cookies
            .iter()
            .map(|(name, value)| format!("{name}={value}"))
            .collect();
        Some(pairs.join("; "))
    }

    /// 是否构成有效登录态(三要素齐备)。
    pub fn is_logged_in(&self) -> bool {
        LOGIN_COOKIES
            .iter()
            .all(|name| self.cookies.contains_key(*name))
    }

    /// 登录用户 mid(未登录为 `None`)。
    pub fn mid(&self) -> Option<i64> {
        self.get("DedeUserID").and_then(|value| value.parse().ok())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn login_headers_round_trip_without_cookie_attributes() {
        let mut jar = CookieJar::new();
        for header in [
            "SESSDATA=fixture%3Dvalue; Path=/; HttpOnly; Secure; SameSite=None",
            "bili_jct=fixture; Domain=.bilibili.com; Max-Age=1800",
            "DedeUserID=7; Expires=Wed, 21 Oct 2026 07:28:00 GMT",
        ] {
            jar.absorb(header);
        }
        assert!(jar.is_logged_in());
        assert_eq!(jar.mid(), Some(7));
        let mut restored = CookieJar::new();
        restored.absorb(&jar.header_value().unwrap());
        assert_eq!(restored, jar);
        assert_eq!(jar.cookies.len(), 3);
    }
}
