use std::path::{Component, Path, PathBuf};

pub fn normalize(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::from("/");
    for component in path.components() {
        match component {
            Component::Prefix(_) | Component::RootDir => {}
            Component::CurDir => {}
            Component::ParentDir => {
                normalized.pop();
            }
            Component::Normal(segment) => normalized.push(segment),
        }
    }
    normalized
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
    use super::{is_within, normalize, parent_of};
    use std::path::{Path, PathBuf};

    #[test]
    fn makes_relative_paths_absolute() {
        assert_eq!(normalize(Path::new("work/project")), PathBuf::from("/work/project"));
    }

    #[test]
    fn collapses_current_directory_segments() {
        assert_eq!(normalize(Path::new("/work/./project")), PathBuf::from("/work/project"));
    }

    #[test]
    fn resolves_parent_segments() {
        assert_eq!(normalize(Path::new("/work/project/../other")), PathBuf::from("/work/other"));
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
        assert_eq!(parent_of(Path::new("/work/project")), Some(PathBuf::from("/work")));
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
}
