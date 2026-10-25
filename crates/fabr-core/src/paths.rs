use crate::error::FabrError;
use crate::workspace::Workspace;
use std::path::PathBuf;

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
