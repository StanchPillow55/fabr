use crate::error::FabrError;
use crate::sources::prune_name_ok;

pub const EXIT_MISMATCH: i32 = 42;
pub const EXIT_IO: i32 = 43;
pub const EXIT_NOT_NODE: i32 = 44;
pub const EXIT_EXTRA_FILES: i32 = 45;

const TAR: &str = "COPYFILE_DISABLE=1 tar -c -f - --null --no-recursion -T -";

pub fn snapshot_script(prune: &[String]) -> Result<String, FabrError> {
    if let Some(p) = prune.iter().find(|p| !prune_name_ok(p)) {
        return Err(FabrError::BadRequest(format!("bad prune name {p:?}")));
    }
    let pruned = if prune.is_empty() {
        String::new()
    } else {
        let names: Vec<String> = prune.iter().map(|p| format!("-name {p}")).collect();
        format!("-o \"(\" {} \")\" -type d -prune -print0 ", names.join(" -o "))
    };
    Ok(format!(
        "cd \"$1\" || exit {EXIT_NOT_NODE}; find . \"(\" -type d -name \".*\" \"!\" -name . \"!\" -name .fabr \"!\" -name .fabric-space \")\" -prune \
{pruned}-o \"(\" -type d -o -path \"*/.fabr/meta.toml\" -o -path \"*/.fabric-space/tasks/*.md\" \")\" -print0 | {TAR}"
    ))
}

pub fn node_script() -> String {
    format!(
        "cd \"$1\" || exit {EXIT_NOT_NODE}; [ -d \"$2/.fabr\" ] || exit {EXIT_NOT_NODE}; \
{{ find \"$2/.fabr\" \"(\" -type d \"!\" -name .fabr \")\" -prune -o \"(\" -type d -o -name meta.toml \")\" -print0; \
[ -d \"$2/.fabric-space/tasks\" ] && find \"$2/.fabric-space/tasks\" \"(\" -type d \"!\" -name tasks \")\" -prune -o \"(\" -type d -o -name \"*.md\" \")\" -print0; }} | {TAR}"
    )
}

pub fn list_script() -> String {
    format!(
        "cd \"$1\" || exit {EXIT_NOT_NODE}; cd \"$2\" || exit {EXIT_NOT_NODE}; \
find . \"(\" -path \"./*/*\" \"!\" -path \"./*/.fabr\" \")\" -prune -o -type d -print0"
    )
}

pub fn parse_list(out: &[u8]) -> Vec<(String, bool)> {
    let mut dirs: Vec<String> = Vec::new();
    let mut managed = std::collections::HashSet::new();
    for raw in out.split(|b| *b == 0) {
        let Ok(p) = std::str::from_utf8(raw) else { continue };
        let Some(rest) = p.strip_prefix("./") else { continue };
        match rest.split_once('/') {
            None if !rest.is_empty() => dirs.push(rest.to_string()),
            Some((name, ".fabr")) => {
                managed.insert(name.to_string());
            }
            _ => {}
        }
    }
    dirs.into_iter()
        .map(|d| {
            let m = managed.contains(&d);
            (d, m)
        })
        .collect()
}

fn version_guard() -> Vec<String> {
    let (m, io, nn) = (EXIT_MISMATCH, EXIT_IO, EXIT_NOT_NODE);
    vec![
        "d=\"$1/.fabr\"".to_string(),
        format!("[ -d \"$d\" ] || exit {nn}"),
        format!("cd \"$d\" || exit {io}"),
        "h() { cat meta.toml 2>/dev/null | { sha1sum 2>/dev/null || shasum -a 1; }; }".to_string(),
        format!("cur=$(h) || exit {io}"),
        "cur=${cur%% *}".to_string(),
        format!("[ -n \"$cur\" ] || exit {io}"),
        format!("[ \"$cur\" = \"$2\" ] || {{ echo \"$cur\" >&2; exit {m}; }}"),
    ]
}

fn guarded(tail: Vec<String>) -> String {
    let mut parts = version_guard();
    parts.extend(tail);
    parts.join("; ")
}

pub fn write_script() -> String {
    let io = EXIT_IO;
    guarded(vec![
        format!("t=$(mktemp \"./.$3.XXXXXX\") || exit {io}"),
        format!("if [ -e \"$3\" ]; then cp -p \"$3\" \"$t\" || {{ rm -f \"$t\"; exit {io}; }}; else chmod 644 \"$t\"; fi"),
        format!("cat >\"$t\" && mv -f \"$t\" \"$3\" || {{ rm -f \"$t\"; exit {io}; }}"),
        format!("new=$(h) || exit {io}"),
        "echo \"${new%% *}\"".to_string(),
    ])
}

pub fn remove_script() -> String {
    let (io, ex) = (EXIT_IO, EXIT_EXTRA_FILES);
    guarded(vec![
        "extra=$(ls -A | grep -v -e '^meta[.]toml$' | head -n 1)".to_string(),
        format!("[ -z \"$extra\" ] || exit {ex}"),
        format!("rm -f meta.toml || exit {io}"),
        format!("cd .. || exit {io}"),
        format!("rmdir .fabr || exit {io}"),
    ])
}

pub fn init_script() -> String {
    format!("[ -d \"$1\" ] || exit {EXIT_NOT_NODE}; [ -e \"$1/.fabr\" ] && exit {EXIT_MISMATCH}; mkdir \"$1/.fabr\" || exit {EXIT_IO}")
}

pub fn quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\"'\"'"))
}

pub fn shell_command(script: &str, args: &[&str]) -> String {
    let mut out = format!("sh -c {} sh", quote(script));
    for a in args {
        out.push(' ');
        out.push_str(&quote(a));
    }
    out
}
