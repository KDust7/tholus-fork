use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard, PoisonError};

use rustc_hash::FxHashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum LockKind {
    Shared,
    Exclusive,
}

#[derive(Debug, Default, Clone, Copy)]
struct LockState {
    shared: usize,
    exclusive: bool,
}

impl LockState {
    fn is_free(self) -> bool {
        self.shared == 0 && !self.exclusive
    }

    fn admits(self, kind: LockKind) -> bool {
        match kind {
            LockKind::Shared => !self.exclusive,
            LockKind::Exclusive => self.is_free(),
        }
    }
}

thread_local! {
    static HELD: RefCell<FxHashMap<PathBuf, LockState>> = RefCell::new(FxHashMap::default());
}

pub(super) type Holding = Mutex<Option<LockKind>>;

pub(super) fn guard<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

pub(super) fn release(path: &Path, holding: &Holding) {
    let Some(kind) = guard(holding).take() else {
        return;
    };
    HELD.with_borrow_mut(|locks| {
        let Some(state) = locks.get_mut(path) else {
            return;
        };
        match kind {
            LockKind::Shared => state.shared = state.shared.saturating_sub(1),
            LockKind::Exclusive => state.exclusive = false,
        }
        if state.is_free() {
            locks.remove(path);
        }
    });
}

pub(super) fn acquire(path: &Path, holding: &Holding, kind: LockKind) -> bool {
    release(path, holding);
    HELD.with_borrow_mut(|locks| {
        let state = locks.entry(path.to_path_buf()).or_default();
        if !state.admits(kind) {
            return false;
        }
        match kind {
            LockKind::Shared => state.shared += 1,
            LockKind::Exclusive => state.exclusive = true,
        }
        *guard(holding) = Some(kind);
        true
    })
}
