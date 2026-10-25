pub mod local;
pub mod tar;

pub use self::local::LocalFs;
pub use self::tar::TarSnapshot;

pub trait Fs: Send + Sync {
    fn is_dir(&self, rel: &str) -> bool;
    fn subdirs(&self, rel: &str) -> Vec<String>;
    fn exists(&self, rel: &str) -> bool;
    fn read(&self, rel: &str) -> Option<Vec<u8>>;
    fn files(&self, rel: &str) -> Vec<String> {
        let _ = rel;
        Vec::new()
    }
    fn try_subdirs(&self, rel: &str) -> Result<Vec<String>, String> {
        Ok(self.subdirs(rel))
    }
}
