use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use crate::fs::vfs_backed::File;
use crate::global;

pub const TEMP_ROOT: &str = "/tmp";

static COUNTER: AtomicU64 = AtomicU64::new(0);

fn unique_name(prefix: &str, suffix: &str) -> String {
    let ordinal = COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("{prefix}{ordinal:016x}{suffix}")
}

#[derive(Debug)]
pub struct TempDir {
    path: PathBuf,
    persist: bool,
}

impl TempDir {
    pub fn new() -> io::Result<Self> {
        Self::new_in(Path::new(TEMP_ROOT))
    }

    pub fn new_in(parent: impl AsRef<Path>) -> io::Result<Self> {
        Builder::new().tempdir_in(parent)
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn keep(mut self) -> PathBuf {
        self.persist = true;
        self.path.clone()
    }

    pub fn close(mut self) -> io::Result<()> {
        self.persist = true;
        global().remove_dir_all(&self.path)
    }
}

impl AsRef<Path> for TempDir {
    fn as_ref(&self) -> &Path {
        &self.path
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        if !self.persist {
            let _ = global().remove_dir_all(&self.path);
        }
    }
}

#[derive(Debug)]
pub struct TempPath {
    path: PathBuf,
    persist: bool,
}

impl TempPath {
    pub fn keep(mut self) -> io::Result<PathBuf> {
        self.persist = true;
        Ok(self.path.clone())
    }
}

impl AsRef<Path> for TempPath {
    fn as_ref(&self) -> &Path {
        &self.path
    }
}

impl std::ops::Deref for TempPath {
    type Target = Path;

    fn deref(&self) -> &Self::Target {
        &self.path
    }
}

impl Drop for TempPath {
    fn drop(&mut self) {
        if !self.persist {
            let _ = global().remove_file(&self.path);
        }
    }
}

#[derive(Debug)]
pub struct NamedTempFile {
    path: PathBuf,
    file: Option<File>,
    persist: bool,
}

impl NamedTempFile {
    pub fn new() -> io::Result<Self> {
        Self::new_in(Path::new(TEMP_ROOT))
    }

    pub fn new_in(parent: impl AsRef<Path>) -> io::Result<Self> {
        Builder::new().tempfile_in(parent)
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn as_file(&self) -> &File {
        self.file
            .as_ref()
            .expect("the handle is present until persist consumes it")
    }

    pub fn as_file_mut(&mut self) -> &mut File {
        self.file
            .as_mut()
            .expect("the handle is present until persist consumes it")
    }

    #[expect(
        clippy::result_large_err,
        reason = "this mirrors tempfile::NamedTempFile::persist, whose error hands the file back so a caller can retry"
    )]
    pub fn persist(mut self, destination: impl AsRef<Path>) -> Result<File, PersistError> {
        if let Some(file) = self.file.as_mut() {
            if let Err(error) = io::Write::flush(file) {
                return Err(PersistError { error, file: self });
            }
        }
        self.file = None;
        if let Err(error) = global().rename(&self.path, destination.as_ref()) {
            self.file = File::open(&self.path).ok();
            return Err(PersistError { error, file: self });
        }
        self.persist = true;
        match File::open(destination.as_ref()) {
            Ok(file) => Ok(file),
            Err(error) => Err(PersistError { error, file: self }),
        }
    }

    pub fn into_temp_path(mut self) -> TempPath {
        self.persist = true;
        self.file = None;
        TempPath {
            path: self.path.clone(),
            persist: false,
        }
    }

    pub fn keep(mut self) -> io::Result<(File, PathBuf)> {
        self.persist = true;
        let file = self
            .file
            .take()
            .ok_or_else(|| io::Error::other("the temporary file handle was already taken"))?;
        Ok((file, self.path.clone()))
    }
}

impl AsRef<Path> for NamedTempFile {
    fn as_ref(&self) -> &Path {
        &self.path
    }
}

impl io::Write for NamedTempFile {
    fn write(&mut self, data: &[u8]) -> io::Result<usize> {
        io::Write::write(self.as_file_mut(), data)
    }

    fn flush(&mut self) -> io::Result<()> {
        io::Write::flush(self.as_file_mut())
    }
}

impl io::Read for NamedTempFile {
    fn read(&mut self, out: &mut [u8]) -> io::Result<usize> {
        io::Read::read(self.as_file_mut(), out)
    }
}

impl io::Seek for NamedTempFile {
    fn seek(&mut self, position: io::SeekFrom) -> io::Result<u64> {
        io::Seek::seek(self.as_file_mut(), position)
    }
}

impl Drop for NamedTempFile {
    fn drop(&mut self) {
        if !self.persist {
            self.file = None;
            let _ = global().remove_file(&self.path);
        }
    }
}

#[derive(Debug)]
pub struct PersistError {
    pub error: io::Error,
    pub file: NamedTempFile,
}

impl std::fmt::Display for PersistError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "failed to persist temporary file: {}",
            self.error
        )
    }
}

impl std::error::Error for PersistError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.error)
    }
}

impl From<PersistError> for io::Error {
    fn from(error: PersistError) -> Self {
        error.error
    }
}

#[derive(Debug, Clone)]
pub struct Builder {
    prefix: String,
    suffix: String,
}

impl Default for Builder {
    fn default() -> Self {
        Self::new()
    }
}

impl Builder {
    pub fn new() -> Self {
        Self {
            prefix: ".tmp".to_owned(),
            suffix: String::new(),
        }
    }

    #[must_use]
    pub fn prefix(mut self, prefix: &str) -> Self {
        prefix.clone_into(&mut self.prefix);
        self
    }

    #[must_use]
    pub fn suffix(mut self, suffix: &str) -> Self {
        suffix.clone_into(&mut self.suffix);
        self
    }

    pub fn tempdir(self) -> io::Result<TempDir> {
        self.tempdir_in(Path::new(TEMP_ROOT))
    }

    pub fn tempdir_in(self, parent: impl AsRef<Path>) -> io::Result<TempDir> {
        let vfs = global();
        let parent = parent.as_ref();
        vfs.create_dir_all(parent)?;
        let path = parent.join(unique_name(&self.prefix, &self.suffix));
        vfs.create_dir_all(&path)?;
        Ok(TempDir {
            path,
            persist: false,
        })
    }

    pub fn tempfile(self) -> io::Result<NamedTempFile> {
        self.tempfile_in(Path::new(TEMP_ROOT))
    }

    pub fn tempfile_in(self, parent: impl AsRef<Path>) -> io::Result<NamedTempFile> {
        let vfs = global();
        let parent = parent.as_ref();
        vfs.create_dir_all(parent)?;
        let path = parent.join(unique_name(&self.prefix, &self.suffix));
        let file = File::create(&path)?;
        Ok(NamedTempFile {
            path,
            file: Some(file),
            persist: false,
        })
    }
}

pub fn tempdir() -> io::Result<TempDir> {
    TempDir::new()
}

pub fn tempdir_in(parent: impl AsRef<Path>) -> io::Result<TempDir> {
    TempDir::new_in(parent)
}

pub fn tempfile() -> io::Result<File> {
    Ok(Builder::new().tempfile()?.into_file())
}

pub fn tempfile_in(parent: impl AsRef<Path>) -> io::Result<File> {
    Ok(Builder::new().tempfile_in(parent)?.into_file())
}

impl NamedTempFile {
    fn into_file(mut self) -> File {
        self.persist = true;
        self.file
            .take()
            .expect("the handle is present until persist consumes it")
    }
}
