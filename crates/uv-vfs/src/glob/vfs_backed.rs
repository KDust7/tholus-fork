use std::path::{Component, Path, PathBuf};

pub use glob::{GlobError, MatchOptions, Pattern, PatternError};

use crate::path::normalize;
use crate::walk::vfs_backed::WalkDir;

const WILDCARDS: [char; 3] = ['*', '?', '['];

pub struct Paths {
    matches: std::vec::IntoIter<PathBuf>,
}

impl Iterator for Paths {
    type Item = Result<PathBuf, GlobError>;

    fn next(&mut self) -> Option<Self::Item> {
        self.matches.next().map(Ok)
    }
}

pub fn glob(pattern: &str) -> Result<Paths, PatternError> {
    let (body, directories_only) = match pattern.strip_suffix('/') {
        Some(body) => (body, true),
        None => (pattern, false),
    };
    let compiled = Pattern::new(body)?;
    let options = MatchOptions {
        case_sensitive: true,
        require_literal_separator: true,
        require_literal_leading_dot: false,
    };

    let mut matches: Vec<PathBuf> = WalkDir::new(literal_root(body))
        .into_iter()
        .filter_map(Result::ok)
        .filter(|entry| !directories_only || entry.file_type().is_dir())
        .map(super::super::walk::vfs_backed::DirEntry::into_path)
        .filter(|path| compiled.matches_path_with(path, options))
        .collect();
    matches.sort();
    matches.dedup();

    Ok(Paths {
        matches: matches.into_iter(),
    })
}

fn literal_root(pattern: &str) -> PathBuf {
    let normalized = normalize(Path::new(pattern));
    let mut root = PathBuf::from("/");
    for component in normalized.components() {
        let Component::Normal(part) = component else {
            continue;
        };
        if part.to_string_lossy().contains(WILDCARDS) {
            break;
        }
        root.push(part);
    }
    root
}
