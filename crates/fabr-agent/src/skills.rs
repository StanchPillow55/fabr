//! Loadable SKILLS (M5).
//!
//! A skill is a folder-scoped "how I like X done" personalization that gets
//! injected into the agent's gather context. It lives in a directory under
//! `<node>/.fabric-space/skills/<name>/` (folder-scoped, matching "filetree is
//! the spine") or under `~/.fabric-space/skills/<name>/` (global). Each skill
//! directory holds a `skill.md` with YAML front matter `{ name, when_to_use }`
//! and a markdown body of instructions.
//!
//! [`load_skills`] walks the node directory and its ancestors (plus the global
//! directory) collecting skills; [`render_skills`] turns them into a context
//! block that the gather step concatenates into the task record's context.
//!
//! Front matter is parsed defensively with the same approach the project uses
//! for task records (`strip_prefix("---")` + `find("\n---")` + `serde_yaml`),
//! so a malformed skill is skipped rather than failing the whole load.

use serde::Deserialize;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

/// Directory name that holds the per-node fabric space.
const SPACE_DIR: &str = ".fabric-space";
/// Sub-directory (under a space) that holds skills.
const SKILLS_DIR: &str = "skills";
/// File within a skill directory that carries the front matter and body.
const SKILL_FILE: &str = "skill.md";

/// One loaded skill.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Skill {
    /// Short identifier from front matter (falls back to the directory name).
    pub name: String,
    /// When the agent should reach for this skill, from front matter.
    pub when_to_use: String,
    /// The markdown instructions below the front matter.
    pub body: String,
}

/// Front matter schema. Both fields are optional so a half-filled skill still
/// loads; the directory name backfills a missing `name`.
#[derive(Debug, Default, Deserialize)]
struct FrontMatter {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    when_to_use: Option<String>,
}

/// Walk `node_dir` and its ancestors, then the global `~/.fabric-space`,
/// collecting every readable skill. Nearer directories win on a name clash
/// (a folder-scoped skill overrides a global one of the same name), and the
/// result is sorted by name for a stable context block.
pub fn load_skills(node_dir: &Path) -> Vec<Skill> {
    let mut out: Vec<Skill> = Vec::new();
    let mut seen: BTreeSet<String> = BTreeSet::new();
    for dir in search_dirs(node_dir) {
        for skill in skills_in(&dir) {
            if seen.insert(skill.name.clone()) {
                out.push(skill);
            }
        }
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

/// The skills directories to scan, nearest first: the node's own space, each
/// ancestor's space, then the global space under `$HOME`.
fn search_dirs(node_dir: &Path) -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    let mut cur = Some(node_dir);
    while let Some(dir) = cur {
        dirs.push(dir.join(SPACE_DIR).join(SKILLS_DIR));
        cur = dir.parent();
    }
    if let Some(home) = home_dir() {
        let global = home.join(SPACE_DIR).join(SKILLS_DIR);
        if !dirs.contains(&global) {
            dirs.push(global);
        }
    }
    dirs
}

fn home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME").filter(|h| !h.is_empty()).map(PathBuf::from)
}

/// Load every `<dir>/<name>/skill.md`, skipping anything unreadable or malformed.
fn skills_in(dir: &Path) -> Vec<Skill> {
    let Ok(entries) = std::fs::read_dir(dir) else { return Vec::new() };
    let mut named: Vec<_> = entries.flatten().collect();
    named.sort_by_key(|e| e.file_name());
    let mut out = Vec::new();
    for ent in named {
        if !ent.path().is_dir() {
            continue;
        }
        let file = ent.path().join(SKILL_FILE);
        let Ok(text) = std::fs::read_to_string(&file) else { continue };
        let fallback = ent.file_name().to_string_lossy().to_string();
        if let Some(skill) = parse_skill(&text, &fallback) {
            out.push(skill);
        }
    }
    out
}

/// Parse one `skill.md`. Missing or broken front matter degrades gracefully:
/// the whole text becomes the body and the directory name backfills the name.
fn parse_skill(text: &str, fallback_name: &str) -> Option<Skill> {
    let (fm, body) = split_front_matter(text);
    let name = fm
        .name
        .map(|n| n.trim().to_string())
        .filter(|n| !n.is_empty())
        .unwrap_or_else(|| fallback_name.to_string());
    let when_to_use = fm.when_to_use.unwrap_or_default().trim().to_string();
    let body = body.trim().to_string();
    if name.is_empty() {
        return None;
    }
    Some(Skill { name, when_to_use, body })
}

/// Split leading YAML front matter from the markdown body, mirroring the task
/// record parser. If there is no well-formed front matter, the front matter is
/// empty and the whole text is the body.
fn split_front_matter(text: &str) -> (FrontMatter, &str) {
    let Some(rest) = text.strip_prefix("---") else { return (FrontMatter::default(), text) };
    let Some(end) = rest.find("\n---") else { return (FrontMatter::default(), text) };
    let yaml = &rest[..end];
    let after = &rest[end + 4..];
    let body = after.strip_prefix('\n').unwrap_or(after);
    let body = body.strip_prefix("---").map(|b| b.trim_start_matches('\n')).unwrap_or(body);
    match serde_yaml::from_str::<FrontMatter>(yaml) {
        Ok(fm) => (fm, body),
        Err(_) => (FrontMatter::default(), text),
    }
}

/// Render the loaded skills into a context block to concatenate into the gather
/// context. Returns an empty string when there are no skills, so the caller can
/// append unconditionally without adding noise.
pub fn render_skills(skills: &[Skill]) -> String {
    if skills.is_empty() {
        return String::new();
    }
    let mut out = String::from("### Applicable skills\n");
    for skill in skills {
        out.push_str(&format!("\n- {}", skill.name));
        if !skill.when_to_use.is_empty() {
            out.push_str(&format!(" — when to use: {}", skill.when_to_use));
        }
        out.push('\n');
        if !skill.body.is_empty() {
            for line in skill.body.lines() {
                out.push_str("  ");
                out.push_str(line);
                out.push('\n');
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn write_skill(root: &Path, name: &str, contents: &str) {
        let dir = root.join(SPACE_DIR).join(SKILLS_DIR).join(name);
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join(SKILL_FILE), contents).unwrap();
    }

    #[test]
    fn loads_a_sample_skill_from_a_temp_dir() {
        let tmp = tempfile::tempdir().unwrap();
        write_skill(
            tmp.path(),
            "todos",
            "---\nname: todos\nwhen_to_use: when adding or editing a TODO list\n---\nKeep one task per line.\nPrefix done items with [x].\n",
        );
        let skills = load_skills(tmp.path());
        assert_eq!(skills.len(), 1, "{skills:?}");
        let s = &skills[0];
        assert_eq!(s.name, "todos");
        assert_eq!(s.when_to_use, "when adding or editing a TODO list");
        assert!(s.body.contains("Keep one task per line."), "{}", s.body);
        assert!(s.body.contains("[x]"), "{}", s.body);
    }

    #[test]
    fn render_produces_a_context_block() {
        let skills = vec![Skill {
            name: "todos".into(),
            when_to_use: "editing a TODO list".into(),
            body: "Keep one task per line.".into(),
        }];
        let block = render_skills(&skills);
        assert!(block.starts_with("### Applicable skills\n"), "{block}");
        assert!(block.contains("- todos — when to use: editing a TODO list"), "{block}");
        assert!(block.contains("  Keep one task per line."), "{block}");
    }

    #[test]
    fn render_of_nothing_is_empty() {
        assert_eq!(render_skills(&[]), "");
    }

    #[test]
    fn ancestors_are_searched_and_nearer_wins() {
        let tmp = tempfile::tempdir().unwrap();
        let node = tmp.path().join("a").join("b");
        fs::create_dir_all(&node).unwrap();
        // ancestor skill named "style"
        write_skill(tmp.path(), "style", "---\nname: style\nwhen_to_use: far\n---\nouter body\n");
        // nearer skill of the same name should win
        write_skill(&node, "style", "---\nname: style\nwhen_to_use: near\n---\ninner body\n");
        // and a node-only skill
        write_skill(&node, "todos", "---\nname: todos\nwhen_to_use: lists\n---\nlist body\n");
        let skills = load_skills(&node);
        let names: Vec<_> = skills.iter().map(|s| s.name.as_str()).collect();
        assert_eq!(names, vec!["style", "todos"], "{skills:?}");
        let style = skills.iter().find(|s| s.name == "style").unwrap();
        assert_eq!(style.when_to_use, "near", "nearer skill should win");
        assert_eq!(style.body, "inner body");
    }

    #[test]
    fn malformed_front_matter_degrades_to_body() {
        let tmp = tempfile::tempdir().unwrap();
        // No front matter at all: directory name backfills, whole text is body.
        write_skill(tmp.path(), "loose", "just some instructions\nno front matter here\n");
        // Unterminated front matter: treated as body too.
        write_skill(tmp.path(), "broken", "---\nname: broken\nwhen_to_use: x\nstill going\n");
        let skills = load_skills(tmp.path());
        let loose = skills.iter().find(|s| s.name == "loose").unwrap();
        assert_eq!(loose.when_to_use, "");
        assert!(loose.body.contains("just some instructions"), "{}", loose.body);
        let broken = skills.iter().find(|s| s.name == "broken").unwrap();
        assert!(broken.body.contains("name: broken"), "{}", broken.body);
    }

    #[test]
    fn empty_when_dir_is_missing() {
        let tmp = tempfile::tempdir().unwrap();
        assert!(load_skills(tmp.path()).is_empty());
    }
}
