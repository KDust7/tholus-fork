use std::cell::RefCell;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use web_time::SystemTime;

pub mod env;
pub mod fs;
pub mod glob;
pub mod memory;
pub mod path;
pub mod path_ext;
pub mod temp;
pub mod url;
pub mod walk;

pub use env::{var, var_os};
pub use memory::MemoryFs;
pub use path::{
    EXE_EXTENSION, EXE_SUFFIX, absolute, current_dir, home_dir, set_current_dir, split_paths,
    temp_dir,
};
pub use path_ext::VfsPathExt;
pub use url::UrlFilePathExt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VfsKind {
    File,
    Directory,
    Symlink,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VfsMetadata {
    pub kind: VfsKind,
    pub len: u64,
    pub modified: SystemTime,
}

impl VfsMetadata {
    pub fn is_file(&self) -> bool {
        self.kind == VfsKind::File
    }

    pub fn is_dir(&self) -> bool {
        self.kind == VfsKind::Directory
    }

    pub fn is_symlink(&self) -> bool {
        self.kind == VfsKind::Symlink
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VfsDirEntry {
    pub name: String,
    pub kind: VfsKind,
}

pub trait Vfs: Send + Sync + 'static {
    fn metadata(&self, path: &Path) -> io::Result<VfsMetadata>;

    fn symlink_metadata(&self, path: &Path) -> io::Result<VfsMetadata>;

    fn read(&self, path: &Path) -> io::Result<Vec<u8>>;

    fn write(&self, path: &Path, contents: &[u8]) -> io::Result<()>;

    fn read_dir(&self, path: &Path) -> io::Result<Vec<VfsDirEntry>>;

    fn create_dir_all(&self, path: &Path) -> io::Result<()>;

    fn remove_file(&self, path: &Path) -> io::Result<()>;

    fn remove_dir_all(&self, path: &Path) -> io::Result<()>;

    fn rename(&self, from: &Path, to: &Path) -> io::Result<()>;

    fn symlink(&self, target: &Path, link: &Path) -> io::Result<()>;

    fn read_link(&self, path: &Path) -> io::Result<PathBuf>;

    fn canonicalize(&self, path: &Path) -> io::Result<PathBuf>;

    fn set_modified(&self, path: &Path, time: SystemTime) -> io::Result<()>;

    fn hard_link(&self, source: &Path, target: &Path) -> io::Result<()> {
        let _ = target;
        Err(unsupported("hard link", source))
    }

    fn exists(&self, path: &Path) -> bool {
        self.metadata(path).is_ok()
    }

    fn read_to_string(&self, path: &Path) -> io::Result<String> {
        let bytes = self.read(path)?;
        String::from_utf8(bytes).map_err(|error| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("{} is not valid UTF-8: {error}", path.display()),
            )
        })
    }
}

pub fn not_found(path: &Path) -> io::Error {
    io::Error::new(io::ErrorKind::NotFound, format!("{} was not found", path.display()))
}

pub fn unsupported(operation: &str, path: &Path) -> io::Error {
    io::Error::new(
        io::ErrorKind::Unsupported,
        format!("{operation} is not supported on {}", path.display()),
    )
}

thread_local! {
    static GLOBAL: RefCell<Option<Arc<dyn Vfs>>> = const { RefCell::new(None) };
}

pub fn install_global(vfs: Arc<dyn Vfs>) {
    GLOBAL.with(|slot| {
        *slot.borrow_mut() = Some(vfs);
    });
}

pub fn global() -> Arc<dyn Vfs> {
    GLOBAL.with(|slot| {
        if let Some(vfs) = slot.borrow().as_ref() {
            return Arc::clone(vfs);
        }
        let fallback: Arc<dyn Vfs> = Arc::new(MemoryFs::new());
        *slot.borrow_mut() = Some(Arc::clone(&fallback));
        fallback
    })
}

#[cfg(test)]
mod tests {
    use super::{Vfs, VfsKind, VfsMetadata, global, install_global, not_found, unsupported};
    use crate::MemoryFs;
    use std::io::ErrorKind;
    use std::path::Path;
    use std::sync::Arc;
    use web_time::SystemTime;

    fn metadata(kind: VfsKind) -> VfsMetadata {
        VfsMetadata { kind, len: 0, modified: SystemTime::UNIX_EPOCH }
    }

    #[test]
    fn classifies_files() {
        let subject = metadata(VfsKind::File);
        assert!(subject.is_file());
        assert!(!subject.is_dir());
        assert!(!subject.is_symlink());
    }

    #[test]
    fn classifies_directories() {
        assert!(metadata(VfsKind::Directory).is_dir());
    }

    #[test]
    fn classifies_symlinks() {
        assert!(metadata(VfsKind::Symlink).is_symlink());
    }

    #[test]
    fn not_found_names_the_path() {
        let error = not_found(Path::new("/missing"));
        assert_eq!(error.kind(), ErrorKind::NotFound);
        assert!(error.to_string().contains("/missing"));
    }

    #[test]
    fn unsupported_names_the_operation() {
        let error = unsupported("hard link", Path::new("/a"));
        assert_eq!(error.kind(), ErrorKind::Unsupported);
        assert!(error.to_string().contains("hard link"));
    }

    #[test]
    fn exists_reflects_presence() {
        let fs = MemoryFs::new();
        fs.write(Path::new("/a"), b"x").expect("write");
        assert!(fs.exists(Path::new("/a")));
        assert!(!fs.exists(Path::new("/b")));
    }

    #[test]
    fn reads_text() {
        let fs = MemoryFs::new();
        fs.write(Path::new("/a"), b"hello").expect("write");
        assert_eq!(fs.read_to_string(Path::new("/a")).expect("read"), "hello");
    }

    #[test]
    fn rejects_invalid_utf8() {
        let fs = MemoryFs::new();
        fs.write(Path::new("/a"), &[0xff, 0xfe]).expect("write");
        let error = fs.read_to_string(Path::new("/a")).expect_err("should fail");
        assert_eq!(error.kind(), ErrorKind::InvalidData);
    }

    #[test]
    fn the_global_filesystem_defaults_to_memory() {
        let fs = global();
        fs.write(Path::new("/global-default"), b"x").expect("write");
        assert!(global().exists(Path::new("/global-default")));
    }

    #[test]
    fn an_installed_filesystem_replaces_the_default() {
        let replacement = Arc::new(MemoryFs::new());
        replacement.write(Path::new("/installed"), b"x").expect("write");
        install_global(replacement);
        assert!(global().exists(Path::new("/installed")));
    }
}
