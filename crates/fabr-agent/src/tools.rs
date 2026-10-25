use fabr_core::FabrError;
use std::path::{Component, Path, PathBuf};

pub fn read_file(node_root: &Path, rel: &str) -> Result<String, FabrError> {
    let path = resolve_inside(node_root, rel, false)?;
    if !path.is_file() {
        return Err(FabrError::NotFound(format!("no such file {rel}")));
    }
    let bytes = std::fs::read(&path).map_err(|e| FabrError::Io(format!("{}: {e}", path.display())))?;
    String::from_utf8(bytes).map_err(|e| FabrError::BadRequest(format!("file is not utf-8: {e}")))
}

pub fn write_file(node_root: &Path, rel: &str, content: &str) -> Result<(), FabrError> {
    let path = resolve_inside(node_root, rel, true)?;
    if path.is_dir() {
        return Err(FabrError::BadRequest("path is a directory".into()));
    }
    std::fs::write(&path, content).map_err(|e| FabrError::Io(format!("{}: {e}", path.display())))
}

pub fn list_dir(node_root: &Path, rel: &str) -> Result<Vec<String>, FabrError> {
    let path = if rel.is_empty() || rel == "." { canonicalize_root(node_root)? } else { resolve_inside(node_root, rel, false)? };
    if !path.is_dir() {
        return Err(FabrError::NotFound(format!("no such directory {rel}")));
    }
    let mut names = Vec::new();
    for ent in std::fs::read_dir(&path).map_err(|e| FabrError::Io(format!("{}: {e}", path.display())))? {
        let ent = ent.map_err(|e| FabrError::Io(e.to_string()))?;
        if let Ok(name) = ent.file_name().into_string() {
            names.push(name);
        }
    }
    names.sort();
    Ok(names)
}

const RESULT_CAP: usize = 16 * 1024;

pub fn run(node_root: &Path, name: &str, path: &str, content: Option<&str>) -> Result<(String, String), FabrError> {
    match name {
        "read_file" => {
            let text = read_file(node_root, path)?;
            let line = format!("read_file {path} ({} bytes)", text.len());
            Ok((line, text.chars().take(RESULT_CAP).collect()))
        }
        "write_file" => {
            let body = content.unwrap_or("");
            write_file(node_root, path, body)?;
            Ok((format!("write_file {path}"), format!("wrote {} bytes to {path}", body.len())))
        }
        "list_dir" => {
            let names = list_dir(node_root, path)?;
            let shown = if names.is_empty() { "(empty)".to_string() } else { names.join("\n") };
            Ok((format!("list_dir {path} ({})", names.len()), shown.chars().take(RESULT_CAP).collect()))
        }
        other => Err(FabrError::BadRequest(format!("unknown tool {other}"))),
    }
}

fn canonicalize_root(root: &Path) -> Result<PathBuf, FabrError> {
    root.canonicalize().map_err(|e| FabrError::NotFound(format!("node root: {e}")))
}

fn reserved_name(name: &str) -> bool {
    name == ".fabr" || name == ".fabric-space"
}

fn under_reserved(root: &Path, path: &Path) -> bool {
    path.strip_prefix(root)
        .is_ok_and(|rel| rel.components().any(|c| matches!(c, Component::Normal(s) if reserved_name(&s.to_string_lossy()))))
}

fn outside(root: &Path, path: &Path) -> bool {
    !path.starts_with(root) || under_reserved(root, path)
}

fn resolve_inside(root: &Path, rel: &str, allow_missing_final: bool) -> Result<PathBuf, FabrError> {
    if rel.is_empty() || rel.contains('\0') {
        return Err(FabrError::BadRequest("bad path".into()));
    }
    let root = canonicalize_root(root)?;
    let rel_path = Path::new(rel);
    if rel_path.is_absolute() {
        return Err(FabrError::BadRequest("path is outside the node".into()));
    }
    let mut cur = root.clone();
    let comps: Vec<_> = rel_path.components().collect();
    for (i, comp) in comps.iter().enumerate() {
        let last = i + 1 == comps.len();
        match comp {
            Component::CurDir => {}
            Component::ParentDir => {
                if cur == root {
                    return Err(FabrError::BadRequest("path is outside the node".into()));
                }
                cur.pop();
                if outside(&root, &cur) {
                    return Err(FabrError::BadRequest("path is outside the node".into()));
                }
            }
            Component::Normal(name) => {
                let name = name.to_string_lossy();
                if reserved_name(&name) {
                    return Err(FabrError::BadRequest("path is reserved".into()));
                }
                let next = cur.join(name.as_ref());
                match std::fs::symlink_metadata(&next) {
                    Ok(meta) => {
                        if meta.file_type().is_symlink() {
                            let canon =
                                std::fs::canonicalize(&next).map_err(|_| FabrError::BadRequest("symlink is outside the node".into()))?;
                            if outside(&root, &canon) {
                                return Err(FabrError::BadRequest("symlink is outside the node".into()));
                            }
                            cur = canon;
                        } else {
                            let canon = std::fs::canonicalize(&next).map_err(|e| FabrError::Io(e.to_string()))?;
                            if outside(&root, &canon) {
                                return Err(FabrError::BadRequest("path is outside the node".into()));
                            }
                            cur = canon;
                        }
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::NotFound && last && allow_missing_final => {
                        if outside(&root, &cur) {
                            return Err(FabrError::BadRequest("path is outside the node".into()));
                        }
                        cur = next;
                    }
                    Err(_) => return Err(FabrError::NotFound(format!("no such path {rel}"))),
                }
            }
            _ => return Err(FabrError::BadRequest("bad path".into())),
        }
    }
    if outside(&root, &cur) {
        return Err(FabrError::BadRequest("path is outside the node".into()));
    }
    Ok(cur)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn node() -> (tempfile::TempDir, std::path::PathBuf) {
        let tmp = tempfile::tempdir().unwrap();
        let node = tmp.path().join("node");
        std::fs::create_dir_all(&node).unwrap();
        (tmp, node)
    }

    #[test]
    fn phase5_write_inside_node() {
        let (_tmp, node) = node();
        write_file(&node, "greet.txt", "hi").unwrap();
        assert_eq!(read_file(&node, "greet.txt").unwrap(), "hi");
        assert_eq!(list_dir(&node, ".").unwrap(), vec!["greet.txt".to_string()]);
    }

    #[test]
    fn phase5_reject_reserved_paths() {
        let (_tmp, node) = node();
        std::fs::create_dir_all(node.join(".fabr")).unwrap();
        std::fs::write(node.join(".fabr/meta.toml"), "keep\n").unwrap();
        std::fs::create_dir_all(node.join(".fabric-space")).unwrap();
        let err = write_file(&node, ".fabr/meta.toml", "hack").unwrap_err();
        assert_eq!(std::fs::read_to_string(node.join(".fabr/meta.toml")).unwrap(), "keep\n", "{err}");
        assert!(write_file(&node, ".fabric-space/secret", "no").is_err());
        assert!(!node.join(".fabric-space/secret").exists());
        assert!(read_file(&node, ".fabr/meta.toml").is_err());
        assert_eq!(std::fs::read_to_string(node.join(".fabr/meta.toml")).unwrap(), "keep\n");
    }

    #[test]
    fn phase5_reject_symlink_escape() {
        let (tmp, node) = node();
        let outside = tmp.path().join("outside");
        std::fs::create_dir_all(&outside).unwrap();
        let landed = outside.join("landed.txt");
        std::os::unix::fs::symlink(&landed, node.join("leak")).unwrap();
        let err = write_file(&node, "leak", "secret").unwrap_err();
        assert!(!landed.exists(), "{err}");
        std::os::unix::fs::symlink(&outside, node.join("out")).unwrap();
        assert!(write_file(&node, "out/file.txt", "no").is_err());
        assert!(!outside.join("file.txt").exists());
        assert!(write_file(&node, "../escaped.txt", "no").is_err());
        assert!(!tmp.path().join("escaped.txt").exists());
    }
}
