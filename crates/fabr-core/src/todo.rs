use crate::error::FabrError;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Status {
    Todo,
    Wip,
    Done,
}

impl Status {
    pub fn as_str(&self) -> &'static str {
        match self {
            Status::Todo => "todo",
            Status::Wip => "wip",
            Status::Done => "done",
        }
    }
    pub fn parse(s: &str) -> Option<Status> {
        match s {
            "todo" => Some(Status::Todo),
            "wip" => Some(Status::Wip),
            "done" => Some(Status::Done),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Item {
    #[serde(default)]
    pub id: Option<String>,
    pub status: Status,
    #[serde(default)]
    pub due: Option<String>,
    pub title: String,
    #[serde(default)]
    pub pri: Option<u8>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Line {
    Item(Item),
    Raw { text: String, bad: bool, line: usize },
}

fn is_date_shape(s: &str) -> bool {
    let b = s.as_bytes();
    b.len() == 10 && b.iter().enumerate().all(|(i, c)| if i == 4 || i == 7 { *c == b'-' } else { c.is_ascii_digit() })
}

fn is_due_shape(s: &str) -> bool {
    match s.len() {
        10 => is_date_shape(s),
        16 => {
            let b = s.as_bytes();
            is_date_shape(&s[..10]) && b[10] == b'T' && b[13] == b':' && b[11..13].iter().chain(&b[14..16]).all(u8::is_ascii_digit)
        }
        _ => false,
    }
}

fn is_valid_due(s: &str) -> bool {
    is_due_shape(s)
        && chrono::NaiveDate::parse_from_str(&s[..10], "%Y-%m-%d").is_ok()
        && (s.len() == 10 || (s[11..13].parse::<u8>().is_ok_and(|h| h < 24) && s[14..16].parse::<u8>().is_ok_and(|m| m < 60)))
}

pub fn due_sort_key(due: Option<&str>) -> (u8, String, u8, String) {
    match due {
        None => (1, String::new(), 0, String::new()),
        Some(d) if d.len() > 10 => (0, d[..10].to_string(), 0, d[11..].to_string()),
        Some(d) => (0, d.to_string(), 1, String::new()),
    }
}

fn is_hex(s: &str) -> bool {
    !s.is_empty() && s.bytes().all(|c| matches!(c, b'0'..=b'9' | b'a'..=b'f'))
}

enum Tag {
    Id(String),
    Pri(u8),
}

fn parse_tag(tok: &str) -> Option<Tag> {
    if let Some(h) = tok.strip_prefix("id:") {
        if is_hex(h) {
            return Some(Tag::Id(h.to_string()));
        }
    } else if let Some(p) = tok.strip_prefix("pri:")
        && let [c @ b'1'..=b'3'] = p.as_bytes()
    {
        return Some(Tag::Pri(c - b'0'));
    }
    None
}

fn take_token(s: &str) -> Option<(&str, &str)> {
    let s = s.trim_start();
    if s.is_empty() {
        return None;
    }
    let end = s.find(char::is_whitespace).unwrap_or(s.len());
    Some((&s[..end], &s[end..]))
}

fn parse_item(line: &str) -> Option<Item> {
    let (status, rest) = take_token(line)?;
    if line.starts_with(char::is_whitespace) {
        return None;
    }
    let status = Status::parse(status)?;
    if !rest.starts_with(char::is_whitespace) {
        return None;
    }
    let (due, rest) = take_token(rest)?;
    let due = match due {
        "-" => None,
        d if is_due_shape(d) => Some(d.to_string()),
        _ => return None,
    };
    if !rest.starts_with(char::is_whitespace) {
        return None;
    }
    let mut title = rest.trim();
    if title.is_empty() {
        return None;
    }
    let (mut id, mut pri) = (None, None);
    while let Some(ws) = title.rfind(char::is_whitespace) {
        let tok = &title[ws..].trim_start();
        let head = title[..ws].trim_end();
        if head.is_empty() {
            break;
        }
        match parse_tag(tok) {
            Some(Tag::Id(h)) if id.is_none() => id = Some(h),
            Some(Tag::Pri(p)) if pri.is_none() => pri = Some(p),
            _ => break,
        }
        title = head;
    }
    Some(Item { id, status, due, title: title.to_string(), pri })
}

pub fn parse_todo(text: &str) -> Vec<Line> {
    text.lines()
        .enumerate()
        .map(|(n, line)| match parse_item(line) {
            Some(item) => Line::Item(item),
            None => {
                let t = line.trim_start();
                let bad = !t.is_empty() && !t.starts_with('#');
                Line::Raw { text: line.to_string(), bad, line: n + 1 }
            }
        })
        .collect()
}

pub fn format_item(item: &Item) -> String {
    let mut s = format!("{:<4} {:<10}  {}", item.status.as_str(), item.due.as_deref().unwrap_or("-"), item.title);
    if let Some(p) = item.pri {
        s.push_str(&format!("  pri:{p}"));
    }
    if let Some(id) = &item.id {
        s.push_str(&format!("  id:{id}"));
    }
    s
}

pub fn render_todo(old: &[Line], mut items: Vec<Item>) -> String {
    let mut used: HashSet<String> = items.iter().filter_map(|i| i.id.clone()).collect();
    for item in items.iter_mut() {
        if item.id.as_deref().is_none_or(str::is_empty) {
            let new = loop {
                let n: u16 = rand::random();
                let h = format!("{n:04x}");
                if !used.contains(&h) {
                    break h;
                }
            };
            used.insert(new.clone());
            item.id = Some(new);
        }
    }
    let mut queue = items.into_iter();
    let mut out: Vec<String> = Vec::new();
    for line in old {
        match line {
            Line::Raw { text, .. } => out.push(text.clone()),
            Line::Item(_) => {
                if let Some(i) = queue.next() {
                    out.push(format_item(&i));
                }
            }
        }
    }
    out.extend(queue.map(|i| format_item(&i)));
    out.join("\n") + "\n"
}

pub fn validate_items(items: Vec<Item>) -> Result<Vec<Item>, FabrError> {
    let bad = |i: usize, m: String| FabrError::BadRequest(format!("item {i}: {m}"));
    let mut clean = Vec::with_capacity(items.len());
    for (i, item) in items.into_iter().enumerate() {
        let due = item.due.filter(|d| !d.is_empty());
        if let Some(d) = &due
            && !is_valid_due(d)
        {
            return Err(bad(i, format!("bad deadline {d:?}")));
        }
        let title = item.title.trim().to_string();
        if title.is_empty() || title.contains('\n') || title.contains('\r') {
            return Err(bad(i, "title must be one non-empty line".into()));
        }
        if let Some(ws) = title.rfind(char::is_whitespace)
            && parse_tag(title[ws..].trim_start()).is_some()
        {
            return Err(bad(i, "title may not end with 'id:<hex>' or 'pri:<1-3>'".into()));
        }
        let id = item.id.filter(|s| !s.is_empty());
        if let Some(h) = &id
            && !(is_hex(h) && h.len() <= 16)
        {
            return Err(bad(i, format!("bad id {h:?}")));
        }
        if let Some(p) = item.pri
            && !(1..=3).contains(&p)
        {
            return Err(bad(i, format!("bad priority {p} (1-3)")));
        }
        clean.push(Item { id, status: item.status, due, title, pri: item.pri });
    }
    let mut seen = HashSet::new();
    for id in clean.iter().filter_map(|i| i.id.as_ref()) {
        if !seen.insert(id) {
            return Err(FabrError::BadRequest("duplicate item ids".into()));
        }
    }
    Ok(clean)
}

pub fn now_minute() -> String {
    chrono::Local::now().format("%Y-%m-%dT%H:%M").to_string()
}

pub fn is_overdue_at(due: &str, today: &str, now: &str) -> bool {
    if due.len() > 10 { due < now } else { due < today }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEXT: &str = "# header\n\
wip  2026-10-19  Migrate auth  id:3380\n\
\n\
this line is junk\n\
todo -           No deadline here\n\
# trailing comment\n";

    fn items(lines: &[Line]) -> Vec<Item> {
        lines.iter().filter_map(|l| if let Line::Item(i) = l { Some(i.clone()) } else { None }).collect()
    }

    fn it(status: Status, due: Option<&str>, title: &str, id: Option<&str>) -> Item {
        Item { id: id.map(Into::into), status, due: due.map(Into::into), title: title.into(), pri: None }
    }

    #[test]
    fn parse() {
        let lines = parse_todo(TEXT);
        let its = items(&lines);
        assert_eq!(its.iter().map(|i| i.title.as_str()).collect::<Vec<_>>(), ["Migrate auth", "No deadline here"]);
        assert_eq!(its[0].id.as_deref(), Some("3380"));
        assert_eq!(its[0].due.as_deref(), Some("2026-10-19"));
        assert_eq!(its[0].status, Status::Wip);
        assert!(its[1].id.is_none());
        assert!(its[1].due.is_none());
        let bad: Vec<_> = lines
            .iter()
            .filter_map(|l| match l {
                Line::Raw { text, bad: true, .. } => Some(text.as_str()),
                _ => None,
            })
            .collect();
        assert_eq!(bad, ["this line is junk"]);
        assert!(matches!(&lines[3], Line::Raw { line: 4, .. }));
    }

    #[test]
    fn round_trip_keeps_raw_lines_and_assigns_ids() {
        let lines = parse_todo(TEXT);
        let out = render_todo(&lines, items(&lines));
        for raw in ["# header", "this line is junk", "# trailing comment"] {
            assert!(out.contains(raw));
        }
        let again = items(&parse_todo(&out));
        assert_eq!(again.iter().map(|i| i.title.as_str()).collect::<Vec<_>>(), ["Migrate auth", "No deadline here"]);
        assert_eq!(again[0].id.as_deref(), Some("3380"));
        let id1 = again[1].id.as_deref().unwrap();
        assert!(id1.len() == 4 && is_hex(id1));
        assert_eq!(out.lines().next(), Some("# header"));
    }

    #[test]
    fn added_and_removed_items() {
        let lines = parse_todo(TEXT);
        let out = render_todo(&lines, vec![it(Status::Done, None, "only", Some("aa")), it(Status::Todo, Some("2026-11-01"), "new", None)]);
        let got: Vec<_> = items(&parse_todo(&out)).into_iter().map(|i| (i.status, i.title)).collect();
        assert_eq!(got, [(Status::Done, "only".to_string()), (Status::Todo, "new".to_string())]);
    }

    #[test]
    fn validation() {
        let ok = it(Status::Todo, None, "x", None);
        let cases = [
            Item { due: Some("2026-13-40".into()), ..ok.clone() },
            Item { due: Some("2026-1-4".into()), ..ok.clone() },
            Item { title: "".into(), ..ok.clone() },
            Item { title: "   ".into(), ..ok.clone() },
            Item { title: "a\nb".into(), ..ok.clone() },
            Item { title: "sneaky id:abc".into(), ..ok.clone() },
            Item { title: "sneaky pri:2".into(), ..ok.clone() },
            Item { id: Some("XYZ".into()), ..ok.clone() },
            Item { pri: Some(0), ..ok.clone() },
            Item { pri: Some(4), ..ok.clone() },
        ];
        for bad in cases {
            let e = validate_items(vec![bad.clone()]);
            assert!(matches!(e, Err(FabrError::BadRequest(_))), "{bad:?}");
        }
        let dup = Item { id: Some("aa".into()), ..ok.clone() };
        assert!(validate_items(vec![dup.clone(), dup]).is_err());
        let v = validate_items(vec![Item { due: Some("".into()), id: Some("".into()), title: " t ".into(), ..ok }]).unwrap();
        assert_eq!(v[0], it(Status::Todo, None, "t", None));
        assert!(serde_json::from_str::<Item>(r#"{"status":"nope","title":"x"}"#).is_err());
        let i: Item = serde_json::from_str(r#"{"status":"todo","title":"x","due":null}"#).unwrap();
        assert_eq!(i.pri, None);
    }

    #[test]
    fn url_titles_survive() {
        for title in ["Read https://x.y", "See http://a.b/c?d=e:f", "ratio 3:2", "key k:v", "done:yes"] {
            let item = Item { id: Some("ab12".into()), status: Status::Todo, due: None, title: title.into(), pri: None };
            let line = format_item(&item);
            assert_eq!(items(&parse_todo(&line)), std::slice::from_ref(&item), "{line}");
            let no_id = Item { id: None, ..item };
            assert_eq!(items(&parse_todo(&format_item(&no_id))), [no_id]);
        }
        let got = items(&parse_todo("todo -  Visit https://x.y"));
        assert_eq!(got[0].title, "Visit https://x.y");
        assert!(validate_items(vec![it(Status::Todo, None, "Visit https://x.y", None)]).is_ok());
    }

    #[test]
    fn unknown_tags_stay_in_title() {
        let got = items(&parse_todo("todo -  Fix it  owner:me  id:ab"));
        assert_eq!(got[0].title, "Fix it  owner:me");
        assert_eq!(got[0].id.as_deref(), Some("ab"));
        let got = items(&parse_todo("todo -  Fix it pri:7"));
        assert_eq!((got[0].title.as_str(), got[0].pri), ("Fix it pri:7", None));
        let got = items(&parse_todo("todo -  id:abc"));
        assert_eq!((got[0].title.as_str(), got[0].id.as_deref()), ("id:abc", None));
    }

    #[test]
    fn pri_round_trip() {
        let item =
            Item { id: Some("3380".into()), status: Status::Wip, due: Some("2026-10-19".into()), title: "Migrate".into(), pri: Some(1) };
        let line = format_item(&item);
        assert_eq!(line, "wip  2026-10-19  Migrate  pri:1  id:3380");
        assert_eq!(items(&parse_todo(&line)), [item]);
        let got = items(&parse_todo("todo -  X  id:ab  pri:3"));
        assert_eq!((got[0].title.as_str(), got[0].pri, got[0].id.as_deref()), ("X", Some(3), Some("ab")));
        let line = format_item(&Item { id: Some("ab".into()), status: Status::Todo, due: None, title: "X".into(), pri: None });
        assert_eq!(line, "todo -           X  id:ab");
    }

    #[test]
    fn due_time_round_trip() {
        let item = Item {
            id: Some("ab12".into()),
            status: Status::Todo,
            due: Some("2026-10-21T14:30".into()),
            title: "Call".into(),
            pri: Some(2),
        };
        let line = format_item(&item);
        assert_eq!(line, "todo 2026-10-21T14:30  Call  pri:2  id:ab12");
        assert_eq!(items(&parse_todo(&line)), std::slice::from_ref(&item));
        assert_eq!(validate_items(vec![item]).unwrap()[0].due.as_deref(), Some("2026-10-21T14:30"));
        let d = Item { due: Some("2026-10-21".into()), id: None, pri: None, ..it(Status::Wip, None, "x", None) };
        assert_eq!(format_item(&d), "wip  2026-10-21  x");
    }

    #[test]
    fn due_time_validation() {
        let ok = it(Status::Todo, None, "x", None);
        for d in [
            "2026-10-21T24:00",
            "2026-10-21T12:60",
            "2026-02-30T10:00",
            "2026-10-21T1:30",
            "2026-10-21 10:00",
            "2026-10-21T10:00:00",
            "2026-10-21t10:00",
            "2026-10-21T",
        ] {
            let e = validate_items(vec![Item { due: Some(d.into()), ..ok.clone() }]);
            assert!(matches!(e, Err(FabrError::BadRequest(_))), "{d}");
        }
        for d in ["2026-10-21T00:00", "2026-10-21T23:59", "2026-10-21"] {
            assert!(validate_items(vec![Item { due: Some(d.into()), ..ok.clone() }]).is_ok(), "{d}");
        }
        assert!(matches!(&parse_todo("todo 2026-10-21T14  x")[0], Line::Raw { bad: true, .. }));
    }

    #[test]
    fn due_sorting() {
        let mut v = vec![None, Some("2026-10-21"), Some("2026-10-21T14:30"), Some("2026-10-20"), Some("2026-10-21T09:00")];
        v.sort_by_key(|d| due_sort_key(*d));
        assert_eq!(v, [Some("2026-10-20"), Some("2026-10-21T09:00"), Some("2026-10-21T14:30"), Some("2026-10-21"), None]);
    }

    #[test]
    fn unparsable_lines() {
        for l in ["todo", "todo 2026-10-19", "todo tomorrow  x", "todox - x", " todo - x", "maybe - x"] {
            assert!(matches!(&parse_todo(l)[0], Line::Raw { bad: true, .. }), "{l}");
        }
    }

    #[test]
    fn random_ids_unique() {
        let many: Vec<Item> = (0..2000).map(|n| it(Status::Todo, None, &format!("t{n}"), None)).collect();
        let out = render_todo(&[], many);
        let ids: HashSet<_> = items(&parse_todo(&out)).into_iter().map(|i| i.id.unwrap()).collect();
        assert_eq!(ids.len(), 2000);
    }
}
