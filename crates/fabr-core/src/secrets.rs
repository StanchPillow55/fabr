use crate::error::FabrError;
use crate::sync::lock;
use serde::{Deserialize, Deserializer};
use std::collections::HashMap;
use std::sync::Mutex;

pub const SERVICE: &str = "app.fabr";

#[derive(Clone, PartialEq, Eq, Default)]
pub struct Secret(String);

impl Secret {
    pub fn new(s: impl Into<String>) -> Secret {
        Secret(s.into())
    }
    pub fn expose(&self) -> &str {
        &self.0
    }
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl std::fmt::Debug for Secret {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Secret(***)")
    }
}

impl<'de> Deserialize<'de> for Secret {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        String::deserialize(d).map(Secret)
    }
}

fn account(source_id: &str) -> String {
    format!("ssh:{source_id}")
}

static MEMORY: Mutex<Option<HashMap<String, String>>> = Mutex::new(None);

pub fn use_memory_store() {
    let mut m = lock(&MEMORY);
    if m.is_none() {
        *m = Some(HashMap::new());
    }
}

fn with_memory<T>(f: impl FnOnce(&mut HashMap<String, String>) -> T) -> Option<T> {
    lock(&MEMORY).as_mut().map(f)
}

#[cfg(all(feature = "keychain", any(target_os = "macos", target_os = "linux")))]
mod os {
    use super::*;

    fn entry(source_id: &str) -> Result<keyring::Entry, FabrError> {
        keyring::Entry::new(SERVICE, &account(source_id)).map_err(unavailable)
    }

    fn unavailable(e: keyring::Error) -> FabrError {
        match e {
            keyring::Error::NoStorageAccess(_) | keyring::Error::PlatformFailure(_) => FabrError::Io(format!(
                "the system keychain is not available ({e}); the password was not stored. \
                 On Linux this needs a running Secret Service (gnome-keyring or KWallet)"
            )),
            other => FabrError::Io(format!("keychain: {other}")),
        }
    }

    pub fn set(source_id: &str, pw: &str) -> Result<(), FabrError> {
        entry(source_id)?.set_password(pw).map_err(unavailable)
    }

    pub fn get(source_id: &str) -> Result<Option<String>, FabrError> {
        match entry(source_id)?.get_password() {
            Ok(p) => Ok(Some(p)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(e) => Err(unavailable(e)),
        }
    }

    pub fn delete(source_id: &str) -> Result<(), FabrError> {
        match entry(source_id)?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(e) => Err(unavailable(e)),
        }
    }
}

#[cfg(not(all(feature = "keychain", any(target_os = "macos", target_os = "linux"))))]
mod os {
    use super::*;
    const MSG: &str = "this build has no keychain support; the password was not stored";
    pub fn set(_: &str, _: &str) -> Result<(), FabrError> {
        Err(FabrError::Io(MSG.into()))
    }
    pub fn get(_: &str) -> Result<Option<String>, FabrError> {
        Ok(None)
    }
    pub fn delete(_: &str) -> Result<(), FabrError> {
        Ok(())
    }
}

pub fn set_password(source_id: &str, pw: &str) -> Result<(), FabrError> {
    if pw.is_empty() {
        return delete_password(source_id);
    }
    if let Some(()) = with_memory(|m| {
        m.insert(account(source_id), pw.to_string());
    }) {
        return Ok(());
    }
    os::set(source_id, pw)
}

pub fn get_password(source_id: &str) -> Option<String> {
    if let Some(v) = with_memory(|m| m.get(&account(source_id)).cloned()) {
        return v;
    }
    match os::get(source_id) {
        Ok(v) => v,
        Err(e) => {
            log::warn!("keychain lookup for source {source_id}: {}", e.message());
            None
        }
    }
}

pub fn delete_password(source_id: &str) -> Result<(), FabrError> {
    if let Some(()) = with_memory(|m| {
        m.remove(&account(source_id));
    }) {
        return Ok(());
    }
    os::delete(source_id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn memory_store_round_trip_and_redaction() {
        use_memory_store();
        assert_eq!(get_password("t-sec"), None);
        set_password("t-sec", "hunter2").unwrap();
        assert_eq!(get_password("t-sec").as_deref(), Some("hunter2"));
        set_password("t-sec", "").unwrap();
        assert_eq!(get_password("t-sec"), None);
        let s = Secret::new("hunter2");
        assert!(!format!("{s:?}").contains("hunter2"));
        let s: Secret = serde_json::from_str("\"pw\"").unwrap();
        assert_eq!(s.expose(), "pw");
    }
}
