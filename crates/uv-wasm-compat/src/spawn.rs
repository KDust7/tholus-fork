use std::future::Future;

#[cfg(not(target_family = "wasm"))]
pub fn spawn<F>(future: F)
where
    F: Future<Output = ()> + Send + 'static,
{
    tokio::task::spawn(future);
}

#[cfg(target_family = "wasm")]
pub fn spawn<F>(future: F)
where
    F: Future<Output = ()> + 'static,
{
    wasm_bindgen_futures::spawn_local(future);
}

#[cfg(not(target_family = "wasm"))]
pub fn spawn_blocking<F, R>(task: F) -> tokio::task::JoinHandle<R>
where
    F: FnOnce() -> R + Send + 'static,
    R: Send + 'static,
{
    tokio::task::spawn_blocking(task)
}

#[cfg(target_family = "wasm")]
pub fn spawn_blocking<F, R>(task: F) -> std::future::Ready<Result<R, tokio::task::JoinError>>
where
    F: FnOnce() -> R + Send + 'static,
    R: Send + 'static,
{
    std::future::ready(Ok(task()))
}

#[cfg(all(test, not(target_family = "wasm")))]
mod tests {
    use super::{spawn, spawn_blocking};
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};
    use web_time::Duration;

    #[tokio::test]
    async fn a_blocking_task_returns_its_value() {
        assert_eq!(spawn_blocking(|| 7).await.expect("join"), 7);
    }

    #[tokio::test]
    async fn a_blocking_task_runs_its_side_effects() {
        let flag = Arc::new(AtomicBool::new(false));
        let observed = Arc::clone(&flag);
        spawn_blocking(move || observed.store(true, Ordering::SeqCst)).await.expect("join");
        assert!(flag.load(Ordering::SeqCst));
    }

    #[tokio::test]
    async fn a_blocking_task_carries_a_value_that_is_not_copy() {
        let carried = spawn_blocking(|| String::from("payload")).await.expect("join");
        assert_eq!(carried, "payload");
    }

    #[tokio::test]
    async fn runs_the_future_to_completion() {
        let flag = Arc::new(AtomicBool::new(false));
        let observed = Arc::clone(&flag);
        spawn(async move {
            observed.store(true, Ordering::SeqCst);
        });

        crate::time::sleep(Duration::from_millis(20)).await;
        assert!(flag.load(Ordering::SeqCst));
    }
}
