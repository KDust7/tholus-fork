use std::ffi::{OsStr, OsString};
use std::fmt::{self, Display, Formatter};
use std::io;
use std::path::{Path, PathBuf};

use crate::fs::vfs_backed::{FileType, Metadata, symlink_metadata};
use crate::global;
use crate::path::normalize;

#[derive(Debug)]
pub struct Error {
    path: Option<PathBuf>,
    inner: io::Error,
}

impl Error {
    pub(crate) fn new(path: &Path, inner: io::Error) -> Self {
        Self { path: Some(path.to_path_buf()), inner }
    }

    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    pub fn io_error(&self) -> Option<&io::Error> {
        Some(&self.inner)
    }

    pub fn into_io_error(self) -> Option<io::Error> {
        Some(self.inner)
    }
}

impl Display for Error {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match &self.path {
            Some(path) => write!(formatter, "IO error for operation on {}: {}", path.display(), self.inner),
            None => Display::fmt(&self.inner, formatter),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.inner)
    }
}

impl From<Error> for io::Error {
    fn from(error: Error) -> Self {
        error.inner
    }
}

#[derive(Debug, Clone)]
pub struct DirEntry {
    path: PathBuf,
    file_name: OsString,
    file_type: FileType,
    depth: usize,
}

impl DirEntry {
    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn into_path(self) -> PathBuf {
        self.path
    }

    pub fn file_name(&self) -> &OsStr {
        &self.file_name
    }

    pub fn file_type(&self) -> FileType {
        self.file_type
    }

    pub fn depth(&self) -> usize {
        self.depth
    }

    pub fn path_is_symlink(&self) -> bool {
        self.file_type.is_symlink()
    }

    pub fn metadata(&self) -> Result<Metadata, Error> {
        symlink_metadata(&self.path).map_err(|err| Error::new(&self.path, err))
    }
}

fn entry_for(path: &Path, depth: usize) -> Result<DirEntry, Error> {
    let metadata = symlink_metadata(path).map_err(|err| Error::new(path, err))?;
    Ok(DirEntry {
        file_name: path.file_name().unwrap_or(path.as_os_str()).to_os_string(),
        file_type: metadata.file_type(),
        path: path.to_path_buf(),
        depth,
    })
}

#[derive(Debug)]
pub struct WalkDir {
    root: PathBuf,
    min_depth: usize,
    contents_first: bool,
    sort_by_file_name: bool,
}

impl WalkDir {
    pub fn new(root: impl AsRef<Path>) -> Self {
        Self {
            root: normalize(root.as_ref()),
            min_depth: 0,
            contents_first: false,
            sort_by_file_name: false,
        }
    }

    #[must_use]
    pub fn min_depth(mut self, depth: usize) -> Self {
        self.min_depth = depth;
        self
    }

    #[must_use]
    pub fn contents_first(mut self, yes: bool) -> Self {
        self.contents_first = yes;
        self
    }

    #[must_use]
    pub fn sort_by_file_name(mut self) -> Self {
        self.sort_by_file_name = true;
        self
    }
}

impl IntoIterator for WalkDir {
    type Item = Result<DirEntry, Error>;
    type IntoIter = IntoIter;

    fn into_iter(self) -> IntoIter {
        IntoIter {
            stack: vec![Step::Enter(entry_for(&self.root, 0))],
            min_depth: self.min_depth,
            contents_first: self.contents_first,
            sort_by_file_name: self.sort_by_file_name,
        }
    }
}

#[derive(Debug)]
enum Step {
    Enter(Result<DirEntry, Error>),
    Emit(DirEntry),
}

#[derive(Debug)]
pub struct IntoIter {
    stack: Vec<Step>,
    min_depth: usize,
    contents_first: bool,
    sort_by_file_name: bool,
}

impl IntoIter {
    pub fn filter_entry<P: FnMut(&DirEntry) -> bool>(self, predicate: P) -> FilterEntry<P> {
        FilterEntry { inner: self, predicate }
    }

    fn children_of(&self, entry: &DirEntry) -> Result<Vec<Result<DirEntry, Error>>, Error> {
        let mut names: Vec<String> = global()
            .read_dir(&entry.path)
            .map_err(|err| Error::new(&entry.path, err))?
            .into_iter()
            .map(|child| child.name)
            .collect();
        if self.sort_by_file_name {
            names.sort();
        }
        Ok(names
            .into_iter()
            .map(|name| entry_for(&normalize(&entry.path.join(name)), entry.depth + 1))
            .collect())
    }

    fn expand(&mut self, entry: DirEntry) {
        let children = match self.children_of(&entry) {
            Ok(children) => children,
            Err(err) => {
                self.stack.push(Step::Enter(Err(err)));
                return;
            }
        };
        if self.contents_first {
            self.stack.push(Step::Emit(entry));
        }
        for child in children.into_iter().rev() {
            self.stack.push(Step::Enter(child));
        }
    }

    fn advance(&mut self, mut accepts: impl FnMut(&DirEntry) -> bool) -> Option<Result<DirEntry, Error>> {
        loop {
            match self.stack.pop()? {
                Step::Emit(entry) => {
                    if entry.depth >= self.min_depth {
                        return Some(Ok(entry));
                    }
                }
                Step::Enter(Err(err)) => return Some(Err(err)),
                Step::Enter(Ok(entry)) => {
                    if !accepts(&entry) {
                        continue;
                    }
                    if !entry.file_type.is_dir() {
                        if entry.depth >= self.min_depth {
                            return Some(Ok(entry));
                        }
                        continue;
                    }
                    let announce =
                        (!self.contents_first && entry.depth >= self.min_depth).then(|| entry.clone());
                    self.expand(entry);
                    if announce.is_some() {
                        return announce.map(Ok);
                    }
                }
            }
        }
    }
}

impl Iterator for IntoIter {
    type Item = Result<DirEntry, Error>;

    fn next(&mut self) -> Option<Self::Item> {
        self.advance(|_| true)
    }
}

#[derive(Debug)]
pub struct FilterEntry<P> {
    inner: IntoIter,
    predicate: P,
}

impl<P: FnMut(&DirEntry) -> bool> Iterator for FilterEntry<P> {
    type Item = Result<DirEntry, Error>;

    fn next(&mut self) -> Option<Self::Item> {
        let predicate = &mut self.predicate;
        self.inner.advance(|entry| predicate(entry))
    }
}
