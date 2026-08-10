use std::cell::Cell;
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

use web_time::SystemTime;

use crate::fs::advisory_locks::{self, Holding, LockKind};
use crate::path::normalize;
use crate::{VfsKind, VfsMetadata, global};

const FILE_MODE: u32 = 0o644;

const DIRECTORY_MODE: u32 = 0o755;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Metadata {
    inner: VfsMetadata,
}

impl Metadata {
    pub fn is_file(&self) -> bool {
        self.inner.is_file()
    }

    pub fn is_dir(&self) -> bool {
        self.inner.is_dir()
    }

    pub fn is_symlink(&self) -> bool {
        self.inner.is_symlink()
    }

    pub fn len(&self) -> u64 {
        self.inner.len
    }

    pub fn is_empty(&self) -> bool {
        self.inner.len == 0
    }

    pub fn modified(&self) -> io::Result<SystemTime> {
        Ok(self.inner.modified)
    }

    pub fn created(&self) -> io::Result<SystemTime> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "creation time is not recorded by the virtual filesystem",
        ))
    }

    pub fn permissions(&self) -> Permissions {
        let mode = if self.inner.is_dir() { DIRECTORY_MODE } else { FILE_MODE };
        Permissions { readonly: false, mode }
    }

    pub fn file_type(&self) -> FileType {
        FileType { kind: self.inner.kind }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FileType {
    kind: VfsKind,
}

impl FileType {
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Permissions {
    readonly: bool,
    mode: u32,
}

impl Permissions {
    pub fn readonly(&self) -> bool {
        self.readonly
    }

    pub fn set_readonly(&mut self, readonly: bool) {
        self.readonly = readonly;
    }

    pub fn mode(&self) -> u32 {
        self.mode
    }

    pub fn set_mode(&mut self, mode: u32) {
        self.mode = mode;
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DirEntry {
    path: PathBuf,
    name: String,
    kind: VfsKind,
}

impl DirEntry {
    pub fn path(&self) -> PathBuf {
        self.path.clone()
    }

    pub fn file_name(&self) -> std::ffi::OsString {
        std::ffi::OsString::from(&self.name)
    }

    pub fn file_type(&self) -> io::Result<FileType> {
        Ok(FileType { kind: self.kind })
    }

    pub fn metadata(&self) -> io::Result<Metadata> {
        metadata(&self.path)
    }
}

pub struct ReadDir {
    entries: std::vec::IntoIter<DirEntry>,
}

impl Iterator for ReadDir {
    type Item = io::Result<DirEntry>;

    fn next(&mut self) -> Option<Self::Item> {
        self.entries.next().map(Ok)
    }
}

pub fn read(path: impl AsRef<Path>) -> io::Result<Vec<u8>> {
    global().read(path.as_ref())
}

pub fn read_to_string(path: impl AsRef<Path>) -> io::Result<String> {
    global().read_to_string(path.as_ref())
}

pub fn write(path: impl AsRef<Path>, contents: impl AsRef<[u8]>) -> io::Result<()> {
    global().write(path.as_ref(), contents.as_ref())
}

pub fn create_dir(path: impl AsRef<Path>) -> io::Result<()> {
    global().create_dir_all(path.as_ref())
}

pub fn create_dir_all(path: impl AsRef<Path>) -> io::Result<()> {
    global().create_dir_all(path.as_ref())
}

pub fn remove_dir(path: impl AsRef<Path>) -> io::Result<()> {
    global().remove_dir_all(path.as_ref())
}

pub fn remove_dir_all(path: impl AsRef<Path>) -> io::Result<()> {
    global().remove_dir_all(path.as_ref())
}

pub fn remove_file(path: impl AsRef<Path>) -> io::Result<()> {
    global().remove_file(path.as_ref())
}

pub fn rename(from: impl AsRef<Path>, to: impl AsRef<Path>) -> io::Result<()> {
    global().rename(from.as_ref(), to.as_ref())
}

pub fn copy(from: impl AsRef<Path>, to: impl AsRef<Path>) -> io::Result<u64> {
    let vfs = global();
    let contents = vfs.read(from.as_ref())?;
    let written = contents.len() as u64;
    vfs.write(to.as_ref(), &contents)?;
    Ok(written)
}

pub fn hard_link(source: impl AsRef<Path>, target: impl AsRef<Path>) -> io::Result<()> {
    global().hard_link(source.as_ref(), target.as_ref())
}

pub fn exists(path: impl AsRef<Path>) -> bool {
    global().exists(path.as_ref())
}

pub fn try_exists(path: impl AsRef<Path>) -> io::Result<bool> {
    Ok(global().exists(path.as_ref()))
}

pub fn is_file(path: impl AsRef<Path>) -> bool {
    global().metadata(path.as_ref()).is_ok_and(|entry| entry.is_file())
}

pub fn is_dir(path: impl AsRef<Path>) -> bool {
    global().metadata(path.as_ref()).is_ok_and(|entry| entry.is_dir())
}

pub fn metadata(path: impl AsRef<Path>) -> io::Result<Metadata> {
    Ok(Metadata { inner: global().metadata(path.as_ref())? })
}

pub fn symlink_metadata(path: impl AsRef<Path>) -> io::Result<Metadata> {
    Ok(Metadata { inner: global().symlink_metadata(path.as_ref())? })
}

pub fn read_link(path: impl AsRef<Path>) -> io::Result<PathBuf> {
    global().read_link(path.as_ref())
}

pub fn canonicalize(path: impl AsRef<Path>) -> io::Result<PathBuf> {
    let normalized = normalize(path.as_ref());
    global().metadata(&normalized)?;
    Ok(normalized)
}

pub fn set_permissions(path: impl AsRef<Path>, _permissions: Permissions) -> io::Result<()> {
    global().metadata(path.as_ref())?;
    Ok(())
}

pub fn read_dir(path: impl AsRef<Path>) -> io::Result<ReadDir> {
    let root = normalize(path.as_ref());
    let entries = global()
        .read_dir(&root)?
        .into_iter()
        .map(|entry| DirEntry {
            path: normalize(&root.join(&entry.name)),
            name: entry.name,
            kind: entry.kind,
        })
        .collect::<Vec<_>>();
    Ok(ReadDir { entries: entries.into_iter() })
}

pub mod os {
    pub mod unix {
        pub mod fs {
            use std::io;
            use std::path::Path;

            use crate::global;

            pub fn symlink(original: impl AsRef<Path>, link: impl AsRef<Path>) -> io::Result<()> {
                global().symlink(original.as_ref(), link.as_ref())
            }
        }
    }

    pub mod windows {
        pub mod fs {
            use std::io;
            use std::path::Path;

            use crate::global;

            pub fn symlink_file(
                original: impl AsRef<Path>,
                link: impl AsRef<Path>,
            ) -> io::Result<()> {
                global().symlink(original.as_ref(), link.as_ref())
            }

            pub fn symlink_dir(
                original: impl AsRef<Path>,
                link: impl AsRef<Path>,
            ) -> io::Result<()> {
                global().symlink(original.as_ref(), link.as_ref())
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OpenOptions {
    read: bool,
    write: bool,
    append: bool,
    truncate: bool,
    create: bool,
    create_new: bool,
}

impl Default for OpenOptions {
    fn default() -> Self {
        Self::new()
    }
}

impl OpenOptions {
    pub fn new() -> Self {
        Self {
            read: false,
            write: false,
            append: false,
            truncate: false,
            create: false,
            create_new: false,
        }
    }

    pub fn read(&mut self, value: bool) -> &mut Self {
        self.read = value;
        self
    }

    pub fn write(&mut self, value: bool) -> &mut Self {
        self.write = value;
        self
    }

    pub fn append(&mut self, value: bool) -> &mut Self {
        self.append = value;
        self
    }

    pub fn truncate(&mut self, value: bool) -> &mut Self {
        self.truncate = value;
        self
    }

    pub fn create(&mut self, value: bool) -> &mut Self {
        self.create = value;
        self
    }

    pub fn create_new(&mut self, value: bool) -> &mut Self {
        self.create_new = value;
        self
    }

    pub fn open(&self, path: impl AsRef<Path>) -> io::Result<File> {
        File::open_with(path.as_ref(), *self)
    }
}

#[derive(Debug)]
pub struct File {
    path: PathBuf,
    buffer: Vec<u8>,
    cursor: u64,
    writable: bool,
    dirty: bool,
    holding: Holding,
}

impl File {
    pub fn open(path: impl AsRef<Path>) -> io::Result<Self> {
        let mut options = OpenOptions::new();
        options.read(true);
        Self::open_with(path.as_ref(), options)
    }

    pub fn create(path: impl AsRef<Path>) -> io::Result<Self> {
        let mut options = OpenOptions::new();
        options.write(true).create(true).truncate(true);
        Self::open_with(path.as_ref(), options)
    }

    pub fn options() -> OpenOptions {
        OpenOptions::new()
    }

    fn open_with(path: &Path, options: OpenOptions) -> io::Result<Self> {
        let vfs = global();
        let normalized = normalize(path);
        let exists = vfs.exists(&normalized);

        if options.create_new && exists {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                format!("{} already exists", normalized.display()),
            ));
        }

        let buffer = if exists && !options.truncate {
            vfs.read(&normalized)?
        } else {
            if !exists && !(options.create || options.create_new) {
                return Err(crate::not_found(&normalized));
            }
            Vec::new()
        };

        let writable = options.write || options.append || options.create || options.create_new;
        let cursor = if options.append { buffer.len() as u64 } else { 0 };

        if writable && (!exists || options.truncate) {
            vfs.write(&normalized, &buffer)?;
        }

        Ok(Self {
            path: normalized,
            buffer,
            cursor,
            writable,
            dirty: false,
            holding: Cell::new(None),
        })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn metadata(&self) -> io::Result<Metadata> {
        metadata(&self.path)
    }

    pub fn set_modified(&self, time: SystemTime) -> io::Result<()> {
        global().set_modified(&self.path, time)
    }

    pub fn set_len(&mut self, size: u64) -> io::Result<()> {
        self.require_writable()?;
        let size = usize::try_from(size).unwrap_or(usize::MAX);
        self.buffer.resize(size, 0);
        self.cursor = self.cursor.min(size as u64);
        self.dirty = true;
        self.flush()
    }

    pub fn sync_all(&mut self) -> io::Result<()> {
        self.flush()
    }

    pub fn sync_data(&mut self) -> io::Result<()> {
        self.flush()
    }

    pub fn lock(&self) -> io::Result<()> {
        self.take_or_report_contention(LockKind::Exclusive)
    }

    pub fn lock_shared(&self) -> io::Result<()> {
        self.take_or_report_contention(LockKind::Shared)
    }

    pub fn try_lock(&self) -> Result<(), std::fs::TryLockError> {
        self.try_take(LockKind::Exclusive)
    }

    pub fn try_lock_shared(&self) -> Result<(), std::fs::TryLockError> {
        self.try_take(LockKind::Shared)
    }

    pub fn unlock(&self) -> io::Result<()> {
        advisory_locks::release(&self.path, &self.holding);
        Ok(())
    }

    fn try_take(&self, kind: LockKind) -> Result<(), std::fs::TryLockError> {
        if advisory_locks::acquire(&self.path, &self.holding, kind) {
            return Ok(());
        }
        Err(std::fs::TryLockError::WouldBlock)
    }

    fn take_or_report_contention(&self, kind: LockKind) -> io::Result<()> {
        if advisory_locks::acquire(&self.path, &self.holding, kind) {
            return Ok(());
        }
        Err(io::Error::new(
            io::ErrorKind::WouldBlock,
            format!(
                "{} is locked by another handle on this thread, which cannot release it while we wait",
                self.path.display()
            ),
        ))
    }

    fn require_writable(&self) -> io::Result<()> {
        if self.writable {
            return Ok(());
        }
        Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            format!("{} was not opened for writing", self.path.display()),
        ))
    }
}

impl Read for File {
    fn read(&mut self, out: &mut [u8]) -> io::Result<usize> {
        let start = usize::try_from(self.cursor).unwrap_or(usize::MAX).min(self.buffer.len());
        let available = &self.buffer[start..];
        let count = available.len().min(out.len());
        out[..count].copy_from_slice(&available[..count]);
        self.cursor += count as u64;
        Ok(count)
    }
}

impl Write for File {
    fn write(&mut self, data: &[u8]) -> io::Result<usize> {
        self.require_writable()?;
        let start = usize::try_from(self.cursor).unwrap_or(usize::MAX);
        if start > self.buffer.len() {
            self.buffer.resize(start, 0);
        }
        let end = start + data.len();
        if end > self.buffer.len() {
            self.buffer.resize(end, 0);
        }
        self.buffer[start..end].copy_from_slice(data);
        self.cursor = end as u64;
        self.dirty = true;
        Ok(data.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        if !self.dirty {
            return Ok(());
        }
        global().write(&self.path, &self.buffer)?;
        self.dirty = false;
        Ok(())
    }
}

impl Seek for File {
    fn seek(&mut self, position: SeekFrom) -> io::Result<u64> {
        let length = self.buffer.len() as i64;
        let target = match position {
            SeekFrom::Start(offset) => i64::try_from(offset).unwrap_or(i64::MAX),
            SeekFrom::End(offset) => length.saturating_add(offset),
            SeekFrom::Current(offset) => {
                i64::try_from(self.cursor).unwrap_or(i64::MAX).saturating_add(offset)
            }
        };
        if target < 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "cannot seek before the start of the file",
            ));
        }
        self.cursor = target as u64;
        Ok(self.cursor)
    }
}

impl Drop for File {
    fn drop(&mut self) {
        let _ = self.flush();
        advisory_locks::release(&self.path, &self.holding);
    }
}

pub mod tokio {
    use std::io::{self, Read as _, Seek as _, SeekFrom, Write as _};
    use std::path::{Path, PathBuf};
    use std::pin::Pin;
    use std::task::{Context, Poll};

    use tokio::io::{AsyncRead, AsyncSeek, AsyncWrite, ReadBuf};

    use super::{FileType, Metadata, Permissions};

    pub async fn read(path: impl AsRef<Path>) -> io::Result<Vec<u8>> {
        super::read(path)
    }

    pub async fn read_to_string(path: impl AsRef<Path>) -> io::Result<String> {
        super::read_to_string(path)
    }

    pub async fn write(path: impl AsRef<Path>, contents: impl AsRef<[u8]>) -> io::Result<()> {
        super::write(path, contents)
    }

    pub async fn create_dir_all(path: impl AsRef<Path>) -> io::Result<()> {
        super::create_dir_all(path)
    }

    pub async fn remove_dir_all(path: impl AsRef<Path>) -> io::Result<()> {
        super::remove_dir_all(path)
    }

    pub async fn remove_file(path: impl AsRef<Path>) -> io::Result<()> {
        super::remove_file(path)
    }

    pub async fn rename(from: impl AsRef<Path>, to: impl AsRef<Path>) -> io::Result<()> {
        super::rename(from, to)
    }

    pub async fn metadata(path: impl AsRef<Path>) -> io::Result<Metadata> {
        super::metadata(path)
    }

    pub async fn canonicalize(path: impl AsRef<Path>) -> io::Result<PathBuf> {
        super::canonicalize(path)
    }

    pub async fn set_permissions(
        path: impl AsRef<Path>,
        permissions: Permissions,
    ) -> io::Result<()> {
        super::set_permissions(path, permissions)
    }

    pub async fn symlink_metadata(path: impl AsRef<Path>) -> io::Result<Metadata> {
        super::symlink_metadata(path)
    }

    pub async fn create_dir(path: impl AsRef<Path>) -> io::Result<()> {
        super::create_dir(path)
    }

    pub async fn remove_dir(path: impl AsRef<Path>) -> io::Result<()> {
        super::remove_dir(path)
    }

    pub async fn copy(from: impl AsRef<Path>, to: impl AsRef<Path>) -> io::Result<u64> {
        super::copy(from, to)
    }

    pub async fn hard_link(source: impl AsRef<Path>, target: impl AsRef<Path>) -> io::Result<()> {
        super::hard_link(source, target)
    }

    pub async fn symlink(original: impl AsRef<Path>, link: impl AsRef<Path>) -> io::Result<()> {
        super::os::unix::fs::symlink(original, link)
    }

    pub async fn read_link(path: impl AsRef<Path>) -> io::Result<PathBuf> {
        super::read_link(path)
    }

    pub async fn try_exists(path: impl AsRef<Path>) -> io::Result<bool> {
        super::try_exists(path)
    }

    pub async fn read_dir(path: impl AsRef<Path>) -> io::Result<ReadDir> {
        Ok(ReadDir { inner: super::read_dir(path)? })
    }

    pub struct ReadDir {
        inner: super::ReadDir,
    }

    impl ReadDir {
        pub async fn next_entry(&mut self) -> io::Result<Option<DirEntry>> {
            self.inner.next().transpose().map(|entry| entry.map(|inner| DirEntry { inner }))
        }
    }

    pub struct DirEntry {
        inner: super::DirEntry,
    }

    impl DirEntry {
        pub fn path(&self) -> PathBuf {
            self.inner.path()
        }

        pub fn file_name(&self) -> std::ffi::OsString {
            self.inner.file_name()
        }

        pub async fn file_type(&self) -> io::Result<FileType> {
            self.inner.file_type()
        }

        pub async fn metadata(&self) -> io::Result<Metadata> {
            self.inner.metadata()
        }
    }

    #[derive(Debug, Clone, Copy)]
    pub struct OpenOptions {
        inner: super::OpenOptions,
    }

    impl Default for OpenOptions {
        fn default() -> Self {
            Self::new()
        }
    }

    impl OpenOptions {
        pub fn new() -> Self {
            Self { inner: super::OpenOptions::new() }
        }

        pub fn read(&mut self, value: bool) -> &mut Self {
            self.inner.read(value);
            self
        }

        pub fn write(&mut self, value: bool) -> &mut Self {
            self.inner.write(value);
            self
        }

        pub fn append(&mut self, value: bool) -> &mut Self {
            self.inner.append(value);
            self
        }

        pub fn truncate(&mut self, value: bool) -> &mut Self {
            self.inner.truncate(value);
            self
        }

        pub fn create(&mut self, value: bool) -> &mut Self {
            self.inner.create(value);
            self
        }

        pub fn create_new(&mut self, value: bool) -> &mut Self {
            self.inner.create_new(value);
            self
        }

        pub async fn open(&self, path: impl AsRef<Path>) -> io::Result<File> {
            Ok(File::new(self.inner.open(path)?))
        }
    }

    #[derive(Debug)]
    pub struct File {
        inner: super::File,
        seeked: Option<u64>,
    }

    impl File {
        fn new(inner: super::File) -> Self {
            Self { inner, seeked: None }
        }

        pub async fn open(path: impl AsRef<Path>) -> io::Result<Self> {
            Ok(Self::new(super::File::open(path)?))
        }

        pub async fn create(path: impl AsRef<Path>) -> io::Result<Self> {
            Ok(Self::new(super::File::create(path)?))
        }

        pub async fn create_new(path: impl AsRef<Path>) -> io::Result<Self> {
            Ok(Self::new(super::OpenOptions::new().write(true).create_new(true).open(path)?))
        }

        pub fn options() -> OpenOptions {
            OpenOptions::new()
        }

        pub fn path(&self) -> &Path {
            self.inner.path()
        }

        pub async fn metadata(&self) -> io::Result<Metadata> {
            self.inner.metadata()
        }

        pub async fn set_len(&mut self, size: u64) -> io::Result<()> {
            self.inner.set_len(size)
        }

        pub async fn set_permissions(&self, permissions: Permissions) -> io::Result<()> {
            super::set_permissions(self.inner.path(), permissions)
        }

        pub async fn sync_all(&mut self) -> io::Result<()> {
            self.inner.sync_all()
        }

        pub async fn sync_data(&mut self) -> io::Result<()> {
            self.inner.sync_data()
        }
    }

    impl AsyncRead for File {
        fn poll_read(
            mut self: Pin<&mut Self>,
            _context: &mut Context<'_>,
            buffer: &mut ReadBuf<'_>,
        ) -> Poll<io::Result<()>> {
            let count = match self.inner.read(buffer.initialize_unfilled()) {
                Ok(count) => count,
                Err(error) => return Poll::Ready(Err(error)),
            };
            buffer.advance(count);
            Poll::Ready(Ok(()))
        }
    }

    impl AsyncWrite for File {
        fn poll_write(
            mut self: Pin<&mut Self>,
            _context: &mut Context<'_>,
            data: &[u8],
        ) -> Poll<io::Result<usize>> {
            Poll::Ready(self.inner.write(data))
        }

        fn poll_flush(
            mut self: Pin<&mut Self>,
            _context: &mut Context<'_>,
        ) -> Poll<io::Result<()>> {
            Poll::Ready(self.inner.flush())
        }

        fn poll_shutdown(
            self: Pin<&mut Self>,
            context: &mut Context<'_>,
        ) -> Poll<io::Result<()>> {
            self.poll_flush(context)
        }
    }

    impl AsyncSeek for File {
        fn start_seek(mut self: Pin<&mut Self>, position: SeekFrom) -> io::Result<()> {
            self.seeked = Some(self.inner.seek(position)?);
            Ok(())
        }

        fn poll_complete(
            mut self: Pin<&mut Self>,
            _context: &mut Context<'_>,
        ) -> Poll<io::Result<u64>> {
            if let Some(position) = self.seeked.take() {
                return Poll::Ready(Ok(position));
            }
            Poll::Ready(self.inner.stream_position())
        }
    }
}
