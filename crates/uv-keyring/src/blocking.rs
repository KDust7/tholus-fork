use crate::error::{Error as ErrorCode, Result};

pub(crate) async fn spawn_blocking<F, T>(f: F) -> Result<T>
where
    F: FnOnce() -> Result<T> + Send + 'static,
    T: Send + 'static,
{
    uv_wasm_compat::spawn_blocking(f)
        .await
        .map_err(|e| ErrorCode::PlatformFailure(Box::new(e)))?
}
