use std::sync::{Mutex, MutexGuard, RwLock};

pub(crate) fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

pub(crate) struct Shared<T>(RwLock<T>);

impl<T> Shared<T> {
    pub(crate) const fn new(v: T) -> Self {
        Shared(RwLock::new(v))
    }

    pub(crate) fn get(&self) -> T
    where
        T: Clone,
    {
        self.0.read().unwrap_or_else(|e| e.into_inner()).clone()
    }

    pub(crate) fn set(&self, v: T) {
        *self.0.write().unwrap_or_else(|e| e.into_inner()) = v;
    }
}
