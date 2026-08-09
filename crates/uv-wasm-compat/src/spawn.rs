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

#[cfg(all(test, not(target_family = "wasm")))]
mod tests {
    use super::spawn;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};
    use web_time::Duration;

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
