use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use futures::lock::{Mutex as AsyncMutex, OwnedMutexGuard};
use rustc_hash::FxHashMap;
use web_time::Duration;

use crate::time::{Elapsed, timeout};

pub const DEFAULT_LOCK_TIMEOUT: Duration = Duration::from_mins(5);

#[derive(Debug)]
pub struct LockedFile {
    path: PathBuf,
    guard: Option<OwnedMutexGuard<()>>,
}

impl LockedFile {
    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn release(&mut self) {
        self.guard = None;
    }
}

#[derive(Debug, Default)]
pub struct LockRegistry {
    entries: Mutex<FxHashMap<PathBuf, Arc<AsyncMutex<()>>>>,
}

impl LockRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    fn entry(&self, path: &Path) -> Option<Arc<AsyncMutex<()>>> {
        let mut entries = self.entries.lock().ok()?;
        Some(Arc::clone(
            entries
                .entry(path.to_path_buf())
                .or_insert_with(|| Arc::new(AsyncMutex::new(()))),
        ))
    }

    pub fn try_acquire(&self, path: &Path) -> Option<LockedFile> {
        let entry = self.entry(path)?;
        let guard = entry.try_lock_owned()?;
        Some(LockedFile {
            path: path.to_path_buf(),
            guard: Some(guard),
        })
    }

    pub async fn acquire(&self, path: &Path, budget: Duration) -> Result<LockedFile, Elapsed> {
        let Some(entry) = self.entry(path) else {
            return Err(Elapsed);
        };
        let guard = timeout(budget, entry.lock_owned()).await?;
        Ok(LockedFile {
            path: path.to_path_buf(),
            guard: Some(guard),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::{DEFAULT_LOCK_TIMEOUT, LockRegistry};
    use std::path::Path;
    use web_time::Duration;

    #[test]
    fn the_default_budget_matches_uv() {
        assert_eq!(DEFAULT_LOCK_TIMEOUT, Duration::from_mins(5));
    }

    #[test]
    fn an_uncontended_lock_is_available() {
        let registry = LockRegistry::new();
        let lock = registry.try_acquire(Path::new("/cache/uv.lock"));
        assert!(lock.is_some());
    }

    #[test]
    fn a_held_lock_blocks_a_second_taker() {
        let registry = LockRegistry::new();
        let _held = registry
            .try_acquire(Path::new("/cache/uv.lock"))
            .expect("first acquire");
        assert!(registry.try_acquire(Path::new("/cache/uv.lock")).is_none());
    }

    #[test]
    fn different_paths_do_not_contend() {
        let registry = LockRegistry::new();
        let _first = registry.try_acquire(Path::new("/cache/a")).expect("first");
        assert!(registry.try_acquire(Path::new("/cache/b")).is_some());
    }

    #[test]
    fn releasing_frees_the_lock() {
        let registry = LockRegistry::new();
        let mut held = registry
            .try_acquire(Path::new("/cache/uv.lock"))
            .expect("acquire");
        held.release();
        assert!(registry.try_acquire(Path::new("/cache/uv.lock")).is_some());
    }

    #[test]
    fn dropping_frees_the_lock() {
        let registry = LockRegistry::new();
        drop(
            registry
                .try_acquire(Path::new("/cache/uv.lock"))
                .expect("acquire"),
        );
        assert!(registry.try_acquire(Path::new("/cache/uv.lock")).is_some());
    }

    #[test]
    fn a_lock_remembers_its_path() {
        let registry = LockRegistry::new();
        let held = registry
            .try_acquire(Path::new("/cache/uv.lock"))
            .expect("acquire");
        assert_eq!(held.path(), Path::new("/cache/uv.lock"));
    }

    #[tokio::test]
    async fn waiting_for_a_free_lock_succeeds() {
        let registry = LockRegistry::new();
        let lock = registry
            .acquire(Path::new("/cache/uv.lock"), Duration::from_secs(1))
            .await;
        assert!(lock.is_ok());
    }

    #[tokio::test]
    async fn waiting_for_a_held_lock_times_out() {
        let registry = LockRegistry::new();
        let _held = registry
            .try_acquire(Path::new("/cache/uv.lock"))
            .expect("acquire");
        let outcome = registry
            .acquire(Path::new("/cache/uv.lock"), Duration::from_millis(20))
            .await;
        assert!(outcome.is_err());
    }
}
