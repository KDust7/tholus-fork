use std::ffi::{OsStr, OsString};

#[cfg(any(target_family = "wasm", test))]
mod backing {
    use std::cell::RefCell;
    use std::collections::BTreeMap;
    use std::ffi::{OsStr, OsString};

    thread_local! {
        static ENVIRONMENT: RefCell<BTreeMap<OsString, OsString>> = const {
            RefCell::new(BTreeMap::new())
        };
    }

    pub(crate) fn get(key: &OsStr) -> Option<OsString> {
        ENVIRONMENT.with(|environment| environment.borrow().get(key).cloned())
    }

    pub(crate) fn set(key: OsString, value: OsString) {
        ENVIRONMENT.with(|environment| {
            environment.borrow_mut().insert(key, value);
        });
    }

    pub(crate) fn unset(key: &OsStr) {
        ENVIRONMENT.with(|environment| {
            environment.borrow_mut().remove(key);
        });
    }

    pub(crate) fn replace(entries: impl IntoIterator<Item = (OsString, OsString)>) {
        ENVIRONMENT.with(|environment| {
            *environment.borrow_mut() = entries.into_iter().collect();
        });
    }

    pub(crate) fn snapshot() -> Vec<(OsString, OsString)> {
        ENVIRONMENT.with(|environment| {
            environment
                .borrow()
                .iter()
                .map(|(key, value)| (key.clone(), value.clone()))
                .collect()
        })
    }

    pub(crate) fn lookup(key: &OsStr) -> Result<String, std::env::VarError> {
        match get(key) {
            None => Err(std::env::VarError::NotPresent),
            Some(value) => value.into_string().map_err(std::env::VarError::NotUnicode),
        }
    }
}

#[cfg(target_family = "wasm")]
pub fn set(key: impl Into<OsString>, value: impl Into<OsString>) {
    backing::set(key.into(), value.into());
}

#[cfg(target_family = "wasm")]
pub fn unset(key: impl AsRef<OsStr>) {
    backing::unset(key.as_ref());
}

#[cfg(target_family = "wasm")]
pub fn replace(entries: impl IntoIterator<Item = (OsString, OsString)>) {
    backing::replace(entries);
}

#[cfg(target_family = "wasm")]
pub fn snapshot() -> Vec<(OsString, OsString)> {
    backing::snapshot()
}

#[cfg(target_family = "wasm")]
pub fn var_os(key: impl AsRef<OsStr>) -> Option<OsString> {
    backing::get(key.as_ref())
}

#[cfg(not(target_family = "wasm"))]
pub fn var_os(key: impl AsRef<OsStr>) -> Option<OsString> {
    std::env::var_os(key)
}

#[cfg(target_family = "wasm")]
pub fn var(key: impl AsRef<OsStr>) -> Result<String, std::env::VarError> {
    backing::lookup(key.as_ref())
}

#[cfg(not(target_family = "wasm"))]
pub fn var(key: impl AsRef<OsStr>) -> Result<String, std::env::VarError> {
    std::env::var(key)
}

#[cfg(test)]
mod tests {
    use super::backing;
    use std::env::VarError;

    #[test]
    fn reads_back_what_the_host_installed() {
        backing::replace([("HOME".into(), "/home/browser".into())]);
        assert_eq!(backing::lookup("HOME".as_ref()).as_deref(), Ok("/home/browser"));
        assert_eq!(backing::get("HOME".as_ref()), Some("/home/browser".into()));
    }

    #[test]
    fn reports_an_absent_variable_the_way_std_does() {
        backing::replace([]);
        assert!(matches!(backing::lookup("NOPE".as_ref()), Err(VarError::NotPresent)));
        assert_eq!(backing::get("NOPE".as_ref()), None);
    }

    #[test]
    fn replace_drops_what_came_before() {
        backing::replace([("A".into(), "1".into())]);
        backing::replace([("B".into(), "2".into())]);
        assert_eq!(backing::get("A".as_ref()), None);
        assert_eq!(backing::get("B".as_ref()), Some("2".into()));
    }

    #[test]
    fn set_and_unset_move_one_key_at_a_time() {
        backing::replace([]);
        backing::set("UV_CACHE_DIR".into(), "/cache".into());
        backing::set("VIRTUAL_ENV".into(), "/work/.venv".into());
        assert_eq!(backing::snapshot().len(), 2);
        backing::unset("VIRTUAL_ENV".as_ref());
        assert_eq!(backing::get("VIRTUAL_ENV".as_ref()), None);
        assert_eq!(backing::get("UV_CACHE_DIR".as_ref()), Some("/cache".into()));
    }

    #[test]
    fn snapshot_is_ordered_so_the_host_sees_a_stable_list() {
        backing::replace([]);
        backing::set("B".into(), "2".into());
        backing::set("A".into(), "1".into());
        let keys: Vec<_> = backing::snapshot().into_iter().map(|(key, _)| key).collect();
        assert_eq!(keys, vec!["A", "B"]);
    }
}
