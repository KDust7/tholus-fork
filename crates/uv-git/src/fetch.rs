use std::path::{Path, PathBuf};

use uv_git_types::GitUrl;
use uv_redacted::DisplaySafeUrl;

pub struct Fetch {
    /// The [`GitUrl`] reference that was fetched.
    pub(crate) git: GitUrl,
    /// The path to the checked out repository.
    pub(crate) path: PathBuf,
    /// Git LFS artifacts have been initialized (if requested).
    pub(crate) lfs_ready: bool,
}

impl Fetch {
    pub fn git(&self) -> &GitUrl {
        &self.git
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn lfs_ready(&self) -> &bool {
        &self.lfs_ready
    }
}

pub trait Reporter: Send + Sync {
    /// Callback to invoke when a repository checkout begins.
    fn on_checkout_start(&self, url: &DisplaySafeUrl, rev: &str) -> usize;

    /// Callback to invoke when a repository checkout completes.
    fn on_checkout_complete(&self, url: &DisplaySafeUrl, rev: &str, index: usize);
}
