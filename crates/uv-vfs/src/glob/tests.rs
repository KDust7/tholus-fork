use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::Vfs;
use crate::memory::MemoryFs;

const TREE: &[&str] = &[
    "packages/alpha/pyproject.toml",
    "packages/alpha/src/alpha/__init__.py",
    "packages/beta/pyproject.toml",
    "packages/beta/src/beta/__init__.py",
    "packages/.hidden/pyproject.toml",
    "vendor/gamma/pyproject.toml",
    "pyproject.toml",
    "README.md",
];

const PATTERNS: &[&str] = &[
    "packages/*",
    "packages/*/",
    "packages/*/pyproject.toml",
    "packages/**/pyproject.toml",
    "**/pyproject.toml",
    "*/pyproject.toml",
    "packages/alpha",
    "packages/alpha/pyproject.toml",
    "packages/nothing/*",
    "*.md",
    "**/*.py",
    "packages/[ab]*",
    "?ackages/*",
];

fn seed_memory(root: &Path) {
    let fs = MemoryFs::new();
    for entry in TREE {
        let path = root.join(entry);
        if let Some(parent) = path.parent() {
            fs.create_dir_all(parent).expect("create parent");
        }
        fs.write(&path, b"").expect("write file");
    }
    crate::install_global(Arc::new(fs));
}

fn seed_disk(root: &Path) {
    for entry in TREE {
        let path = root.join(entry);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).expect("create parent");
        }
        std::fs::write(&path, b"").expect("write file");
    }
}

fn relative(paths: Vec<PathBuf>, root: &Path) -> Vec<String> {
    let mut out: Vec<String> = paths
        .into_iter()
        .filter_map(|path| {
            path.strip_prefix(root)
                .ok()
                .map(|rest| rest.to_string_lossy().replace('\\', "/"))
        })
        .filter(|rest| !rest.is_empty())
        .collect();
    out.sort();
    out
}

#[test]
fn the_vfs_walker_agrees_with_the_glob_crate() {
    let disk = tempfile::tempdir().expect("tempdir");
    let disk_root = disk.path().to_path_buf();
    seed_disk(&disk_root);

    let memory_root = Path::new("/ws");
    seed_memory(memory_root);

    for pattern in PATTERNS {
        let on_disk = glob::glob(&format!(
            "{}/{pattern}",
            glob::Pattern::escape(&disk_root.to_string_lossy())
        ))
        .expect("native pattern")
        .filter_map(Result::ok)
        .collect::<Vec<_>>();

        let in_memory = super::vfs_backed::glob(&format!("/ws/{pattern}"))
            .expect("vfs pattern")
            .filter_map(Result::ok)
            .collect::<Vec<_>>();

        assert_eq!(
            relative(in_memory, memory_root),
            relative(on_disk, &disk_root),
            "pattern `{pattern}` disagreed"
        );
    }
}

#[test]
fn an_invalid_pattern_is_rejected_the_way_the_crate_rejects_it() {
    assert!(super::vfs_backed::glob("/ws/[").is_err());
    assert!(glob::glob("/ws/[").is_err());
}

#[test]
fn a_pattern_with_no_wildcards_yields_the_path_when_it_exists() {
    seed_memory(Path::new("/lit"));
    let found: Vec<_> = super::vfs_backed::glob("/lit/pyproject.toml")
        .expect("pattern")
        .filter_map(Result::ok)
        .collect();
    assert_eq!(found, vec![PathBuf::from("/lit/pyproject.toml")]);

    let missing: Vec<_> = super::vfs_backed::glob("/lit/absent.toml")
        .expect("pattern")
        .filter_map(Result::ok)
        .collect();
    assert!(missing.is_empty());
}
