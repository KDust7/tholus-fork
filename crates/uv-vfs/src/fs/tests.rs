use std::io::{Read, Seek, SeekFrom, Write};
use std::path::Path;
use std::sync::Arc;

use crate::fs::vfs_backed::{
    File, OpenOptions, canonicalize, copy, create_dir_all, hard_link, metadata, os, read,
    read_dir, read_link, read_to_string, remove_dir_all, remove_file, rename, symlink_metadata,
    write,
};
use crate::{MemoryFs, install_global};

fn fresh() {
    install_global(Arc::new(MemoryFs::new()));
    create_dir_all(Path::new("/work")).expect("create root");
}

#[test]
fn writes_and_reads_bytes() {
    fresh();
    write("/work/a.txt", b"hello").expect("write");
    assert_eq!(read("/work/a.txt").expect("read"), b"hello");
}

#[test]
fn reads_text() {
    fresh();
    write("/work/a.txt", "hello").expect("write");
    assert_eq!(read_to_string("/work/a.txt").expect("read"), "hello");
}

#[test]
fn copies_a_file_and_reports_the_size() {
    fresh();
    write("/work/a.txt", b"hello").expect("write");
    assert_eq!(copy("/work/a.txt", "/work/b.txt").expect("copy"), 5);
    assert_eq!(read("/work/b.txt").expect("read"), b"hello");
}

#[test]
fn renames_a_file() {
    fresh();
    write("/work/a.txt", b"hello").expect("write");
    rename("/work/a.txt", "/work/b.txt").expect("rename");
    assert!(read("/work/a.txt").is_err());
    assert_eq!(read("/work/b.txt").expect("read"), b"hello");
}

#[test]
fn removes_a_file() {
    fresh();
    write("/work/a.txt", b"hello").expect("write");
    remove_file("/work/a.txt").expect("remove");
    assert!(read("/work/a.txt").is_err());
}

#[test]
fn removes_a_tree() {
    fresh();
    create_dir_all("/work/nested/deep").expect("create");
    write("/work/nested/deep/a.txt", b"x").expect("write");
    remove_dir_all("/work/nested").expect("remove");
    assert!(metadata("/work/nested").is_err());
}

#[test]
fn reports_metadata() {
    fresh();
    write("/work/a.txt", b"hello").expect("write");
    let info = metadata("/work/a.txt").expect("metadata");
    assert!(info.is_file());
    assert!(!info.is_dir());
    assert_eq!(info.len(), 5);
    assert!(!info.is_empty());
    assert!(info.modified().is_ok());
    assert!(info.file_type().is_file());
    assert!(!info.permissions().readonly());
}

#[test]
fn an_empty_file_reports_itself_as_empty() {
    fresh();
    write("/work/a.txt", b"").expect("write");
    assert!(metadata("/work/a.txt").expect("metadata").is_empty());
}

#[test]
fn lists_a_directory() {
    fresh();
    write("/work/a.txt", b"x").expect("write");
    write("/work/b.txt", b"y").expect("write");

    let mut names: Vec<String> = read_dir("/work")
        .expect("read_dir")
        .map(|entry| {
            let entry = entry.expect("entry");
            entry.file_name().to_string_lossy().into_owned()
        })
        .collect();
    names.sort();
    assert_eq!(names, vec!["a.txt", "b.txt"]);
}

#[test]
fn directory_entries_carry_their_path_and_kind() {
    fresh();
    write("/work/a.txt", b"x").expect("write");
    let entry = read_dir("/work").expect("read_dir").next().expect("one entry").expect("ok");
    assert_eq!(entry.path(), Path::new("/work/a.txt"));
    assert!(entry.file_type().expect("file type").is_file());
    assert_eq!(entry.metadata().expect("metadata").len(), 1);
}

#[test]
fn symlinks_round_trip() {
    fresh();
    write("/work/a.txt", b"hello").expect("write");
    os::unix::fs::symlink("/work/a.txt", "/work/link").expect("symlink");

    assert_eq!(read("/work/link").expect("read"), b"hello");
    assert_eq!(read_link("/work/link").expect("read_link"), Path::new("/work/a.txt"));
    assert!(symlink_metadata("/work/link").expect("metadata").is_symlink());
}

#[test]
fn canonicalize_normalises_an_existing_path() {
    fresh();
    write("/work/a.txt", b"x").expect("write");
    assert_eq!(canonicalize("/work/./a.txt").expect("canonicalize"), Path::new("/work/a.txt"));
}

#[test]
fn canonicalize_rejects_a_missing_path() {
    fresh();
    assert!(canonicalize("/work/missing").is_err());
}

#[test]
fn hard_links_are_reported_as_unsupported() {
    fresh();
    write("/work/a.txt", b"x").expect("write");
    let error = hard_link("/work/a.txt", "/work/b.txt").expect_err("should fail");
    assert_eq!(error.kind(), std::io::ErrorKind::Unsupported);
}

#[test]
fn creating_a_file_truncates_an_existing_one() {
    fresh();
    write("/work/a.txt", b"original").expect("write");
    {
        let mut file = File::create("/work/a.txt").expect("create");
        file.write_all(b"new").expect("write");
    }
    assert_eq!(read("/work/a.txt").expect("read"), b"new");
}

#[test]
fn opening_a_missing_file_for_reading_fails() {
    fresh();
    assert!(File::open("/work/missing").is_err());
}

#[test]
fn a_file_reads_back_through_the_reader() {
    fresh();
    write("/work/a.txt", b"hello world").expect("write");
    let mut file = File::open("/work/a.txt").expect("open");
    let mut text = String::new();
    file.read_to_string(&mut text).expect("read");
    assert_eq!(text, "hello world");
}

#[test]
fn seeking_moves_the_cursor() {
    fresh();
    write("/work/a.txt", b"hello world").expect("write");
    let mut file = File::open("/work/a.txt").expect("open");
    file.seek(SeekFrom::Start(6)).expect("seek");
    let mut text = String::new();
    file.read_to_string(&mut text).expect("read");
    assert_eq!(text, "world");
}

#[test]
fn seeking_from_the_end_works() {
    fresh();
    write("/work/a.txt", b"hello").expect("write");
    let mut file = File::open("/work/a.txt").expect("open");
    assert_eq!(file.seek(SeekFrom::End(-2)).expect("seek"), 3);
}

#[test]
fn seeking_before_the_start_fails() {
    fresh();
    write("/work/a.txt", b"hello").expect("write");
    let mut file = File::open("/work/a.txt").expect("open");
    assert!(file.seek(SeekFrom::Current(-10)).is_err());
}

#[test]
fn writing_to_a_read_only_handle_fails() {
    fresh();
    write("/work/a.txt", b"hello").expect("write");
    let mut file = File::open("/work/a.txt").expect("open");
    assert!(file.write_all(b"x").is_err());
}

#[test]
fn append_mode_starts_at_the_end() {
    fresh();
    write("/work/a.txt", b"hello").expect("write");
    {
        let mut file =
            OpenOptions::new().append(true).open("/work/a.txt").expect("open for append");
        file.write_all(b" world").expect("append");
    }
    assert_eq!(read_to_string("/work/a.txt").expect("read"), "hello world");
}

#[test]
fn create_new_refuses_an_existing_file() {
    fresh();
    write("/work/a.txt", b"hello").expect("write");
    let error = OpenOptions::new().create_new(true).open("/work/a.txt").expect_err("should fail");
    assert_eq!(error.kind(), std::io::ErrorKind::AlreadyExists);
}

#[test]
fn truncating_discards_previous_contents() {
    fresh();
    write("/work/a.txt", b"hello").expect("write");
    {
        let _file =
            OpenOptions::new().write(true).truncate(true).open("/work/a.txt").expect("open");
    }
    assert_eq!(read("/work/a.txt").expect("read"), b"");
}

#[test]
fn set_len_resizes_the_file() {
    fresh();
    write("/work/a.txt", b"hello").expect("write");
    let mut file = OpenOptions::new().write(true).open("/work/a.txt").expect("open");
    file.set_len(2).expect("set_len");
    drop(file);
    assert_eq!(read("/work/a.txt").expect("read"), b"he");
}

#[test]
fn a_file_knows_its_own_path_and_metadata() {
    fresh();
    write("/work/a.txt", b"hello").expect("write");
    let file = File::open("/work/a.txt").expect("open");
    assert_eq!(file.path(), Path::new("/work/a.txt"));
    assert_eq!(file.metadata().expect("metadata").len(), 5);
}

#[test]
fn dropping_a_writer_flushes_it() {
    fresh();
    {
        let mut file = File::create("/work/a.txt").expect("create");
        file.write_all(b"flushed").expect("write");
    }
    assert_eq!(read("/work/a.txt").expect("read"), b"flushed");
}

#[test]
fn syncing_persists_without_dropping() {
    fresh();
    let mut file = File::create("/work/a.txt").expect("create");
    file.write_all(b"synced").expect("write");
    file.sync_all().expect("sync");
    assert_eq!(read("/work/a.txt").expect("read"), b"synced");
}

#[tokio::test]
async fn the_async_surface_mirrors_the_sync_one() {
    fresh();
    crate::fs::vfs_backed::tokio::write("/work/a.txt", b"async").await.expect("write");
    assert_eq!(
        crate::fs::vfs_backed::tokio::read_to_string("/work/a.txt").await.expect("read"),
        "async"
    );
    assert_eq!(crate::fs::vfs_backed::tokio::read("/work/a.txt").await.expect("read"), b"async");
    assert!(crate::fs::vfs_backed::tokio::metadata("/work/a.txt").await.is_ok());
    crate::fs::vfs_backed::tokio::rename("/work/a.txt", "/work/b.txt").await.expect("rename");
    assert!(crate::fs::vfs_backed::tokio::canonicalize("/work/b.txt").await.is_ok());
    crate::fs::vfs_backed::tokio::remove_file("/work/b.txt").await.expect("remove");
    crate::fs::vfs_backed::tokio::create_dir_all("/work/nested").await.expect("create");
    crate::fs::vfs_backed::tokio::remove_dir_all("/work/nested").await.expect("remove");
}

#[test]
fn a_files_permissions_report_a_posix_mode() {
    fresh();
    write("/work/a.txt", b"hello").expect("write");
    assert_eq!(metadata("/work/a.txt").expect("metadata").permissions().mode(), 0o644);
}

#[test]
fn a_directorys_permissions_report_a_traversable_mode() {
    fresh();
    assert_eq!(metadata("/work").expect("metadata").permissions().mode(), 0o755);
}

#[test]
fn setting_a_mode_is_remembered_by_the_handle() {
    fresh();
    let mut permissions = metadata("/work").expect("metadata").permissions();
    permissions.set_mode(0o700);
    assert_eq!(permissions.mode(), 0o700);
}

#[test]
fn an_exclusive_lock_is_available_on_an_unlocked_file() {
    fresh();
    let file = File::create("/work/a.txt").expect("create");
    assert!(file.try_lock().is_ok());
}

#[test]
fn an_exclusive_lock_excludes_a_second_handle() {
    fresh();
    let held = File::create("/work/a.txt").expect("create");
    held.try_lock().expect("first lock");

    let other = File::open("/work/a.txt").expect("open");
    assert!(matches!(other.try_lock(), Err(std::fs::TryLockError::WouldBlock)));
}

#[test]
fn shared_locks_coexist_across_handles() {
    fresh();
    let first = File::create("/work/a.txt").expect("create");
    first.try_lock_shared().expect("first lock");

    let second = File::open("/work/a.txt").expect("open");
    assert!(second.try_lock_shared().is_ok());
}

#[test]
fn a_shared_lock_excludes_an_exclusive_lock() {
    fresh();
    let reader = File::create("/work/a.txt").expect("create");
    reader.try_lock_shared().expect("shared lock");

    let writer = File::open("/work/a.txt").expect("open");
    assert!(matches!(writer.try_lock(), Err(std::fs::TryLockError::WouldBlock)));
}

#[test]
fn an_exclusive_lock_excludes_a_shared_lock() {
    fresh();
    let writer = File::create("/work/a.txt").expect("create");
    writer.try_lock().expect("exclusive lock");

    let reader = File::open("/work/a.txt").expect("open");
    assert!(matches!(reader.try_lock_shared(), Err(std::fs::TryLockError::WouldBlock)));
}

#[test]
fn unlocking_releases_the_lock_for_another_handle() {
    fresh();
    let held = File::create("/work/a.txt").expect("create");
    held.try_lock().expect("first lock");
    held.unlock().expect("unlock");

    let other = File::open("/work/a.txt").expect("open");
    assert!(other.try_lock().is_ok());
}

#[test]
fn dropping_a_handle_releases_its_lock() {
    fresh();
    let held = File::create("/work/a.txt").expect("create");
    held.try_lock().expect("first lock");
    drop(held);

    let other = File::open("/work/a.txt").expect("open");
    assert!(other.try_lock().is_ok());
}

#[test]
fn releasing_one_shared_holder_keeps_the_lock_for_the_rest() {
    fresh();
    let first = File::create("/work/a.txt").expect("create");
    first.try_lock_shared().expect("first lock");
    let second = File::open("/work/a.txt").expect("open");
    second.try_lock_shared().expect("second lock");
    drop(first);

    let writer = File::open("/work/a.txt").expect("open");
    assert!(matches!(writer.try_lock(), Err(std::fs::TryLockError::WouldBlock)));
}

#[test]
fn locks_on_different_paths_do_not_contend() {
    fresh();
    let first = File::create("/work/a.txt").expect("create");
    first.try_lock().expect("first lock");

    let second = File::create("/work/b.txt").expect("create");
    assert!(second.try_lock().is_ok());
}

#[test]
fn relocking_the_same_handle_converts_the_lock() {
    fresh();
    let file = File::create("/work/a.txt").expect("create");
    file.try_lock_shared().expect("shared lock");
    file.try_lock().expect("conversion to exclusive");

    let other = File::open("/work/a.txt").expect("open");
    assert!(matches!(other.try_lock_shared(), Err(std::fs::TryLockError::WouldBlock)));
}

#[test]
fn unlocking_an_unlocked_handle_succeeds() {
    fresh();
    let file = File::create("/work/a.txt").expect("create");
    assert!(file.unlock().is_ok());
}

#[test]
fn a_blocking_lock_succeeds_when_uncontended() {
    fresh();
    let file = File::create("/work/a.txt").expect("create");
    assert!(file.lock().is_ok());
    file.unlock().expect("unlock");
    assert!(file.lock_shared().is_ok());
}

#[test]
fn a_blocking_lock_reports_contention_rather_than_hanging() {
    fresh();
    let held = File::create("/work/a.txt").expect("create");
    held.try_lock().expect("first lock");

    let other = File::open("/work/a.txt").expect("open");
    let error = other.lock().expect_err("the second lock cannot be waited for");
    assert_eq!(error.kind(), std::io::ErrorKind::WouldBlock);
}
