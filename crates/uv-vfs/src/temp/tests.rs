use std::io::{Read, Seek, SeekFrom, Write};
use std::path::Path;
use std::sync::Arc;

use crate::temp::vfs_backed::{
    Builder, NamedTempFile, TEMP_ROOT, TempDir, tempdir, tempdir_in, tempfile, tempfile_in,
};
use crate::{MemoryFs, Vfs, install_global};

fn fresh() -> Arc<MemoryFs> {
    let fs = Arc::new(MemoryFs::new());
    install_global(Arc::clone(&fs) as Arc<dyn Vfs>);
    fs
}

#[test]
fn a_temporary_directory_lives_under_the_temp_root() {
    fresh();
    let dir = tempdir().expect("tempdir");
    assert!(dir.path().starts_with(TEMP_ROOT));
}

#[test]
fn a_temporary_directory_exists_while_held() {
    let fs = fresh();
    let dir = tempdir().expect("tempdir");
    assert!(fs.exists(dir.path()));
}

#[test]
fn a_temporary_directory_is_removed_on_drop() {
    let fs = fresh();
    let path = {
        let dir = tempdir().expect("tempdir");
        dir.path().to_path_buf()
    };
    assert!(!fs.exists(&path));
}

#[test]
fn keeping_a_directory_survives_the_drop() {
    let fs = fresh();
    let path = tempdir().expect("tempdir").keep();
    assert!(fs.exists(&path));
}

#[test]
fn closing_a_directory_removes_it() {
    let fs = fresh();
    let dir = tempdir().expect("tempdir");
    let path = dir.path().to_path_buf();
    dir.close().expect("close");
    assert!(!fs.exists(&path));
}

#[test]
fn temporary_directories_do_not_collide() {
    fresh();
    let first = tempdir().expect("first");
    let second = tempdir().expect("second");
    assert_ne!(first.path(), second.path());
}

#[test]
fn a_directory_can_be_created_somewhere_specific() {
    let fs = fresh();
    fs.create_dir_all(Path::new("/work")).expect("create");
    let dir = tempdir_in("/work").expect("tempdir_in");
    assert!(dir.path().starts_with("/work"));
}

#[test]
fn a_directory_honours_a_prefix_and_suffix() {
    fresh();
    let dir = Builder::new().prefix("build-").suffix(".d").tempdir().expect("tempdir");
    let name = dir.path().file_name().expect("name").to_string_lossy().into_owned();
    assert!(name.starts_with("build-"));
    assert!(name.ends_with(".d"));
}

#[test]
fn a_directory_reference_borrows_its_path() {
    fresh();
    let dir = tempdir().expect("tempdir");
    let borrowed: &Path = dir.as_ref();
    assert_eq!(borrowed, dir.path());
}

#[test]
fn a_named_file_is_writable_and_readable() {
    fresh();
    let mut file = NamedTempFile::new().expect("tempfile");
    file.write_all(b"hello").expect("write");
    file.flush().expect("flush");
    file.seek(SeekFrom::Start(0)).expect("seek");

    let mut text = String::new();
    file.read_to_string(&mut text).expect("read");
    assert_eq!(text, "hello");
}

#[test]
fn a_named_file_is_removed_on_drop() {
    let fs = fresh();
    let path = {
        let file = NamedTempFile::new().expect("tempfile");
        file.path().to_path_buf()
    };
    assert!(!fs.exists(&path));
}

#[test]
fn persisting_moves_the_file_to_its_destination() {
    let fs = fresh();
    fs.create_dir_all(Path::new("/work")).expect("create");

    let mut file = NamedTempFile::new().expect("tempfile");
    file.write_all(b"payload").expect("write");
    let temp_path = file.path().to_path_buf();
    file.persist("/work/final.txt").expect("persist");

    assert!(!fs.exists(&temp_path));
    assert_eq!(fs.read(Path::new("/work/final.txt")).expect("read"), b"payload");
}

#[test]
fn a_temp_path_removes_its_file_on_drop() {
    let fs = fresh();
    let path = {
        let handle = NamedTempFile::new().expect("tempfile").into_temp_path();
        handle.to_path_buf()
    };
    assert!(!fs.exists(&path));
}

#[test]
fn keeping_a_temp_path_survives_the_drop() {
    let fs = fresh();
    let handle = NamedTempFile::new().expect("tempfile").into_temp_path();
    let path = handle.keep().expect("keep");
    assert!(fs.exists(&path));
}

#[test]
fn keeping_a_named_file_returns_its_handle_and_path() {
    let fs = fresh();
    let file = NamedTempFile::new().expect("tempfile");
    let (_handle, path) = file.keep().expect("keep");
    assert!(fs.exists(&path));
}

#[test]
fn a_named_file_exposes_its_underlying_handle() {
    fresh();
    let mut file = NamedTempFile::new().expect("tempfile");
    file.as_file_mut().write_all(b"x").expect("write");
    assert_eq!(file.as_file().path(), file.path());
}

#[test]
fn an_anonymous_file_can_be_created() {
    fresh();
    let mut file = tempfile().expect("tempfile");
    file.write_all(b"anonymous").expect("write");
    file.flush().expect("flush");
}

#[test]
fn an_anonymous_file_can_be_placed_somewhere_specific() {
    let fs = fresh();
    fs.create_dir_all(Path::new("/work")).expect("create");
    let file = tempfile_in("/work").expect("tempfile_in");
    assert!(file.path().starts_with("/work"));
}

#[test]
fn the_builder_can_place_a_file_somewhere_specific() {
    let fs = fresh();
    fs.create_dir_all(Path::new("/work")).expect("create");
    let file = Builder::new().prefix("wheel-").tempfile_in("/work").expect("tempfile_in");
    let name = file.path().file_name().expect("name").to_string_lossy().into_owned();
    assert!(name.starts_with("wheel-"));
}

#[test]
fn a_directory_can_be_constructed_directly() {
    let fs = fresh();
    let dir = TempDir::new().expect("new");
    assert!(fs.exists(dir.path()));
}

#[test]
fn a_named_temporary_file_borrows_as_its_path() {
    let fs = fresh();
    fs.create_dir_all(Path::new("/work")).expect("create");
    let file = Builder::new().tempfile_in("/work").expect("tempfile_in");
    let borrowed: &Path = file.as_ref();
    assert_eq!(borrowed, file.path());
}

#[test]
fn a_named_temporary_file_can_be_passed_to_path_taking_apis() {
    let fs = fresh();
    fs.create_dir_all(Path::new("/work")).expect("create");
    let file = Builder::new().tempfile_in("/work").expect("tempfile_in");
    crate::fs::vfs_backed::write(&file, b"payload").expect("write");
    assert_eq!(crate::fs::vfs_backed::read(file.path()).expect("read"), b"payload");
}
