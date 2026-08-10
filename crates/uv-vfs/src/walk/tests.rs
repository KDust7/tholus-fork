use std::path::Path;
use std::sync::Arc;

use crate::fs::vfs_backed::{create_dir_all, os, write};
use crate::walk::vfs_backed::{DirEntry, Error, WalkDir};
use crate::{MemoryFs, install_global};

fn fresh() {
    install_global(Arc::new(MemoryFs::new()));
    create_dir_all(Path::new("/work")).expect("create root");
}

fn tree() {
    fresh();
    create_dir_all("/work/a/inner").expect("create a/inner");
    create_dir_all("/work/b").expect("create b");
    write("/work/top.txt", b"top").expect("write top");
    write("/work/a/one.txt", b"one").expect("write one");
    write("/work/a/inner/deep.txt", b"deep").expect("write deep");
    write("/work/b/two.txt", b"two").expect("write two");
}

fn walked(walk: WalkDir) -> Vec<String> {
    walk.into_iter()
        .map(|entry| entry.expect("entry").path().display().to_string())
        .collect()
}

#[test]
fn a_walk_yields_the_root_first() {
    tree();
    let seen = walked(WalkDir::new("/work").sort_by_file_name());
    assert_eq!(seen.first().map(String::as_str), Some("/work"));
}

#[test]
fn a_walk_reaches_every_entry() {
    tree();
    let seen = walked(WalkDir::new("/work").sort_by_file_name());
    assert_eq!(
        seen,
        vec![
            "/work",
            "/work/a",
            "/work/a/inner",
            "/work/a/inner/deep.txt",
            "/work/a/one.txt",
            "/work/b",
            "/work/b/two.txt",
            "/work/top.txt",
        ]
    );
}

#[test]
fn contents_first_yields_a_directory_after_its_children() {
    tree();
    let seen = walked(WalkDir::new("/work").sort_by_file_name().contents_first(true));
    assert_eq!(
        seen,
        vec![
            "/work/a/inner/deep.txt",
            "/work/a/inner",
            "/work/a/one.txt",
            "/work/a",
            "/work/b/two.txt",
            "/work/b",
            "/work/top.txt",
            "/work",
        ]
    );
}

#[test]
fn min_depth_drops_the_root() {
    tree();
    let seen = walked(WalkDir::new("/work").sort_by_file_name().min_depth(1));
    assert!(!seen.contains(&"/work".to_owned()));
    assert!(seen.contains(&"/work/a".to_owned()));
}

#[test]
fn min_depth_drops_every_shallower_entry() {
    tree();
    let seen = walked(WalkDir::new("/work").sort_by_file_name().min_depth(2));
    assert_eq!(
        seen,
        vec!["/work/a/inner", "/work/a/inner/deep.txt", "/work/a/one.txt", "/work/b/two.txt"]
    );
}

#[test]
fn filter_entry_prunes_the_subtree_it_rejects() {
    tree();
    let seen: Vec<String> = WalkDir::new("/work")
        .sort_by_file_name()
        .into_iter()
        .filter_entry(|entry: &DirEntry| entry.file_name() != "a")
        .map(|entry| entry.expect("entry").path().display().to_string())
        .collect();
    assert_eq!(seen, vec!["/work", "/work/b", "/work/b/two.txt", "/work/top.txt"]);
}

#[test]
fn filter_entry_rejecting_the_root_yields_nothing() {
    tree();
    let seen: Vec<String> = WalkDir::new("/work")
        .into_iter()
        .filter_entry(|_: &DirEntry| false)
        .map(|entry| entry.expect("entry").path().display().to_string())
        .collect();
    assert!(seen.is_empty());
}

#[test]
fn entries_report_their_depth_and_file_name() {
    tree();
    let entry = WalkDir::new("/work")
        .sort_by_file_name()
        .into_iter()
        .filter_map(Result::ok)
        .find(|entry| entry.file_name() == "deep.txt")
        .expect("deep.txt");
    assert_eq!(entry.depth(), 3);
    assert!(entry.file_type().is_file());
    assert_eq!(entry.metadata().expect("metadata").len(), 4);
}

#[test]
fn a_symlink_is_reported_without_being_followed() {
    fresh();
    create_dir_all("/work/target").expect("create target");
    write("/work/target/inside.txt", b"x").expect("write inside");
    os::unix::fs::symlink("/work/target", "/work/link").expect("symlink");

    let entries: Vec<DirEntry> = WalkDir::new("/work")
        .sort_by_file_name()
        .into_iter()
        .filter_map(Result::ok)
        .collect();
    let link = entries.iter().find(|entry| entry.file_name() == "link").expect("link");
    assert!(link.path_is_symlink());
    assert!(!entries.iter().any(|entry| entry.path().parent() == Some(Path::new("/work/link"))));
}

#[test]
fn a_file_root_yields_only_itself() {
    fresh();
    write("/work/only.txt", b"x").expect("write");
    assert_eq!(walked(WalkDir::new("/work/only.txt")), vec!["/work/only.txt"]);
}

#[test]
fn a_missing_root_yields_one_error() {
    fresh();
    let outcomes: Vec<Result<DirEntry, Error>> = WalkDir::new("/work/missing").into_iter().collect();
    assert_eq!(outcomes.len(), 1);
    let error = outcomes.into_iter().next().expect("outcome").expect_err("should fail");
    assert_eq!(error.path(), Some(Path::new("/work/missing")));
    assert!(error.io_error().is_some());
}

#[test]
fn an_unreadable_directory_reports_the_path_it_failed_on() {
    tree();
    let error = Error::new(Path::new("/work/a"), std::io::Error::other("denied"));
    assert!(error.to_string().contains("/work/a"));
    assert!(error.to_string().contains("denied"));
}

#[test]
fn an_error_converts_back_to_io() {
    let error = Error::new(Path::new("/work"), std::io::Error::other("boom"));
    let io_error: std::io::Error = error.into();
    assert_eq!(io_error.to_string(), "boom");
}
