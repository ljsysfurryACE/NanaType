use crate::{ApiError, cookie::CookieJar, wbi::WbiKeys};
use serde::Deserialize;
use serde_json::Value;
use std::sync::{Mutex, MutexGuard};
use std::time::{Duration, Instant};

pub(crate) const USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/126.0.0.0 Safari/537.36";
pub struct Client {
    agent: ureq::Agent,
    jar: Mutex<CookieJar>,
    keys: Mutex<Option<(WbiKeys, Instant)>>,
}
#[derive(Default, Deserialize)]
pub(crate) struct Envelope {
    pub code: i64,
    #[serde(default)]
    pub message: String,
    #[serde(default)]
    pub msg: String,
    #[serde(default)]
    pub data: Value,
}
impl Envelope {
    pub fn into_data(self) -> Result<Value, ApiError> {
        if self.code != 0 {
            return Err(ApiError::Business {
                code: self.code,
                message: if self.message.is_empty() {
                    self.msg
                } else {
                    self.message
                },
            });
        }
        Ok(self.data)
    }
}
pub(crate) struct RawResponse {
    pub body: Vec<u8>,
    pub set_cookies: Vec<String>,
}
pub(crate) fn parse_envelope(body: &[u8]) -> Result<Envelope, ApiError> {
    serde_json::from_slice(body).map_err(|_| ApiError::Parse("invalid API response".into()))
}
impl Default for Client {
    fn default() -> Self {
        Self::in_memory()
    }
}
impl Client {
    pub fn in_memory() -> Self {
        Self {
            agent: ureq::Agent::config_builder()
                .timeout_global(Some(Duration::from_secs(10)))
                .max_redirects(0)
                .http_status_as_error(false)
                .build()
                .into(),
            jar: Mutex::new(CookieJar::new()),
            keys: Mutex::new(None),
        }
    }
    pub fn snapshot(&self) -> Self {
        Self {
            agent: self.agent.clone(),
            jar: Mutex::new(self.jar().clone()),
            keys: Mutex::new(None),
        }
    }
    pub(crate) fn jar(&self) -> MutexGuard<'_, CookieJar> {
        self.jar.lock().unwrap_or_else(|p| p.into_inner())
    }
    pub fn mid(&self) -> Option<i64> {
        self.jar().mid()
    }
    pub fn is_logged_in(&self) -> bool {
        self.jar().is_logged_in()
    }
    pub fn require_login(&self) -> Result<(), ApiError> {
        if self.is_logged_in() {
            Ok(())
        } else {
            Err(ApiError::LoginRequired)
        }
    }
    pub(crate) fn replace_login(&self, mut jar: CookieJar) -> bool {
        if !jar.is_logged_in() {
            return false;
        }
        let mut previous = self.jar();
        // Device cookies belong to this client; account cookies come only from the new login.
        for name in ["buvid3", "buvid4"] {
            if jar.get(name).is_none()
                && let Some(value) = previous.get(name)
            {
                jar.set(name, value);
            }
        }
        *previous = jar;
        *self.keys.lock().unwrap_or_else(|p| p.into_inner()) = None;
        true
    }
    pub(crate) fn restore(&self, bytes: &[u8]) -> Result<(), ApiError> {
        let text = std::str::from_utf8(bytes)
            .map_err(|_| ApiError::Io("invalid saved credentials".into()))?;
        self.import_cookies(text)
            .then_some(())
            .ok_or_else(|| ApiError::Io("incomplete saved credentials".into()))
    }
    pub(crate) fn credentials(&self) -> Vec<u8> {
        self.jar().header_value().unwrap_or_default().into_bytes()
    }
    pub(crate) fn clear(&self) {
        self.jar().clear();
    }
    pub(crate) fn send(&self, url: &str) -> Result<RawResponse, ApiError> {
        let mut req = self
            .agent
            .get(url)
            .header("User-Agent", USER_AGENT)
            .header("Referer", "https://live.bilibili.com/");
        if let Some(cookie) = self.jar().header_value() {
            req = req.header("Cookie", &cookie);
        }
        finish(req.call())
    }
    pub(crate) fn send_form(&self, url: &str, form: &str) -> Result<RawResponse, ApiError> {
        let mut req = self
            .agent
            .post(url)
            .header("User-Agent", USER_AGENT)
            .header("Referer", "https://live.bilibili.com/")
            .header("Content-Type", "application/x-www-form-urlencoded");
        if let Some(cookie) = self.jar().header_value() {
            req = req.header("Cookie", &cookie);
        }
        finish(req.send(form.as_bytes()))
    }
    pub(crate) fn get_json(&self, url: &str) -> Result<Envelope, ApiError> {
        parse_envelope(&self.send(url)?.body)
    }
    pub fn ensure_buvid(&self) -> Result<(), ApiError> {
        if self.jar().get("buvid3").is_some() {
            return Ok(());
        }
        let data = self
            .get_json("https://api.bilibili.com/x/frontend/finger/spi")?
            .into_data()?;
        let mut jar = self.jar();
        for (key, field) in [("buvid3", "b_3"), ("buvid4", "b_4")] {
            if let Some(v) = data[field].as_str() {
                jar.set(key, v);
            }
        }
        Ok(())
    }
    fn wbi_keys(&self) -> Result<WbiKeys, ApiError> {
        if let Some((keys, at)) = &*self.keys.lock().unwrap_or_else(|p| p.into_inner())
            && at.elapsed() < Duration::from_secs(3600)
        {
            return Ok(keys.clone());
        }
        let d = self
            .get_json("https://api.bilibili.com/x/web-interface/nav")?
            .into_data()?;
        let k = crate::wbi::keys_from_nav(
            d["wbi_img"]["img_url"].as_str().unwrap_or_default(),
            d["wbi_img"]["sub_url"].as_str().unwrap_or_default(),
        )
        .ok_or_else(|| ApiError::Parse("missing WBI keys".into()))?;
        *self.keys.lock().unwrap_or_else(|p| p.into_inner()) = Some((k.clone(), Instant::now()));
        Ok(k)
    }
    pub(crate) fn signed_get(
        &self,
        url: &str,
        params: Vec<(String, String)>,
    ) -> Result<Value, ApiError> {
        self.ensure_buvid()?;
        for attempt in 0..2 {
            let k = self.wbi_keys()?;
            let query = crate::wbi::sign(
                &mut params.clone(),
                &k,
                nanatype_model::unix_millis() / 1000,
            );
            let result = self.get_json(&format!("{url}?{query}"))?.into_data();
            if attempt == 0 && matches!(result, Err(ApiError::Business { code: -352, .. })) {
                *self.keys.lock().unwrap_or_else(|p| p.into_inner()) = None;
                continue;
            }
            return result;
        }
        unreachable!()
    }
}
fn finish(
    result: Result<ureq::http::Response<ureq::Body>, ureq::Error>,
) -> Result<RawResponse, ApiError> {
    let mut response =
        result.map_err(|_| ApiError::Network("request failed or timed out".into()))?;
    let status = response.status().as_u16();
    if !(200..300).contains(&status) {
        return Err(ApiError::Http { status });
    }
    let set_cookies = response
        .headers()
        .get_all("set-cookie")
        .iter()
        .filter_map(|v| v.to_str().ok().map(str::to_owned))
        .collect();
    let body = response
        .body_mut()
        .with_config()
        .limit(4 * 1024 * 1024)
        .read_to_vec()
        .map_err(|_| ApiError::Network("response interrupted".into()))?;
    Ok(RawResponse { body, set_cookies })
}
