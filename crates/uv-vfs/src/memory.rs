use std::io;
use std::path::{Path, PathBuf};
use std::sync::RwLock;

use rustc_hash::FxHashMap;
use web_time::SystemTime;

use crate::path::{normalize, parent_of};
use crate::{Vfs, VfsDirEntry, VfsKind, VfsMetadata, not_found, unsupported};

const SYMLINK_HOP_LIMIT: usize = 16;

#[derive(Debug, Clone)]
enum Node {
    File { data: Vec<u8>, modified: SystemTime },
    Directory { modified: SystemTime },
    Symlink { target: PathBuf, modified: SystemTime },
}

impl Node {
    fn kind(&self) -> VfsKind {
        match self {
            Node::File { .. } => VfsKind::File,
            Node::Directory { .. } => VfsKind::Directory,
            Node::Symlink { .. } => VfsKind::Symlink,
        }
    }

    fn modified(&self) -> SystemTime {
        match self {
            Node::File { modified, .. }
            | Node::Directory { modified, .. }
            | Node::Symlink { modified, .. } => *modified,
        }
    }

    fn len(&self) -> u64 {
        match self {
            Node::File { data, .. } => data.len() as u64,
            Node::Directory { .. } => 0,
            Node::Symlink { target, .. } => target.as_os_str().len() as u64,
        }
    }

    fn metadata(&self) -> VfsMetadata {
        VfsMetadata { kind: self.kind(), len: self.len(), modified: self.modified() }
    }
}

#[derive(Debug)]
pub struct MemoryFs {
    nodes: RwLock<FxHashMap<PathBuf, Node>>,
}

impl Default for MemoryFs {
    fn default() -> Self {
        Self::new()
    }
}

impl MemoryFs {
    pub fn new() -> Self {
        let mut nodes = FxHashMap::default();
        nodes.insert(PathBuf::from("/"), Node::Directory { modified: now() });
        nodes.insert(
            normalize(Path::new(crate::temp::vfs_backed::TEMP_ROOT)),
            Node::Directory { modified: now() },
        );
        Self { nodes: RwLock::new(nodes) }
    }

    fn resolve(
        nodes: &FxHashMap<PathBuf, Node>,
        path: &Path,
        follow: bool,
    ) -> io::Result<(PathBuf, Node)> {
        let mut current = named(path)?;
        for _ in 0..SYMLINK_HOP_LIMIT {
            let Some(node) = nodes.get(&current) else {
                return Err(not_found(&current));
            };
            match node {
                Node::Symlink { target, .. } if follow => {
                    current = normalize(target);
                }
                _ => return Ok((current, node.clone())),
            }
        }
        Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("too many symbolic links while resolving {}", path.display()),
        ))
    }

    fn require_directory(nodes: &FxHashMap<PathBuf, Node>, path: &Path) -> io::Result<()> {
        match Self::resolve(nodes, path, true) {
            Ok((_, Node::Directory { .. })) => Ok(()),
            Ok((resolved, _)) => Err(io::Error::new(
                io::ErrorKind::NotADirectory,
                format!("{} is not a directory", resolved.display()),
            )),
            Err(error) => Err(error),
        }
    }
}

fn named(path: &Path) -> io::Result<PathBuf> {
    if path.as_os_str().is_empty() {
        return Err(not_found(path));
    }
    Ok(normalize(path))
}

fn now() -> SystemTime {
    SystemTime::now()
}

impl Vfs for MemoryFs {
    fn metadata(&self, path: &Path) -> io::Result<VfsMetadata> {
        let nodes = self.nodes.read().map_err(poisoned)?;
        let (_, node) = MemoryFs::resolve(&nodes, path, true)?;
        Ok(node.metadata())
    }

    fn symlink_metadata(&self, path: &Path) -> io::Result<VfsMetadata> {
        let nodes = self.nodes.read().map_err(poisoned)?;
        let (_, node) = MemoryFs::resolve(&nodes, path, false)?;
        Ok(node.metadata())
    }

    fn read(&self, path: &Path) -> io::Result<Vec<u8>> {
        let nodes = self.nodes.read().map_err(poisoned)?;
        let (resolved, node) = MemoryFs::resolve(&nodes, path, true)?;
        match node {
            Node::File { data, .. } => Ok(data),
            _ => Err(io::Error::new(
                io::ErrorKind::IsADirectory,
                format!("{} is not a file", resolved.display()),
            )),
        }
    }

    fn write(&self, path: &Path, contents: &[u8]) -> io::Result<()> {
        let target = named(path)?;
        let mut nodes = self.nodes.write().map_err(poisoned)?;
        if let Some(parent) = parent_of(&target) {
            MemoryFs::require_directory(&nodes, &parent)?;
        }
        if let Some(Node::Directory { .. }) = nodes.get(&target) {
            return Err(io::Error::new(
                io::ErrorKind::IsADirectory,
                format!("{} is a directory", target.display()),
            ));
        }
        nodes.insert(target, Node::File { data: contents.to_vec(), modified: now() });
        Ok(())
    }

    fn read_dir(&self, path: &Path) -> io::Result<Vec<VfsDirEntry>> {
        let nodes = self.nodes.read().map_err(poisoned)?;
        let (resolved, node) = MemoryFs::resolve(&nodes, path, true)?;
        if !matches!(node, Node::Directory { .. }) {
            return Err(io::Error::new(
                io::ErrorKind::NotADirectory,
                format!("{} is not a directory", resolved.display()),
            ));
        }

        let mut entries: Vec<VfsDirEntry> = nodes
            .iter()
            .filter_map(|(candidate, node)| {
                if candidate.parent() != Some(resolved.as_path()) || candidate == &resolved {
                    return None;
                }
                let name = candidate.file_name()?.to_str()?.to_owned();
                Some(VfsDirEntry { name, kind: node.kind() })
            })
            .collect();
        entries.sort_by(|left, right| left.name.cmp(&right.name));
        Ok(entries)
    }

    fn create_dir_all(&self, path: &Path) -> io::Result<()> {
        let target = named(path)?;
        let mut nodes = self.nodes.write().map_err(poisoned)?;

        let mut cursor = PathBuf::from("/");
        for component in target.components().skip(1) {
            cursor.push(component);
            match nodes.get(&cursor) {
                Some(Node::Directory { .. }) => {}
                Some(_) => {
                    return Err(io::Error::new(
                        io::ErrorKind::AlreadyExists,
                        format!("{} exists and is not a directory", cursor.display()),
                    ));
                }
                None => {
                    nodes.insert(cursor.clone(), Node::Directory { modified: now() });
                }
            }
        }
        Ok(())
    }

    fn remove_file(&self, path: &Path) -> io::Result<()> {
        let target = named(path)?;
        let mut nodes = self.nodes.write().map_err(poisoned)?;
        match nodes.get(&target) {
            Some(Node::Directory { .. }) => Err(io::Error::new(
                io::ErrorKind::IsADirectory,
                format!("{} is a directory", target.display()),
            )),
            Some(_) => {
                nodes.remove(&target);
                Ok(())
            }
            None => Err(not_found(&target)),
        }
    }

    fn remove_dir_all(&self, path: &Path) -> io::Result<()> {
        let target = named(path)?;
        let mut nodes = self.nodes.write().map_err(poisoned)?;
        if !nodes.contains_key(&target) {
            return Err(not_found(&target));
        }
        nodes.retain(|candidate, _| candidate != &target && !candidate.starts_with(&target));
        Ok(())
    }

    fn rename(&self, from: &Path, to: &Path) -> io::Result<()> {
        let source = named(from)?;
        let destination = named(to)?;
        let mut nodes = self.nodes.write().map_err(poisoned)?;

        if !nodes.contains_key(&source) {
            return Err(not_found(&source));
        }
        if let Some(parent) = parent_of(&destination) {
            MemoryFs::require_directory(&nodes, &parent)?;
        }

        let moved: Vec<PathBuf> = nodes
            .keys()
            .filter(|candidate| *candidate == &source || candidate.starts_with(&source))
            .cloned()
            .collect();

        nodes.retain(|candidate, _| {
            candidate != &destination && !candidate.starts_with(&destination)
        });

        for path in moved {
            let Some(node) = nodes.remove(&path) else {
                continue;
            };
            let relocated = match path.strip_prefix(&source) {
                Ok(suffix) if suffix.as_os_str().is_empty() => destination.clone(),
                Ok(suffix) => destination.join(suffix),
                Err(_) => continue,
            };
            nodes.insert(relocated, node);
        }
        Ok(())
    }

    fn symlink(&self, target: &Path, link: &Path) -> io::Result<()> {
        let location = named(link)?;
        let mut nodes = self.nodes.write().map_err(poisoned)?;
        if let Some(parent) = parent_of(&location) {
            MemoryFs::require_directory(&nodes, &parent)?;
        }
        nodes
            .insert(location, Node::Symlink { target: target.to_path_buf(), modified: now() });
        Ok(())
    }

    fn read_link(&self, path: &Path) -> io::Result<PathBuf> {
        let nodes = self.nodes.read().map_err(poisoned)?;
        let (resolved, node) = MemoryFs::resolve(&nodes, path, false)?;
        match node {
            Node::Symlink { target, .. } => Ok(target),
            _ => Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("{} is not a symbolic link", resolved.display()),
            )),
        }
    }

    fn set_modified(&self, path: &Path, time: SystemTime) -> io::Result<()> {
        let target = named(path)?;
        let mut nodes = self.nodes.write().map_err(poisoned)?;
        let Some(node) = nodes.get_mut(&target) else {
            return Err(not_found(&target));
        };
        match node {
            Node::File { modified, .. }
            | Node::Directory { modified, .. }
            | Node::Symlink { modified, .. } => *modified = time,
        }
        Ok(())
    }

    fn hard_link(&self, source: &Path, _target: &Path) -> io::Result<()> {
        Err(unsupported("hard link", source))
    }
}

fn poisoned<T>(_error: T) -> io::Error {
    io::Error::other("the virtual filesystem lock was poisoned")
}

#[cfg(test)]
mod tests {
    use super::MemoryFs;
    use crate::{Vfs, VfsKind};
    use std::io::ErrorKind;
    use std::path::Path;
    use web_time::{Duration, SystemTime};

    fn populated() -> MemoryFs {
        let fs = MemoryFs::new();
        fs.create_dir_all(Path::new("/work/project")).expect("create");
        fs.write(Path::new("/work/project/pyproject.toml"), b"[project]").expect("write");
        fs
    }

    #[test]
    fn reads_back_what_was_written() {
        let fs = populated();
        assert_eq!(fs.read(Path::new("/work/project/pyproject.toml")).expect("read"), b"[project]");
    }

    #[test]
    fn reports_file_metadata() {
        let fs = populated();
        let metadata = fs.metadata(Path::new("/work/project/pyproject.toml")).expect("metadata");
        assert_eq!(metadata.kind, VfsKind::File);
        assert_eq!(metadata.len, 9);
    }

    #[test]
    fn missing_paths_report_not_found() {
        let fs = MemoryFs::new();
        let error = fs.read(Path::new("/nope")).expect_err("should fail");
        assert_eq!(error.kind(), ErrorKind::NotFound);
    }

    #[test]
    fn writing_without_a_parent_directory_fails() {
        let fs = MemoryFs::new();
        let error = fs.write(Path::new("/missing/file"), b"x").expect_err("should fail");
        assert_eq!(error.kind(), ErrorKind::NotFound);
    }

    #[test]
    fn creating_directories_is_idempotent() {
        let fs = populated();
        assert!(fs.create_dir_all(Path::new("/work/project")).is_ok());
    }

    #[test]
    fn a_directory_cannot_replace_a_file() {
        let fs = populated();
        let error = fs
            .create_dir_all(Path::new("/work/project/pyproject.toml/inner"))
            .expect_err("should fail");
        assert_eq!(error.kind(), ErrorKind::AlreadyExists);
    }

    #[test]
    fn lists_immediate_children_in_name_order() {
        let fs = populated();
        fs.write(Path::new("/work/project/README.md"), b"docs").expect("write");
        let entries = fs.read_dir(Path::new("/work/project")).expect("read_dir");
        let names: Vec<&str> = entries.iter().map(|entry| entry.name.as_str()).collect();
        assert_eq!(names, vec!["README.md", "pyproject.toml"]);
    }

    #[test]
    fn a_fresh_filesystem_has_a_temp_directory() {
        let fs = MemoryFs::new();
        let metadata = fs.metadata(Path::new("/tmp")).expect("temp directory");
        assert!(metadata.is_dir());
    }

    #[test]
    fn an_empty_path_is_not_the_root() {
        let fs = populated();
        assert_eq!(fs.metadata(Path::new("")).unwrap_err().kind(), ErrorKind::NotFound);
        assert_eq!(fs.read_dir(Path::new("")).unwrap_err().kind(), ErrorKind::NotFound);
        assert_eq!(fs.create_dir_all(Path::new("")).unwrap_err().kind(), ErrorKind::NotFound);
        assert_eq!(fs.write(Path::new(""), b"x").unwrap_err().kind(), ErrorKind::NotFound);
    }

    #[test]
    fn listing_a_file_fails() {
        let fs = populated();
        let error =
            fs.read_dir(Path::new("/work/project/pyproject.toml")).expect_err("should fail");
        assert_eq!(error.kind(), ErrorKind::NotADirectory);
    }

    #[test]
    fn removes_a_subtree() {
        let fs = populated();
        fs.remove_dir_all(Path::new("/work")).expect("remove");
        assert!(fs.metadata(Path::new("/work/project/pyproject.toml")).is_err());
        assert!(fs.metadata(Path::new("/")).is_ok());
    }

    #[test]
    fn removing_a_directory_as_a_file_fails() {
        let fs = populated();
        let error = fs.remove_file(Path::new("/work")).expect_err("should fail");
        assert_eq!(error.kind(), ErrorKind::IsADirectory);
    }

    #[test]
    fn renames_a_subtree() {
        let fs = populated();
        fs.create_dir_all(Path::new("/other")).expect("create");
        fs.rename(Path::new("/work/project"), Path::new("/other/moved")).expect("rename");

        assert_eq!(fs.read(Path::new("/other/moved/pyproject.toml")).expect("read"), b"[project]");
        assert!(fs.metadata(Path::new("/work/project")).is_err());
    }

    #[test]
    fn renaming_a_missing_path_fails() {
        let fs = MemoryFs::new();
        let error = fs.rename(Path::new("/a"), Path::new("/b")).expect_err("should fail");
        assert_eq!(error.kind(), ErrorKind::NotFound);
    }

    #[test]
    fn follows_symlinks_when_reading() {
        let fs = populated();
        fs.symlink(Path::new("/work/project/pyproject.toml"), Path::new("/work/link")).expect("link");
        assert_eq!(fs.read(Path::new("/work/link")).expect("read"), b"[project]");
    }

    #[test]
    fn symlink_metadata_does_not_follow() {
        let fs = populated();
        fs.symlink(Path::new("/work/project/pyproject.toml"), Path::new("/work/link")).expect("link");
        assert_eq!(
            fs.symlink_metadata(Path::new("/work/link")).expect("metadata").kind,
            VfsKind::Symlink
        );
    }

    #[test]
    fn reports_the_symlink_target() {
        let fs = populated();
        fs.symlink(Path::new("/work/project"), Path::new("/work/link")).expect("link");
        assert_eq!(
            fs.read_link(Path::new("/work/link")).expect("read_link"),
            Path::new("/work/project")
        );
    }

    #[test]
    fn reading_a_link_that_is_not_one_fails() {
        let fs = populated();
        let error = fs.read_link(Path::new("/work/project")).expect_err("should fail");
        assert_eq!(error.kind(), ErrorKind::InvalidInput);
    }

    #[test]
    fn breaks_out_of_a_symlink_cycle() {
        let fs = MemoryFs::new();
        fs.symlink(Path::new("/b"), Path::new("/a")).expect("link");
        fs.symlink(Path::new("/a"), Path::new("/b")).expect("link");
        let error = fs.metadata(Path::new("/a")).expect_err("should fail");
        assert_eq!(error.kind(), ErrorKind::InvalidData);
    }

    #[test]
    fn records_an_explicit_modification_time() {
        let fs = populated();
        let stamp = SystemTime::UNIX_EPOCH + Duration::from_secs(1_700_000_000);
        fs.set_modified(Path::new("/work/project/pyproject.toml"), stamp).expect("set");
        assert_eq!(
            fs.metadata(Path::new("/work/project/pyproject.toml")).expect("metadata").modified,
            stamp
        );
    }

    #[test]
    fn hard_links_are_unsupported() {
        let fs = populated();
        let error = fs
            .hard_link(Path::new("/work/project/pyproject.toml"), Path::new("/work/copy"))
            .expect_err("should fail");
        assert_eq!(error.kind(), ErrorKind::Unsupported);
    }
}
