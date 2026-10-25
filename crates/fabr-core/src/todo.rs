#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Legacy {
    pub done: bool,
    pub due: Option<String>,
    pub pri: u8,
    pub title: String,
}

fn take_token(s: &str) -> Option<(&str, &str)> {
    let s = s.trim_start();
    if s.is_empty() {
        return None;
    }
    let end = s.find(char::is_whitespace).unwrap_or(s.len());
    Some((&s[..end], &s[end..]))
}

fn is_hex(s: &str) -> bool {
    !s.is_empty() && s.bytes().all(|c| matches!(c, b'0'..=b'9' | b'a'..=b'f'))
}

fn parse_line(line: &str) -> Option<Legacy> {
    if line.starts_with(char::is_whitespace) {
        return None;
    }
    let (status, rest) = take_token(line)?;
    if !matches!(status, "todo" | "wip" | "done") || !rest.starts_with(char::is_whitespace) {
        return None;
    }
    let (due, rest) = take_token(rest)?;
    let due = match due {
        "-" => None,
        d if crate::taskrec::valid_due(d) => Some(d.to_string()),
        _ => return None,
    };
    if !rest.starts_with(char::is_whitespace) {
        return None;
    }
    let mut title = rest.trim();
    let mut pri = 0;
    while let Some(ws) = title.rfind(char::is_whitespace) {
        let tok = title[ws..].trim_start();
        let head = title[..ws].trim_end();
        if head.is_empty() {
            break;
        }
        let is_id = tok.strip_prefix("id:").is_some_and(is_hex);
        let tag_pri = match tok.strip_prefix("pri:").map(str::as_bytes) {
            Some([c @ b'1'..=b'3']) => Some(c - b'0'),
            _ => None,
        };
        if let Some(p) = tag_pri {
            pri = p;
        } else if !is_id {
            break;
        }
        title = head;
    }
    (!title.is_empty()).then(|| Legacy { done: status == "done", due, pri, title: title.to_string() })
}

pub fn parse_legacy(text: &str) -> Vec<Legacy> {
    text.lines().filter_map(parse_line).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_lines_and_skips_the_rest() {
        let text = "# fabr todo\n\
wip  2026-10-19  Migrate auth  pri:2  id:3380\n\
\n\
junk line\n\
done -  Shipped\n\
todo 2026-10-20T09:30  Call https://x.example/a?b=1\n\
todo -  No deadline here\n";
        let v = parse_legacy(text);
        let titles: Vec<_> = v.iter().map(|l| l.title.as_str()).collect();
        assert_eq!(titles, ["Migrate auth", "Shipped", "Call https://x.example/a?b=1", "No deadline here"]);
        assert_eq!((v[0].pri, v[0].due.as_deref(), v[0].done), (2, Some("2026-10-19"), false));
        assert!(v[1].done && v[1].due.is_none());
        assert_eq!(v[2].due.as_deref(), Some("2026-10-20T09:30"));
        assert_eq!(v[3].pri, 0);
    }
}
