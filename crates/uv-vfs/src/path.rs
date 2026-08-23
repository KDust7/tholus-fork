use std::ffi::OsStr;
use std::path::{Component, Path, PathBuf};

thread_local! {
    static WORKING_DIRECTORY: std::cell::RefCell<PathBuf> =
        std::cell::RefCell::new(PathBuf::from("/"));
}

pub fn working_directory() -> PathBuf {
    WORKING_DIRECTORY.with(|current| current.borrow().clone())
}

pub fn set_working_directory(path: &Path) {
    let resolved = resolve(path);
    WORKING_DIRECTORY.with(|current| {
        *current.borrow_mut() = resolved;
    });
}

pub fn resolve(path: &Path) -> PathBuf {
    normalize(&working_directory().join(path))
}

#[cfg(target_family = "wasm")]
pub fn current_dir() -> std::io::Result<PathBuf> {
    Ok(working_directory())
}

#[cfg(not(target_family = "wasm"))]
pub fn current_dir() -> std::io::Result<PathBuf> {
    std::env::current_dir()
}

#[cfg(target_family = "wasm")]
pub fn set_current_dir(path: impl AsRef<Path>) -> std::io::Result<()> {
    set_working_directory(path.as_ref());
    Ok(())
}

#[cfg(not(target_family = "wasm"))]
pub fn set_current_dir(path: impl AsRef<Path>) -> std::io::Result<()> {
    std::env::set_current_dir(path)
}

#[cfg(target_family = "wasm")]
pub fn absolute(path: impl AsRef<Path>) -> std::io::Result<PathBuf> {
    Ok(normalize(&working_directory().join(path.as_ref())))
}

#[cfg(not(target_family = "wasm"))]
pub fn absolute(path: impl AsRef<Path>) -> std::io::Result<PathBuf> {
    std::path::absolute(path)
}

pub const PATH_SEPARATOR: char = ':';

pub fn split_posix_path_list(unparsed: &OsStr) -> Vec<PathBuf> {
    unparsed
        .to_string_lossy()
        .split(PATH_SEPARATOR)
        .map(PathBuf::from)
        .collect()
}

#[cfg(target_family = "wasm")]
pub const EXE_SUFFIX: &str = "";

#[cfg(not(target_family = "wasm"))]
pub const EXE_SUFFIX: &str = std::env::consts::EXE_SUFFIX;

#[cfg(target_family = "wasm")]
pub const EXE_EXTENSION: &str = "";

#[cfg(not(target_family = "wasm"))]
pub const EXE_EXTENSION: &str = std::env::consts::EXE_EXTENSION;

#[cfg(target_family = "wasm")]
pub fn temp_dir() -> PathBuf {
    normalize(Path::new(crate::temp::vfs_backed::TEMP_ROOT))
}

#[cfg(not(target_family = "wasm"))]
pub fn temp_dir() -> PathBuf {
    std::env::temp_dir()
}

#[cfg(target_family = "wasm")]
pub fn home_dir() -> Option<PathBuf> {
    crate::env::var_os("HOME").map(PathBuf::from)
}

#[cfg(not(target_family = "wasm"))]
pub fn home_dir() -> Option<PathBuf> {
    std::env::home_dir()
}

#[cfg(target_family = "wasm")]
pub fn split_paths<T: AsRef<OsStr> + ?Sized>(unparsed: &T) -> std::vec::IntoIter<PathBuf> {
    split_posix_path_list(unparsed.as_ref()).into_iter()
}

#[cfg(not(target_family = "wasm"))]
pub fn split_paths<T: AsRef<OsStr> + ?Sized>(unparsed: &T) -> std::env::SplitPaths<'_> {
    std::env::split_paths(unparsed)
}

pub fn normalize(path: &Path) -> PathBuf {
    let mut segments: Vec<&OsStr> = Vec::new();
    for component in path.components() {
        match component {
            Component::Prefix(_) | Component::RootDir | Component::CurDir => {}
            Component::ParentDir => {
                segments.pop();
            }
            Component::Normal(segment) => segments.push(segment),
        }
    }

    let mut normalized = String::from("/");
    for (index, segment) in segments.iter().enumerate() {
        if index > 0 {
            normalized.push('/');
        }
        normalized.push_str(&segment.to_string_lossy());
    }
    PathBuf::from(normalized)
}

pub fn parent_of(path: &Path) -> Option<PathBuf> {
    let normalized = normalize(path);
    if normalized == Path::new("/") {
        return None;
    }
    normalized.parent().map(Path::to_path_buf)
}

pub fn is_within(ancestor: &Path, descendant: &Path) -> bool {
    let ancestor = normalize(ancestor);
    let descendant = normalize(descendant);
    descendant != ancestor && descendant.starts_with(&ancestor)
}

#[cfg(test)]
mod tests {
    use super::{
        is_within, normalize, parent_of, resolve, set_working_directory, split_posix_path_list,
        working_directory,
    };
    use std::ffi::OsStr;
    use std::path::{Path, PathBuf};

    #[test]
    fn splits_a_path_list_on_colons() {
        assert_eq!(
            split_posix_path_list(OsStr::new("/usr/bin:/work/.venv/bin")),
            vec![PathBuf::from("/usr/bin"), PathBuf::from("/work/.venv/bin")]
        );
    }

    #[test]
    fn an_empty_path_list_is_one_empty_entry() {
        assert_eq!(split_posix_path_list(OsStr::new("")), vec![PathBuf::new()]);
    }

    #[test]
    fn keeps_the_empty_entry_a_trailing_separator_leaves() {
        assert_eq!(
            split_posix_path_list(OsStr::new("/usr/bin:")),
            vec![PathBuf::from("/usr/bin"), PathBuf::new()]
        );
    }

    #[test]
    fn does_not_split_a_windows_path_list() {
        assert_eq!(
            split_posix_path_list(OsStr::new("/usr/bin;/work/bin")),
            vec![PathBuf::from("/usr/bin;/work/bin")]
        );
    }

    #[test]
    fn makes_relative_paths_absolute() {
        assert_eq!(
            normalize(Path::new("work/project")),
            PathBuf::from("/work/project")
        );
    }

    #[test]
    fn joins_with_posix_separators_on_every_host() {
        assert_eq!(
            normalize(Path::new("/work/a/b")).display().to_string(),
            "/work/a/b"
        );
    }

    #[test]
    fn rewrites_a_host_separator_into_a_posix_one() {
        assert_eq!(
            normalize(Path::new("/work")).join("a"),
            PathBuf::from("/work/a")
        );
        assert_eq!(
            normalize(&Path::new("/work").join("a"))
                .display()
                .to_string(),
            "/work/a"
        );
    }

    #[test]
    fn collapses_current_directory_segments() {
        assert_eq!(
            normalize(Path::new("/work/./project")),
            PathBuf::from("/work/project")
        );
    }

    #[test]
    fn resolves_parent_segments() {
        assert_eq!(
            normalize(Path::new("/work/project/../other")),
            PathBuf::from("/work/other")
        );
    }

    #[test]
    fn cannot_escape_the_root() {
        assert_eq!(normalize(Path::new("/../../etc")), PathBuf::from("/etc"));
    }

    #[test]
    fn root_has_no_parent() {
        assert_eq!(parent_of(Path::new("/")), None);
    }

    #[test]
    fn reports_the_containing_directory() {
        assert_eq!(
            parent_of(Path::new("/work/project")),
            Some(PathBuf::from("/work"))
        );
    }

    #[test]
    fn recognises_descendants() {
        assert!(is_within(Path::new("/work"), Path::new("/work/project")));
    }

    #[test]
    fn a_directory_does_not_contain_itself() {
        assert!(!is_within(Path::new("/work"), Path::new("/work")));
    }

    #[test]
    fn rejects_unrelated_paths() {
        assert!(!is_within(Path::new("/work"), Path::new("/cache")));
    }

    #[test]
    fn a_relative_path_resolves_against_the_working_directory() {
        set_working_directory(Path::new("/work"));
        assert_eq!(
            resolve(Path::new("requirements.in")),
            PathBuf::from("/work/requirements.in")
        );
        assert_eq!(
            resolve(Path::new("./sub/file")),
            PathBuf::from("/work/sub/file")
        );
        set_working_directory(Path::new("/"));
    }

    #[test]
    fn an_absolute_path_ignores_the_working_directory() {
        set_working_directory(Path::new("/work"));
        assert_eq!(
            resolve(Path::new("/etc/hosts")),
            PathBuf::from("/etc/hosts")
        );
        set_working_directory(Path::new("/"));
    }

    #[test]
    fn the_root_working_directory_leaves_resolution_as_normalization() {
        set_working_directory(Path::new("/"));
        for path in ["requirements.in", "/abs/path", "./rel", "a/../b"] {
            assert_eq!(resolve(Path::new(path)), normalize(Path::new(path)));
        }
    }

    #[test]
    fn changing_directory_is_itself_relative_to_the_current_one() {
        set_working_directory(Path::new("/"));
        set_working_directory(Path::new("work"));
        assert_eq!(working_directory(), PathBuf::from("/work"));
        set_working_directory(Path::new("sub"));
        assert_eq!(working_directory(), PathBuf::from("/work/sub"));
        set_working_directory(Path::new(".."));
        assert_eq!(working_directory(), PathBuf::from("/work"));
        set_working_directory(Path::new("/"));
    }
}
