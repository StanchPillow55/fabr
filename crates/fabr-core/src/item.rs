use crate::fs::Fs;
use crate::node::join;
use crate::taskrec::State;
use serde::{Deserialize, Serialize};

pub const SPACE_DIR: &str = ".fabric-space";
pub const TASKS_DIR: &str = "tasks";

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

    pub fn of(state: State) -> Status {
        match state {
            State::Reminder => Status::Todo,
            State::Task | State::Defined | State::InProgress | State::Failed => Status::Wip,
            State::Succeeded => Status::Done,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Item {
    pub id: String,
    pub title: String,
    pub status: Status,
    #[serde(default)]
    pub due: Option<String>,
    #[serde(default)]
    pub pri: u8,
    #[serde(default)]
    pub suspended: bool,
}

#[derive(Deserialize)]
struct Front {
    id: String,
    title: String,
    #[serde(default)]
    due: Option<String>,
    #[serde(default)]
    pri: u8,
    state: State,
    #[serde(default)]
    suspended: bool,
    #[serde(default)]
    suspend_until: Option<String>,
}

pub fn item_from_record(text: &str) -> Option<Item> {
    let rest = text.strip_prefix("---")?;
    let yaml = &rest[..rest.find("\n---")?];
    let f: Front = serde_yaml::from_str(yaml).ok()?;
    let suspended = f.suspended && f.suspend_until.as_deref().is_none_or(|u| u > now_minute().as_str());
    Some(Item { id: f.id, title: f.title, status: Status::of(f.state), due: f.due, pri: f.pri.min(3), suspended })
}

pub fn sort_items(items: &mut [Item]) {
    items.sort_by(|a, b| {
        a.suspended
            .cmp(&b.suspended)
            .then_with(|| due_sort_key(a.due.as_deref()).cmp(&due_sort_key(b.due.as_deref())))
            .then(b.pri.cmp(&a.pri))
            .then_with(|| a.title.cmp(&b.title))
    });
}

pub fn items_in(fs: &dyn Fs, node_path: &str) -> Vec<Item> {
    let dir = join(&join(node_path, SPACE_DIR), TASKS_DIR);
    let mut names = fs.files(&dir);
    names.retain(|n| n.ends_with(".md") && !n.starts_with('.'));
    let mut items: Vec<Item> =
        names.iter().filter_map(|n| fs.read(&join(&dir, n))).filter_map(|b| item_from_record(&String::from_utf8_lossy(&b))).collect();
    sort_items(&mut items);
    items
}

pub fn due_sort_key(due: Option<&str>) -> (u8, String, u8, String) {
    match due {
        None => (1, String::new(), 0, String::new()),
        Some(d) if d.len() > 10 => (0, d[..10].to_string(), 0, d[11..].to_string()),
        Some(d) => (0, d.to_string(), 1, String::new()),
    }
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

    fn record(id: &str, title: &str, due: &str, pri: u8, state: &str) -> String {
        format!(
            "---\nschema: 3\nid: {id}\ntitle: {title}\ndue: {due}\npri: {pri}\nstate: {state}\nlog_not_a_field: x\n---\n## Log\n- a: b\n"
        )
    }

    #[test]
    fn states_map_to_statuses() {
        let want = [
            ("reminder", Status::Todo),
            ("task", Status::Wip),
            ("defined", Status::Wip),
            ("in_progress", Status::Wip),
            ("failed", Status::Wip),
            ("succeeded", Status::Done),
        ];
        for (state, status) in want {
            let item = item_from_record(&record("a", "t", "null", 0, state)).unwrap();
            assert_eq!(item.status, status, "{state}");
        }
    }

    #[test]
    fn projection_reads_only_the_front_matter() {
        let item = item_from_record(&record("a-b", "Pay rent", "2026-10-28T09:30", 2, "reminder")).unwrap();
        assert_eq!(
            item,
            Item {
                id: "a-b".into(),
                title: "Pay rent".into(),
                status: Status::Todo,
                due: Some("2026-10-28T09:30".into()),
                pri: 2,
                suspended: false
            }
        );
        assert!(item_from_record("no front matter").is_none());
        assert!(item_from_record("---\nid: x\n").is_none());
    }

    #[test]
    fn items_sort_by_due_then_priority_then_title() {
        let mk = |t: &str, due: Option<&str>, pri| Item {
            id: t.into(),
            title: t.into(),
            status: Status::Todo,
            due: due.map(Into::into),
            pri,
            suspended: false,
        };
        let mut v = vec![
            mk("none", None, 3),
            mk("b", Some("2026-10-28"), 0),
            mk("a", Some("2026-10-28"), 0),
            mk("hi", Some("2026-10-28"), 2),
            mk("timed", Some("2026-10-28T08:00"), 0),
            mk("early", Some("2026-10-01"), 0),
        ];
        sort_items(&mut v);
        let order: Vec<_> = v.iter().map(|i| i.title.as_str()).collect();
        assert_eq!(order, ["early", "timed", "hi", "a", "b", "none"]);
    }

    #[test]
    fn suspended_projects_with_and_without_until_and_sorts_last() {
        let rec = |extra: &str| item_from_record(&format!("---\nschema: 4\nid: a\ntitle: t\nstate: task\n{extra}---\n")).unwrap();
        assert!(!rec("").suspended);
        assert!(rec("suspended: true\n").suspended);
        assert!(rec("suspended: true\nsuspend_until: 2999-01-01\n").suspended);
        assert!(!rec("suspended: true\nsuspend_until: 2000-01-01T00:00\n").suspended);
        assert!(!rec("suspended: false\nsuspend_until: 2999-01-01\n").suspended);
        let mk = |t: &str, due: Option<&str>, suspended| Item {
            id: t.into(),
            title: t.into(),
            status: Status::Todo,
            due: due.map(Into::into),
            pri: 0,
            suspended,
        };
        let mut v = vec![mk("s", Some("2026-01-01"), true), mk("n", None, false)];
        sort_items(&mut v);
        assert_eq!(v[0].title, "n");
    }

    #[test]
    fn due_keys_and_overdue() {
        assert!(due_sort_key(Some("2026-10-28T08:00")) < due_sort_key(Some("2026-10-28")));
        assert!(due_sort_key(Some("2026-10-28")) < due_sort_key(None));
        assert!(is_overdue_at("2026-10-24", "2026-10-25", "2026-10-25T08:00"));
        assert!(!is_overdue_at("2026-10-25", "2026-10-25", "2026-10-25T08:00"));
        assert!(is_overdue_at("2026-10-25T07:59", "2026-10-25", "2026-10-25T08:00"));
    }
}
