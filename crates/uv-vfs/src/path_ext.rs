use std::io;
use std::path::{Path, PathBuf};

use crate::fs::Metadata;

#[cfg(not(target_family = "wasm"))]
pub type VfsReadDir = std::fs::ReadDir;

#[cfg(target_family = "wasm")]
pub type VfsReadDir = crate::fs::ReadDir;

pub trait VfsPathExt {
    fn vfs_exists(&self) -> bool;

    fn vfs_try_exists(&self) -> io::Result<bool>;

    fn vfs_is_file(&self) -> bool;

    fn vfs_is_dir(&self) -> bool;

    fn vfs_metadata(&self) -> io::Result<Metadata>;

    fn vfs_symlink_metadata(&self) -> io::Result<Metadata>;

    fn vfs_canonicalize(&self) -> io::Result<PathBuf>;

    fn vfs_read_link(&self) -> io::Result<PathBuf>;

    fn vfs_read_dir(&self) -> io::Result<VfsReadDir>;

    fn vfs_is_absolute(&self) -> bool;

    fn vfs_is_relative(&self) -> bool;
}

#[cfg(not(target_family = "wasm"))]
impl VfsPathExt for Path {
    fn vfs_exists(&self) -> bool {
        self.exists()
    }

    fn vfs_try_exists(&self) -> io::Result<bool> {
        self.try_exists()
    }

    fn vfs_is_file(&self) -> bool {
        self.is_file()
    }

    fn vfs_is_dir(&self) -> bool {
        self.is_dir()
    }

    fn vfs_metadata(&self) -> io::Result<Metadata> {
        self.metadata()
    }

    fn vfs_symlink_metadata(&self) -> io::Result<Metadata> {
        self.symlink_metadata()
    }

    fn vfs_canonicalize(&self) -> io::Result<PathBuf> {
        self.canonicalize()
    }

    fn vfs_read_link(&self) -> io::Result<PathBuf> {
        self.read_link()
    }

    fn vfs_read_dir(&self) -> io::Result<VfsReadDir> {
        self.read_dir()
    }

    fn vfs_is_absolute(&self) -> bool {
        self.is_absolute()
    }

    fn vfs_is_relative(&self) -> bool {
        self.is_relative()
    }
}

#[cfg(target_family = "wasm")]
impl VfsPathExt for Path {
    fn vfs_exists(&self) -> bool {
        crate::fs::exists(self)
    }

    fn vfs_try_exists(&self) -> io::Result<bool> {
        crate::fs::try_exists(self)
    }

    fn vfs_is_file(&self) -> bool {
        crate::fs::is_file(self)
    }

    fn vfs_is_dir(&self) -> bool {
        crate::fs::is_dir(self)
    }

    fn vfs_metadata(&self) -> io::Result<Metadata> {
        crate::fs::metadata(self)
    }

    fn vfs_symlink_metadata(&self) -> io::Result<Metadata> {
        crate::fs::symlink_metadata(self)
    }

    fn vfs_canonicalize(&self) -> io::Result<PathBuf> {
        crate::fs::canonicalize(self)
    }

    fn vfs_read_link(&self) -> io::Result<PathBuf> {
        crate::fs::read_link(self)
    }

    fn vfs_read_dir(&self) -> io::Result<VfsReadDir> {
        crate::fs::read_dir(self)
    }

    fn vfs_is_absolute(&self) -> bool {
        self.has_root()
    }

    fn vfs_is_relative(&self) -> bool {
        !self.has_root()
    }
}

#[cfg(all(test, not(target_family = "wasm")))]
mod tests {
    use super::VfsPathExt;
    use std::path::PathBuf;

    fn sandbox() -> tempfile::TempDir {
        tempfile::tempdir().expect("tempdir")
    }

    #[test]
    fn absoluteness_matches_the_inherent_method() {
        let dir = sandbox();
        assert_eq!(dir.path().vfs_is_absolute(), dir.path().is_absolute());
        assert_eq!(dir.path().vfs_is_relative(), dir.path().is_relative());

        let relative = PathBuf::from("a/b.txt");
        assert!(!relative.vfs_is_absolute());
        assert!(relative.vfs_is_relative());
    }

    #[test]
    fn presence_matches_the_inherent_method() {
        let dir = sandbox();
        let file = dir.path().join("a.txt");
        std::fs::write(&file, b"hello").expect("write");

        assert!(file.vfs_exists());
        assert!(dir.path().vfs_exists());
        assert!(!dir.path().join("missing").vfs_exists());
    }

    #[test]
    fn fallible_presence_matches_the_inherent_method() {
        let dir = sandbox();
        let file = dir.path().join("a.txt");
        std::fs::write(&file, b"hello").expect("write");

        assert!(file.vfs_try_exists().expect("try_exists"));
        assert!(!dir.path().join("missing").vfs_try_exists().expect("try_exists"));
    }

    #[test]
    fn files_are_distinguished_from_directories() {
        let dir = sandbox();
        let file = dir.path().join("a.txt");
        std::fs::write(&file, b"hello").expect("write");

        assert!(file.vfs_is_file());
        assert!(!file.vfs_is_dir());
        assert!(dir.path().vfs_is_dir());
        assert!(!dir.path().vfs_is_file());
    }

    #[test]
    fn metadata_reports_the_length() {
        let dir = sandbox();
        let file = dir.path().join("a.txt");
        std::fs::write(&file, b"hello").expect("write");

        assert_eq!(file.vfs_metadata().expect("metadata").len(), 5);
        assert!(file.vfs_symlink_metadata().expect("symlink_metadata").is_file());
    }

    #[test]
    fn canonicalize_resolves_to_an_existing_path() {
        let dir = sandbox();
        let file = dir.path().join("a.txt");
        std::fs::write(&file, b"hello").expect("write");

        let resolved: PathBuf = file.vfs_canonicalize().expect("canonicalize");
        assert!(resolved.vfs_is_file());
    }

    #[test]
    fn a_missing_path_reports_absent_rather_than_failing() {
        let dir = sandbox();
        let missing = dir.path().join("missing");

        assert!(!missing.vfs_exists());
        assert!(!missing.vfs_is_file());
        assert!(!missing.vfs_is_dir());
        assert!(missing.vfs_metadata().is_err());
    }
}
