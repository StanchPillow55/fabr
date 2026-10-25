use crate::fs::Fs;
use crate::node::join;
use crate::taskrec::{Approval, FrontMatter, Question, State, TaskRecord};
use serde::{Deserialize, Serialize};

pub const SPACE_DIR: &str = ".fabric-space";
pub const TASKS_DIR: &str = "tasks";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Phase {
    Todo,
    Working,
    NeedsYou,
    Paused,
    Suspended,
    Done,
    Failed,
}

pub trait Phased {
    fn state(&self) -> State;
    fn is_paused(&self) -> bool;
    fn is_waiting(&self) -> bool;
    fn suspend_flag(&self) -> bool;
    fn suspend_until(&self) -> Option<&str>;
    fn question(&self) -> Option<&Question>;
    fn approval(&self) -> Option<&Approval>;

    fn suspended_at(&self, now: &str) -> bool {
        self.suspend_flag() && self.suspend_until().is_none_or(|u| u > now)
    }

    fn question_open(&self) -> bool {
        self.question().is_some_and(|q| q.answer.is_none())
    }

    fn needs_you(&self) -> bool {
        let undecided = self.approval().is_none_or(|a| !matches!(a.status.as_str(), "approved" | "refused"));
        (self.is_waiting() && self.question_open()) || (self.state() == State::Defined && undecided)
    }
}

macro_rules! phased {
    ($t:ty) => {
        impl Phased for $t {
            fn state(&self) -> State {
                self.state
            }
            fn is_paused(&self) -> bool {
                self.paused
            }
            fn is_waiting(&self) -> bool {
                self.waiting
            }
            fn suspend_flag(&self) -> bool {
                self.suspended
            }
            fn suspend_until(&self) -> Option<&str> {
                self.suspend_until.as_deref()
            }
            fn question(&self) -> Option<&Question> {
                self.question.as_ref()
            }
            fn approval(&self) -> Option<&Approval> {
                self.approval.as_ref()
            }
        }
    };
}

phased!(TaskRecord);
phased!(FrontMatter);
phased!(Front);

impl Phase {
    pub fn as_str(&self) -> &'static str {
        match self {
            Phase::Todo => "todo",
            Phase::Working => "working",
            Phase::NeedsYou => "needs_you",
            Phase::Paused => "paused",
            Phase::Suspended => "suspended",
            Phase::Done => "done",
            Phase::Failed => "failed",
        }
    }

    pub fn of(rec: &impl Phased, now: &str) -> Phase {
        match rec.state() {
            State::Succeeded => Phase::Done,
            State::Failed => Phase::Failed,
            _ if rec.suspended_at(now) => Phase::Suspended,
            State::Reminder => Phase::Todo,
            _ if rec.is_paused() => Phase::Paused,
            _ if rec.needs_you() => Phase::NeedsYou,
            _ => Phase::Working,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Item {
    pub id: String,
    pub title: String,
    pub phase: Phase,
    #[serde(default)]
    pub due: Option<String>,
    #[serde(default)]
    pub pri: u8,
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
    waiting: bool,
    #[serde(default)]
    paused: bool,
    #[serde(default)]
    suspended: bool,
    #[serde(default)]
    suspend_until: Option<String>,
    #[serde(default)]
    question: Option<Question>,
    #[serde(default)]
    approval: Option<Approval>,
}

pub fn item_from_record(text: &str) -> Option<Item> {
    let rest = text.strip_prefix("---")?;
    let yaml = &rest[..rest.find("\n---")?];
    let f: Front = serde_yaml::from_str(yaml).ok()?;
    let phase = Phase::of(&f, &now_minute());
    Some(Item { id: f.id, title: f.title, phase, due: f.due, pri: f.pri.min(3) })
}

pub fn pri_rank(pri: u8) -> u8 {
    if pri == 0 { u8::MAX } else { pri }
}

pub fn sort_items(items: &mut [Item]) {
    items.sort_by(|a, b| {
        (a.phase == Phase::Suspended)
            .cmp(&(b.phase == Phase::Suspended))
            .then_with(|| due_sort_key(a.due.as_deref()).cmp(&due_sort_key(b.due.as_deref())))
            .then(pri_rank(a.pri).cmp(&pri_rank(b.pri)))
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
    fn projection_reads_only_the_front_matter() {
        let item = item_from_record(&record("a-b", "Pay rent", "2026-10-28T09:30", 2, "reminder")).unwrap();
        assert_eq!(
            item,
            Item { id: "a-b".into(), title: "Pay rent".into(), phase: Phase::Todo, due: Some("2026-10-28T09:30".into()), pri: 2 }
        );
        assert!(item_from_record("no front matter").is_none());
        assert!(item_from_record("---\nid: x\n").is_none());
    }

    fn mk(t: &str, due: Option<&str>, pri: u8, phase: Phase) -> Item {
        Item { id: t.into(), title: t.into(), phase, due: due.map(Into::into), pri }
    }

    #[test]
    fn items_sort_by_due_then_priority_then_title() {
        let mut v = vec![
            mk("none", None, 3, Phase::Todo),
            mk("b", Some("2026-10-28"), 0, Phase::Todo),
            mk("a", Some("2026-10-28"), 0, Phase::Todo),
            mk("lo", Some("2026-10-28"), 3, Phase::Todo),
            mk("hi", Some("2026-10-28"), 1, Phase::Todo),
            mk("timed", Some("2026-10-28T08:00"), 0, Phase::Todo),
            mk("early", Some("2026-10-01"), 0, Phase::Todo),
        ];
        sort_items(&mut v);
        let order: Vec<_> = v.iter().map(|i| i.title.as_str()).collect();
        assert_eq!(order, ["early", "timed", "hi", "lo", "a", "b", "none"]);
    }

    #[test]
    fn suspended_projects_with_and_without_until_and_sorts_last() {
        let rec = |extra: &str| item_from_record(&format!("---\nschema: 4\nid: a\ntitle: t\nstate: task\n{extra}---\n")).unwrap().phase;
        assert_eq!(rec(""), Phase::Working);
        assert_eq!(rec("suspended: true\n"), Phase::Suspended);
        assert_eq!(rec("suspended: true\nsuspend_until: 2999-01-01\n"), Phase::Suspended);
        assert_eq!(rec("suspended: true\nsuspend_until: 2000-01-01T00:00\n"), Phase::Working);
        assert_eq!(rec("suspended: false\nsuspend_until: 2999-01-01\n"), Phase::Working);
        let mut v = vec![mk("s", Some("2026-01-01"), 0, Phase::Suspended), mk("n", None, 0, Phase::Working)];
        sort_items(&mut v);
        assert_eq!(v[0].title, "n");
    }

    #[test]
    fn phase_table() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/phase-table.json");
        let rows: Vec<serde_json::Value> = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
        assert!(rows.len() >= 12);
        for row in rows {
            let mut fm = row["front_matter"].clone();
            fm["id"] = "x".into();
            fm["title"] = "t".into();
            let f: Front = serde_json::from_value(fm).unwrap();
            let want: Phase = serde_json::from_value(row["phase"].clone()).unwrap();
            assert_eq!(Phase::of(&f, row["now"].as_str().unwrap()), want, "{}", row["name"]);
        }
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
