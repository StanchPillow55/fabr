use crate::sync::lock;
use std::sync::Mutex;

static SECRETS: Mutex<Vec<(String, String)>> = Mutex::new(Vec::new());

pub fn fingerprint(key: &str) -> String {
    let chars: Vec<char> = key.chars().collect();
    if chars.len() < 20 {
        return "…".into();
    }
    let head: String = chars[..7].iter().collect();
    let tail: String = chars[chars.len() - 4..].iter().collect();
    format!("{head}…{tail}")
}

pub fn register_secret(secret: &str, label: &str) {
    if secret.chars().count() < 8 {
        return;
    }
    let mut all = lock(&SECRETS);
    if !all.iter().any(|(s, _)| s == secret) {
        all.push((secret.to_string(), label.to_string()));
    }
}

fn token_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_' || c == '-'
}

fn mask_key_shapes(s: &str) -> String {
    let mut out = String::new();
    let mut rest = s;
    while let Some(i) = rest.find("sk-ant-") {
        out.push_str(&rest[..i]);
        let tail = &rest[i..];
        let end = tail.find(|c: char| !token_char(c)).unwrap_or(tail.len());
        let token = &tail[..end];
        out.push_str(&if token.chars().count() < 20 { token.to_string() } else { fingerprint(token) });
        rest = &tail[end..];
    }
    out.push_str(rest);
    out
}

fn mask_headers(s: &str) -> String {
    let lower = s.to_ascii_lowercase();
    let mut out = String::new();
    let mut at = 0;
    while at < s.len() {
        let hit = ["x-api-key", "authorization"].iter().filter_map(|h| lower[at..].find(h).map(|i| (at + i, h.len()))).min();
        let Some((i, n)) = hit else { break };
        let after = i + n;
        let sep = s[after..].find(|c: char| !matches!(c, ' ' | ':' | '=' | '"' | '\'')).map_or(s.len(), |k| after + k);
        out.push_str(&s[at..sep]);
        let stop = |c: char| c.is_whitespace() || matches!(c, '"' | '\'' | ',' | ';' | '}');
        let mut end = s[sep..].find(stop).map_or(s.len(), |k| sep + k);
        if matches!(lower[sep..end].trim(), "bearer" | "basic") {
            let start = s[end..].find(|c: char| !c.is_whitespace()).map_or(s.len(), |k| end + k);
            end = s[start..].find(stop).map_or(s.len(), |k| start + k);
        }
        if end > sep {
            out.push_str("[redacted]");
        }
        at = end.max(sep);
    }
    out.push_str(&s[at..]);
    out
}

pub fn redact(s: &str) -> String {
    let mut out = s.to_string();
    for (secret, label) in lock(&SECRETS).iter() {
        out = out.replace(secret.as_str(), label);
    }
    mask_headers(&mask_key_shapes(&out))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fingerprint_keeps_head_and_tail_only() {
        assert_eq!(fingerprint("sk-ant-api03-SECRETSECRETSECRET-x9Qa"), "sk-ant-…x9Qa");
        assert_eq!(fingerprint("short"), "…");
    }

    #[test]
    fn masks_registered_secrets_key_shapes_and_headers() {
        register_secret("bot123456:ABCDEFGHIJKLMNOP", "[bot token]");
        let out = redact(
            "call failed: bot123456:ABCDEFGHIJKLMNOP; key sk-ant-api03-ZZZZZZZZZZZZZZZZ-abcd; X-Api-Key: hunter2hunter2, Authorization: Bearer tok999tok",
        );
        assert!(!out.contains("ABCDEFGHIJKLMNOP") && !out.contains("ZZZZ") && !out.contains("hunter2"));
        assert!(out.contains("[bot token]") && out.contains("sk-ant-…abcd"));
        assert!(!out.contains("tok999tok") && out.contains("Authorization: [redacted]"));
    }
}
