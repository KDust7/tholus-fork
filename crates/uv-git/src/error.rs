use owo_colors::OwoColorize;

#[derive(Debug, thiserror::Error)]
pub enum GitError {
    #[error("Git executable not found. Ensure that Git is installed and available.")]
    GitNotFound,
    #[error("Git LFS extension not found. Ensure that Git LFS is installed and available.")]
    GitLfsNotFound,
    #[error("Is Git LFS configured? Run `{}` to initialize Git LFS.", "git lfs install".green())]
    GitLfsNotConfigured,
    #[cfg(not(target_family = "wasm"))]
    #[error(transparent)]
    Other(#[from] which::Error),
    #[error(
        "Remote Git fetches are not allowed because network connectivity is disabled (i.e., with `--offline`)"
    )]
    TransportNotAllowed,
}
