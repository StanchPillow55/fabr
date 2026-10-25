use crate::error::FabrError;
use crate::workspace::Workspace;
use std::path::{Path, PathBuf};

pub const APP: &str = "fabr";

pub struct AppDirs {
    pub config: PathBuf,
    pub cache: PathBuf,
}

impl AppDirs {
    pub fn resolve() -> AppDirs {
        match directories::BaseDirs::new() {
            Some(b) => AppDirs { config: b.config_dir().to_path_buf(), cache: b.cache_dir().join(APP) },
            None => AppDirs { config: PathBuf::from(".fabr-config"), cache: std::env::temp_dir().join(APP) },
        }
    }

    pub fn env_root() -> Option<PathBuf> {
        std::env::var_os("FABR_ROOT").map(PathBuf::from)
    }

    pub fn workspace(&self, root: Option<PathBuf>) -> Result<Workspace, FabrError> {
        if let Some(r) = &root
            && !r.is_dir()
        {
            return Err(FabrError::NotFound(format!("{} is not a directory", r.display())));
        }
        Workspace::from_config(&self.config, &self.cache, root)
    }
}

pub fn expand_home(path: &str, home: &Path) -> PathBuf {
    match path.strip_prefix("~/") {
        Some(rest) => home.join(rest),
        None if path == "~" => home.to_path_buf(),
        None => PathBuf::from(path),
    }
}

#[cfg(test)]
mod expand_tests {
    use super::*;

    #[test]
    fn expands_only_a_leading_tilde() {
        let h = Path::new("/h");
        assert_eq!(expand_home("~", h), Path::new("/h"));
        assert_eq!(expand_home("~/", h), Path::new("/h"));
        assert_eq!(expand_home("~/x/y", h), Path::new("/h/x/y"));
        assert_eq!(expand_home("/a/~/b", h), Path::new("/a/~/b"));
        assert_eq!(expand_home("~x", h), Path::new("~x"));
    }
}
