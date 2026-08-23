pub use fs_err::{
    DirEntry, File, OpenOptions, ReadDir, canonicalize, copy, create_dir, create_dir_all,
    hard_link, metadata, os, read, read_dir, read_link, read_to_string, remove_dir, remove_dir_all,
    remove_file, rename, set_permissions, symlink_metadata, write,
};
pub use std::fs::{FileType, Metadata, Permissions};

#[cfg(feature = "tokio")]
pub use fs_err::tokio;
