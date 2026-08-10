#[cfg(target_family = "wasm")]
use std::convert::Infallible;
#[cfg(target_family = "wasm")]
use std::sync::LazyLock;

pub use crate::credentials::{store_credentials, store_credentials_from_url};
pub use crate::error::GitError;
pub use crate::fetch::{Fetch, Reporter};
#[cfg(not(target_family = "wasm"))]
pub use crate::git::{GIT, GIT_LFS};
pub use crate::resolver::{
    GitHttpSettings, GitResolver, GitResolverError, RepositoryReference,
    ResolvedRepositoryReference,
};
#[cfg(not(target_family = "wasm"))]
pub(crate) use crate::source::GitSource;

mod credentials;
mod error;
mod fetch;
#[cfg(not(target_family = "wasm"))]
mod git;
mod rate_limit;
mod resolver;
#[cfg(not(target_family = "wasm"))]
mod source;

#[cfg(target_family = "wasm")]
pub static GIT_LFS: LazyLock<Result<Infallible, GitError>> =
    LazyLock::new(|| Err(GitError::GitLfsNotFound));
