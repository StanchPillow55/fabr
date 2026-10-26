//! Google Workspace (Gmail + Calendar) over Google's REST APIs.
//!
//! Auth is PLUG-AND-PLAY. There is no interactive OAuth dance here. A user-
//! provided OAuth access token is read from the `GOOGLE_OAUTH_TOKEN` env var, or
//! from a JSON file at `GOOGLE_TOKEN_PATH` (default `~/.fabric-space/google_token.json`)
//! shaped `{access_token, refresh_token?, client_id?, client_secret?, expiry?}`.
//! When the file carries a refresh_token plus client_id and client_secret and the
//! cached access token is expired, a single refresh POST to Google's token
//! endpoint mints a fresh access token which is written back to the file. When no
//! token can be resolved, every function returns a clear `FabrError` telling the
//! user what to supply; it never panics.
//!
//! Reads (gmail_list, gmail_get, calendar_list) are context: they do not egress
//! new data and carry no second gate. gmail_send is an irreversible external
//! effect: its ToolDescriptor effect is `Effect::ExternalEffect`, so M3's
//! final-confirm gate fires before it is dispatched. The token is never logged.

use crate::now_utc;
use fabr_core::FabrError;
use fabr_core::redact;
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use std::time::Duration;

/// Default JSON token location, relative to `$HOME`.
pub const DEFAULT_TOKEN_SUBPATH: &str = ".fabric-space/google_token.json";
const GMAIL_BASE: &str = "https://gmail.googleapis.com/gmail/v1/users/me";
const CALENDAR_BASE: &str = "https://www.googleapis.com/calendar/v3/calendars/primary";
const TOKEN_ENDPOINT: &str = "https://oauth2.googleapis.com/token";
/// OAuth scopes the supplied token must carry for the four tools to work.
pub const REQUIRED_SCOPES: &[&str] = &[
    "https://www.googleapis.com/auth/gmail.readonly",
    "https://www.googleapis.com/auth/gmail.send",
    "https://www.googleapis.com/auth/calendar.readonly",
];

fn missing_token_error(tool: &str) -> FabrError {
    FabrError::BadRequest(format!(
        "tool {tool} is unavailable: no Google OAuth token. Set GOOGLE_OAUTH_TOKEN, or write \
         {{\"access_token\": \"...\"}} (optionally refresh_token/client_id/client_secret/expiry) to \
         GOOGLE_TOKEN_PATH (default ~/{DEFAULT_TOKEN_SUBPATH}). The token needs scopes \
         gmail.readonly, gmail.send, and calendar.readonly."
    ))
}

// --- token file -----------------------------------------------------------

/// A parsed Google token file. Only `access_token` is required; the rest enable
/// an at-most-once refresh when the cached access token has expired.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TokenFile {
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub client_id: Option<String>,
    pub client_secret: Option<String>,
    /// RFC-3339 instant after which the access token is treated as expired.
    pub expiry: Option<String>,
}

impl TokenFile {
    /// Parse the JSON token document. A missing `access_token` is an error so a
    /// malformed file never silently yields an empty bearer. Never panics.
    pub fn parse(text: &str) -> Result<Self, FabrError> {
        let value: Value =
            serde_json::from_str(text.trim()).map_err(|_| FabrError::BadRequest("google token file is not valid json".into()))?;
        let access_token = value.get("access_token").and_then(|v| v.as_str()).map(str::trim).unwrap_or("").to_string();
        if access_token.is_empty() {
            return Err(FabrError::BadRequest("google token file has no access_token".into()));
        }
        let field = |key: &str| value.get(key).and_then(|v| v.as_str()).map(str::trim).filter(|s| !s.is_empty()).map(str::to_string);
        Ok(TokenFile {
            access_token,
            refresh_token: field("refresh_token"),
            client_id: field("client_id"),
            client_secret: field("client_secret"),
            expiry: field("expiry"),
        })
    }

    /// Serialize back to JSON so a refreshed access token can be cached. Only the
    /// known fields are written; unknown fields in the original file are dropped.
    pub fn to_json(&self) -> String {
        let mut map = serde_json::Map::new();
        map.insert("access_token".into(), Value::String(self.access_token.clone()));
        if let Some(v) = &self.refresh_token {
            map.insert("refresh_token".into(), Value::String(v.clone()));
        }
        if let Some(v) = &self.client_id {
            map.insert("client_id".into(), Value::String(v.clone()));
        }
        if let Some(v) = &self.client_secret {
            map.insert("client_secret".into(), Value::String(v.clone()));
        }
        if let Some(v) = &self.expiry {
            map.insert("expiry".into(), Value::String(v.clone()));
        }
        serde_json::to_string_pretty(&Value::Object(map)).unwrap_or_else(|_| "{}".into())
    }

    /// Whether the cached access token is past its `expiry` relative to `now`
    /// (both RFC-3339). An absent or unparseable expiry is treated as not expired
    /// so a plain `{access_token}` file keeps working.
    pub fn is_expired(&self, now_rfc3339: &str) -> bool {
        let Some(expiry) = &self.expiry else { return false };
        match (parse_rfc3339(expiry), parse_rfc3339(now_rfc3339)) {
            (Some(exp), Some(now)) => now >= exp,
            _ => false,
        }
    }

    /// Whether a one-shot refresh is possible (refresh_token + client credentials).
    pub fn can_refresh(&self) -> bool {
        self.refresh_token.is_some() && self.client_id.is_some() && self.client_secret.is_some()
    }
}

/// The token file path from `GOOGLE_TOKEN_PATH`, else `~/.fabric-space/google_token.json`.
pub fn token_path() -> Option<PathBuf> {
    if let Ok(raw) = std::env::var("GOOGLE_TOKEN_PATH") {
        let raw = raw.trim();
        if !raw.is_empty() {
            return Some(PathBuf::from(raw));
        }
    }
    let home = std::env::var("HOME").ok().filter(|h| !h.trim().is_empty())?;
    Some(Path::new(&home).join(DEFAULT_TOKEN_SUBPATH))
}

/// Resolve a usable bearer access token. Order: `GOOGLE_OAUTH_TOKEN` env wins;
/// otherwise read the JSON file, refreshing once (and caching) if it is expired
/// and carries refresh credentials. Returns `None` when nothing is available.
pub fn access_token() -> Option<String> {
    if let Ok(value) = std::env::var("GOOGLE_OAUTH_TOKEN") {
        let value = value.trim().to_string();
        if !value.is_empty() {
            redact::register_secret(&value, "[redacted]");
            return Some(value);
        }
    }
    let path = token_path()?;
    let text = std::fs::read_to_string(&path).ok()?;
    let mut file = TokenFile::parse(&text).ok()?;
    redact::register_secret(&file.access_token, "[redacted]");
    if let Some(rt) = &file.refresh_token {
        redact::register_secret(rt, "[redacted]");
    }
    if file.is_expired(&now_rfc3339()) && file.can_refresh() && refresh_in_place(&mut file).is_ok() {
        // Cache the fresh access token back to the file, best-effort.
        let _ = std::fs::write(&path, file.to_json());
    }
    Some(file.access_token)
}

/// One refresh POST to Google's token endpoint, updating `file.access_token`
/// (and `expiry` when returned). Network failures leave `file` unchanged.
fn refresh_in_place(file: &mut TokenFile) -> Result<(), FabrError> {
    let (Some(refresh_token), Some(client_id), Some(client_secret)) =
        (file.refresh_token.as_ref(), file.client_id.as_ref(), file.client_secret.as_ref())
    else {
        return Err(FabrError::BadRequest("cannot refresh: refresh_token, client_id, and client_secret are required".into()));
    };
    let body = form_urlencoded(&[
        ("grant_type", "refresh_token"),
        ("refresh_token", refresh_token),
        ("client_id", client_id),
        ("client_secret", client_secret),
    ]);
    let agent = ureq::AgentBuilder::new().timeout(Duration::from_secs(30)).build();
    let sent = agent.post(TOKEN_ENDPOINT).set("content-type", "application/x-www-form-urlencoded").send_string(&body);
    let text = match sent {
        Ok(resp) => resp.into_string().unwrap_or_default(),
        Err(ureq::Error::Status(_, resp)) => resp.into_string().unwrap_or_default(),
        Err(_) => return Err(FabrError::Remote("token refresh failed".into())),
    };
    let value: Value = serde_json::from_str(&text).map_err(|_| FabrError::Remote("token refresh returned non-json".into()))?;
    let new_token = value.get("access_token").and_then(|v| v.as_str()).map(str::trim).filter(|s| !s.is_empty());
    let Some(new_token) = new_token else {
        return Err(FabrError::Remote("token refresh returned no access_token".into()));
    };
    redact::register_secret(new_token, "[redacted]");
    file.access_token = new_token.to_string();
    if let Some(seconds) = value.get("expires_in").and_then(|v| v.as_i64()) {
        file.expiry = Some(rfc3339_in(seconds));
    }
    Ok(())
}

// --- read tools ------------------------------------------------------------

/// Gmail message list: search with `query`, then fetch metadata for up to
/// `limit` hits. Returns markdown (from, subject, date, snippet, id).
pub fn gmail_list(query: &str, limit: usize) -> Result<String, FabrError> {
    let token = access_token().ok_or_else(|| missing_token_error("gmail_list"))?;
    let limit = clamp_limit(limit, 20);
    let qs = query_string(&[("q", query.trim()), ("maxResults", &limit.to_string())]);
    let list = get_json(&format!("{GMAIL_BASE}/messages{qs}"), &token)?;
    let ids: Vec<String> = list
        .get("messages")
        .and_then(|v| v.as_array())
        .map(|items| items.iter().filter_map(|m| m.get("id").and_then(|v| v.as_str()).map(str::to_string)).collect())
        .unwrap_or_default();
    let mut out = format!("source: gsuite\ntool: gmail_list\nquery: {}\ncount: {}\n", query.trim(), ids.len().min(limit));
    if ids.is_empty() {
        out.push_str("messages: 0\n");
        return Ok(out);
    }
    for id in ids.iter().take(limit) {
        let meta_qs = query_string(&[
            ("format", "metadata"),
            ("metadataHeaders", "From"),
            ("metadataHeaders", "Subject"),
            ("metadataHeaders", "Date"),
        ]);
        let msg = match get_json(&format!("{GMAIL_BASE}/messages/{id}{meta_qs}"), &token) {
            Ok(msg) => msg,
            Err(_) => continue,
        };
        let from = header(&msg, "From");
        let subject = header(&msg, "Subject");
        let date = header(&msg, "Date");
        let snippet = msg.get("snippet").and_then(|v| v.as_str()).unwrap_or("").trim().replace(['\n', '\r'], " ");
        out.push_str(&format!(
            "\n- id: {id}\n  from: {from}\n  subject: {subject}\n  date: {date}\n  snippet: {}\n",
            snippet.chars().take(240).collect::<String>()
        ));
    }
    Ok(out)
}

/// One Gmail message, decoded to plain text.
pub fn gmail_get(id: &str) -> Result<String, FabrError> {
    let id = id.trim();
    if id.is_empty() {
        return Err(FabrError::BadRequest("gmail_get needs an id".into()));
    }
    let token = access_token().ok_or_else(|| missing_token_error("gmail_get"))?;
    let qs = query_string(&[("format", "full")]);
    let msg = get_json(&format!("{GMAIL_BASE}/messages/{id}{qs}"), &token)?;
    let from = header(&msg, "From");
    let subject = header(&msg, "Subject");
    let date = header(&msg, "Date");
    let body = decode_plain_body(&msg);
    Ok(format!("source: gsuite\ntool: gmail_get\nid: {id}\nfrom: {from}\nsubject: {subject}\ndate: {date}\n\n{body}\n"))
}

/// Primary-calendar events between `time_min` and `time_max` (RFC-3339).
/// Returns markdown (summary, start, end, attendees).
pub fn calendar_list(time_min: &str, time_max: &str, limit: usize) -> Result<String, FabrError> {
    let token = access_token().ok_or_else(|| missing_token_error("calendar_list"))?;
    let limit = clamp_limit(limit, 20);
    let limit_s = limit.to_string();
    let mut params: Vec<(&str, &str)> =
        vec![("singleEvents", "true"), ("orderBy", "startTime"), ("maxResults", &limit_s)];
    let tmin = time_min.trim();
    let tmax = time_max.trim();
    if !tmin.is_empty() {
        params.push(("timeMin", tmin));
    }
    if !tmax.is_empty() {
        params.push(("timeMax", tmax));
    }
    let qs = query_string(&params);
    let events = get_json(&format!("{CALENDAR_BASE}/events{qs}"), &token)?;
    let items = events.get("items").and_then(|v| v.as_array()).cloned().unwrap_or_default();
    let mut out = format!("source: gsuite\ntool: calendar_list\ncount: {}\n", items.len());
    if items.is_empty() {
        out.push_str("events: 0\n");
        return Ok(out);
    }
    for event in &items {
        let summary = event.get("summary").and_then(|v| v.as_str()).unwrap_or("(no title)");
        let start = event_time(event, "start");
        let end = event_time(event, "end");
        let attendees: Vec<String> = event
            .get("attendees")
            .and_then(|v| v.as_array())
            .map(|a| a.iter().filter_map(|x| x.get("email").and_then(|v| v.as_str()).map(str::to_string)).collect())
            .unwrap_or_default();
        out.push_str(&format!(
            "\n- summary: {summary}\n  start: {start}\n  end: {end}\n  attendees: {}\n",
            if attendees.is_empty() { "none".to_string() } else { attendees.join(", ") }
        ));
    }
    Ok(out)
}

// --- gated external effect -------------------------------------------------

/// Send a Gmail message. IRREVERSIBLE external effect: the lifecycle's M3
/// final-confirm gate must have fired before this runs (see tools_contract
/// `Effect::ExternalEffect`). Builds an RFC-2822 message, base64url-encodes it,
/// and POSTs to the Gmail send endpoint.
pub fn gmail_send(to: &str, subject: &str, body: &str) -> Result<String, FabrError> {
    let to = to.trim();
    if to.is_empty() {
        return Err(FabrError::BadRequest("gmail_send needs a recipient (to)".into()));
    }
    let token = access_token().ok_or_else(|| missing_token_error("gmail_send"))?;
    let raw = encode_send_payload(to, subject, body);
    let agent = ureq::AgentBuilder::new().timeout(Duration::from_secs(30)).build();
    let sent = agent
        .post(&format!("{GMAIL_BASE}/messages/send"))
        .set("authorization", &format!("Bearer {token}"))
        .set("content-type", "application/json")
        .send_json(json!({ "raw": raw }));
    let (status, text) = match sent {
        Ok(resp) => (resp.status(), resp.into_string().unwrap_or_default()),
        Err(ureq::Error::Status(status, resp)) => (status, resp.into_string().unwrap_or_default()),
        Err(err) => {
            let text = scrub(&err.to_string(), &token);
            if text.to_lowercase().contains("timeout") || text.to_lowercase().contains("timed out") {
                return Err(FabrError::Timeout("timeout".into()));
            }
            return Err(FabrError::Remote("unavailable".into()));
        }
    };
    let text = scrub(&text, &token);
    if status == 401 || status == 403 {
        return Err(FabrError::BadRequest("auth failed: token missing gmail.send scope or expired".into()));
    }
    if status == 408 {
        return Err(FabrError::Timeout("timeout".into()));
    }
    if !(200..300).contains(&status) {
        return Err(FabrError::Remote(format!("http {status}")));
    }
    let value: Value = serde_json::from_str(&text).unwrap_or(Value::Null);
    let msg_id = value.get("id").and_then(|v| v.as_str()).unwrap_or("sent");
    Ok(format!("gmail_send to={to} id={msg_id} at={}", now_utc()))
}

/// The base64url-encoded RFC-2822 body a send POST carries in `raw`.
pub fn encode_send_payload(to: &str, subject: &str, body: &str) -> String {
    base64url_encode(rfc2822_message(to, subject, body).as_bytes())
}

/// A minimal RFC-2822 message. Header values are stripped of CR/LF so they
/// cannot inject extra headers.
pub fn rfc2822_message(to: &str, subject: &str, body: &str) -> String {
    let to = sanitize_header(to);
    let subject = sanitize_header(subject);
    let body = body.replace('\r', "");
    format!("To: {to}\r\nSubject: {subject}\r\nContent-Type: text/plain; charset=\"UTF-8\"\r\nMIME-Version: 1.0\r\n\r\n{body}")
}

fn sanitize_header(value: &str) -> String {
    value.replace(['\r', '\n'], " ").trim().to_string()
}

// --- http + json helpers ---------------------------------------------------

fn get_json(url: &str, token: &str) -> Result<Value, FabrError> {
    let agent = ureq::AgentBuilder::new().timeout(Duration::from_secs(30)).build();
    let sent = agent.get(url).set("authorization", &format!("Bearer {token}")).call();
    let (status, text) = match sent {
        Ok(resp) => (resp.status(), resp.into_string().unwrap_or_default()),
        Err(ureq::Error::Status(status, resp)) => (status, resp.into_string().unwrap_or_default()),
        Err(err) => {
            let text = scrub(&err.to_string(), token);
            if text.to_lowercase().contains("timeout") || text.to_lowercase().contains("timed out") {
                return Err(FabrError::Timeout("timeout".into()));
            }
            return Err(FabrError::Remote("unavailable".into()));
        }
    };
    let text = scrub(&text, token);
    if status == 401 || status == 403 {
        return Err(FabrError::BadRequest("auth failed: Google token expired or missing the required scope".into()));
    }
    if status == 408 {
        return Err(FabrError::Timeout("timeout".into()));
    }
    if !(200..300).contains(&status) {
        return Err(FabrError::Remote(format!("http {status}")));
    }
    serde_json::from_str(&text).map_err(|_| FabrError::Remote("google returned non-json".into()))
}

/// A header value from a Gmail message `payload.headers` list.
fn header(msg: &Value, name: &str) -> String {
    msg.get("payload")
        .and_then(|p| p.get("headers"))
        .and_then(|h| h.as_array())
        .and_then(|headers| {
            headers
                .iter()
                .find(|h| h.get("name").and_then(|v| v.as_str()).map(|n| n.eq_ignore_ascii_case(name)).unwrap_or(false))
                .and_then(|h| h.get("value").and_then(|v| v.as_str()))
        })
        .unwrap_or("")
        .trim()
        .to_string()
}

/// Walk a Gmail payload for the first `text/plain` part and base64url-decode it.
/// Falls back to the message snippet when no decodable text part is present.
fn decode_plain_body(msg: &Value) -> String {
    let payload = msg.get("payload").cloned().unwrap_or(Value::Null);
    if let Some(text) = plain_part(&payload) {
        return text;
    }
    msg.get("snippet").and_then(|v| v.as_str()).unwrap_or("").trim().to_string()
}

fn plain_part(part: &Value) -> Option<String> {
    let mime = part.get("mimeType").and_then(|v| v.as_str()).unwrap_or("");
    if mime == "text/plain"
        && let Some(data) = part.get("body").and_then(|b| b.get("data")).and_then(|v| v.as_str())
        && let Some(bytes) = base64url_decode(data)
    {
        return Some(String::from_utf8_lossy(&bytes).trim().to_string());
    }
    if let Some(parts) = part.get("parts").and_then(|v| v.as_array()) {
        for child in parts {
            if let Some(text) = plain_part(child) {
                return Some(text);
            }
        }
    }
    None
}

fn event_time(event: &Value, key: &str) -> String {
    event
        .get(key)
        .and_then(|v| v.get("dateTime").or_else(|| v.get("date")).and_then(|x| x.as_str()))
        .unwrap_or("")
        .to_string()
}

fn clamp_limit(limit: usize, default: usize) -> usize {
    if limit == 0 { default } else { limit.min(100) }
}

/// Build a `?a=1&b=2` query string, percent-encoding each value. Empty values
/// are dropped. Repeated keys are allowed (Gmail metadataHeaders, etc.).
pub fn query_string(params: &[(&str, &str)]) -> String {
    let parts: Vec<String> =
        params.iter().filter(|(_, value)| !value.is_empty()).map(|(key, value)| format!("{key}={}", percent_encode(value))).collect();
    if parts.is_empty() { String::new() } else { format!("?{}", parts.join("&")) }
}

fn form_urlencoded(params: &[(&str, &str)]) -> String {
    params.iter().map(|(key, value)| format!("{key}={}", percent_encode(value))).collect::<Vec<_>>().join("&")
}

fn percent_encode(value: &str) -> String {
    let mut out = String::new();
    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'~') {
            out.push(byte as char);
        } else {
            out.push_str(&format!("%{byte:02X}"));
        }
    }
    out
}

fn scrub(text: &str, token: &str) -> String {
    let out = if token.len() >= 6 { text.replace(token, "[redacted]") } else { text.to_string() };
    redact::redact(&out)
}

// --- base64url (no padding), self-contained --------------------------------

const B64URL: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";

/// Base64url without padding, as Gmail's `raw` field requires.
pub fn base64url_encode(input: &[u8]) -> String {
    let mut out = String::with_capacity(input.len().div_ceil(3) * 4);
    for chunk in input.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = *chunk.get(1).unwrap_or(&0) as u32;
        let b2 = *chunk.get(2).unwrap_or(&0) as u32;
        let n = (b0 << 16) | (b1 << 8) | b2;
        out.push(B64URL[((n >> 18) & 0x3f) as usize] as char);
        out.push(B64URL[((n >> 12) & 0x3f) as usize] as char);
        if chunk.len() > 1 {
            out.push(B64URL[((n >> 6) & 0x3f) as usize] as char);
        }
        if chunk.len() > 2 {
            out.push(B64URL[(n & 0x3f) as usize] as char);
        }
    }
    out
}

/// Decode base64url (padded or not; tolerates standard +/ too) to bytes.
/// Returns `None` on an invalid character so decoding never panics.
pub fn base64url_decode(input: &str) -> Option<Vec<u8>> {
    let mut acc: u32 = 0;
    let mut bits = 0u32;
    let mut out = Vec::with_capacity(input.len() / 4 * 3);
    for ch in input.bytes() {
        let value = match ch {
            b'A'..=b'Z' => ch - b'A',
            b'a'..=b'z' => ch - b'a' + 26,
            b'0'..=b'9' => ch - b'0' + 52,
            b'-' | b'+' => 62,
            b'_' | b'/' => 63,
            b'=' => break,
            b'\r' | b'\n' => continue,
            _ => return None,
        };
        acc = (acc << 6) | value as u32;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8);
        }
    }
    Some(out)
}

// --- time helpers ----------------------------------------------------------

fn now_rfc3339() -> String {
    format!("{}Z", chrono::Utc::now().format("%Y-%m-%dT%H:%M:%S"))
}

fn rfc3339_in(seconds: i64) -> String {
    let when = chrono::Utc::now() + chrono::Duration::seconds(seconds);
    format!("{}Z", when.format("%Y-%m-%dT%H:%M:%S"))
}

/// Parse an RFC-3339 instant to epoch seconds for expiry comparison. Returns
/// `None` when the value cannot be parsed.
fn parse_rfc3339(value: &str) -> Option<i64> {
    chrono::DateTime::parse_from_rfc3339(value).ok().map(|dt| dt.timestamp())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64url_round_trips_without_padding() {
        // "Ma" -> standard base64 "TWE=", base64url drops the pad.
        assert_eq!(base64url_encode(b"Ma"), "TWE");
        assert_eq!(base64url_encode(b"M"), "TQ");
        assert_eq!(base64url_encode(b"Man"), "TWFu");
        assert_eq!(base64url_decode("TWFu").unwrap(), b"Man");
        assert_eq!(base64url_decode("TWE").unwrap(), b"Ma");
        // Round trip a message with bytes that force the url-safe alphabet.
        let bytes: Vec<u8> = (0u8..=255).collect();
        assert_eq!(base64url_decode(&base64url_encode(&bytes)).unwrap(), bytes);
        let encoded = base64url_encode(&bytes);
        assert!(!encoded.contains('+') && !encoded.contains('/') && !encoded.contains('='));
        assert!(base64url_decode("not valid !").is_none());
    }

    #[test]
    fn rfc2822_message_is_well_formed_and_header_safe() {
        let msg = rfc2822_message("mentor@example.test", "Weekly update", "Line one\nLine two");
        assert!(msg.contains("To: mentor@example.test\r\n"));
        assert!(msg.contains("Subject: Weekly update\r\n"));
        assert!(msg.contains("Content-Type: text/plain"));
        // Blank line separates headers from the body.
        assert!(msg.contains("\r\n\r\nLine one"));
        // A header-injection attempt is flattened: the CR/LF is gone, so the
        // "Bcc:"/"X-Evil:" text cannot start its own header line.
        let injected = rfc2822_message("a@b.test\r\nBcc: evil@x.test", "hi\nX-Evil: 1", "body");
        assert!(!injected.contains("\r\nBcc:"));
        assert!(!injected.contains("\r\nX-Evil:"));
        assert!(!injected.contains("\nBcc:"));
        // Four real headers (To, Subject, Content-Type, MIME-Version) plus the
        // blank-line separator: five CRLFs, none introduced by the injection.
        assert_eq!(injected.matches("\r\n").count(), 5);
    }

    #[test]
    fn send_payload_is_decodable_back_to_the_message() {
        let raw = encode_send_payload("to@example.test", "Subject here", "Hello there");
        assert!(!raw.contains('+') && !raw.contains('/') && !raw.contains('='));
        let decoded = String::from_utf8(base64url_decode(&raw).unwrap()).unwrap();
        assert!(decoded.starts_with("To: to@example.test\r\n"));
        assert!(decoded.ends_with("\r\n\r\nHello there"));
    }

    #[test]
    fn token_file_parses_and_round_trips() {
        let text = r#"{
            "access_token": "ya29.abc",
            "refresh_token": "1//rt",
            "client_id": "cid.apps.googleusercontent.com",
            "client_secret": "secret",
            "expiry": "2026-10-26T12:00:00Z"
        }"#;
        let file = TokenFile::parse(text).unwrap();
        assert_eq!(file.access_token, "ya29.abc");
        assert!(file.can_refresh());
        assert_eq!(file.refresh_token.as_deref(), Some("1//rt"));
        // A reserialize keeps the known fields.
        let again = TokenFile::parse(&file.to_json()).unwrap();
        assert_eq!(again, file);
    }

    #[test]
    fn token_file_rejects_missing_access_token_and_bad_json() {
        assert!(TokenFile::parse("{}").is_err());
        assert!(TokenFile::parse(r#"{"refresh_token": "x"}"#).is_err());
        assert!(TokenFile::parse("not json").is_err());
        // A bare access token with no refresh credentials still parses and is usable.
        let minimal = TokenFile::parse(r#"{"access_token": "tok"}"#).unwrap();
        assert_eq!(minimal.access_token, "tok");
        assert!(!minimal.can_refresh());
        assert!(!minimal.is_expired("2099-01-01T00:00:00Z"), "no expiry means never expired");
    }

    #[test]
    fn expiry_comparison_uses_the_instant() {
        let file = TokenFile {
            access_token: "tok".into(),
            refresh_token: Some("rt".into()),
            client_id: Some("cid".into()),
            client_secret: Some("sec".into()),
            expiry: Some("2026-10-26T12:00:00Z".into()),
        };
        assert!(file.is_expired("2026-10-26T12:00:01Z"));
        assert!(file.is_expired("2026-10-26T12:00:00Z"));
        assert!(!file.is_expired("2026-10-26T11:59:59Z"));
        // An unparseable now does not falsely expire the token.
        assert!(!file.is_expired("garbage"));
    }

    #[test]
    fn query_string_encodes_values_and_drops_empties() {
        let qs = query_string(&[("q", "from:mentor is:unread"), ("maxResults", "10"), ("timeMin", "")]);
        assert_eq!(qs, "?q=from%3Amentor%20is%3Aunread&maxResults=10");
        assert_eq!(query_string(&[("q", "")]), "");
        // Repeated keys (Gmail metadataHeaders) are preserved in order.
        let repeated = query_string(&[("metadataHeaders", "From"), ("metadataHeaders", "Subject")]);
        assert_eq!(repeated, "?metadataHeaders=From&metadataHeaders=Subject");
    }

    #[test]
    fn header_lookup_is_case_insensitive() {
        let msg = json!({
            "snippet": "hi there",
            "payload": {
                "headers": [
                    {"name": "From", "value": "a@b.test"},
                    {"name": "subject", "value": "Hello"}
                ],
                "mimeType": "text/plain",
                "body": {"data": base64url_encode(b"the body text")}
            }
        });
        assert_eq!(header(&msg, "From"), "a@b.test");
        assert_eq!(header(&msg, "Subject"), "Hello");
        assert_eq!(header(&msg, "Missing"), "");
        assert_eq!(decode_plain_body(&msg), "the body text");
    }

    #[test]
    fn plain_body_walks_multipart_then_falls_back_to_snippet() {
        let multipart = json!({
            "snippet": "fallback snippet",
            "payload": {
                "mimeType": "multipart/alternative",
                "parts": [
                    {"mimeType": "text/html", "body": {"data": base64url_encode(b"<p>ignored</p>")}},
                    {"mimeType": "text/plain", "body": {"data": base64url_encode(b"plain wins")}}
                ]
            }
        });
        assert_eq!(decode_plain_body(&multipart), "plain wins");
        let no_text = json!({"snippet": "only snippet", "payload": {"mimeType": "text/html"}});
        assert_eq!(decode_plain_body(&no_text), "only snippet");
    }

    #[test]
    fn missing_token_yields_a_clear_error_not_a_panic() {
        // Ensure neither source is set for this check.
        unsafe {
            std::env::remove_var("GOOGLE_OAUTH_TOKEN");
            std::env::set_var("GOOGLE_TOKEN_PATH", "/nonexistent/path/google_token.json");
        }
        let err = gmail_list("test", 5).unwrap_err();
        assert!(err.message().contains("unavailable"));
        assert!(err.message().contains("GOOGLE_OAUTH_TOKEN"));
        let send_err = gmail_send("a@b.test", "s", "b").unwrap_err();
        assert!(send_err.message().contains("unavailable"));
        unsafe {
            std::env::remove_var("GOOGLE_TOKEN_PATH");
        }
    }
}
