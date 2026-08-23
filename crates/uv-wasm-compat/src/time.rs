use std::future::Future;

#[cfg(target_family = "wasm")]
use futures::FutureExt;
#[cfg(target_family = "wasm")]
use futures::future::{Either, select};
use web_time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Elapsed;

#[cfg(not(target_family = "wasm"))]
pub fn to_std_system_time(time: web_time::SystemTime) -> std::time::SystemTime {
    time
}

#[cfg(target_family = "wasm")]
pub fn to_std_system_time(time: web_time::SystemTime) -> std::time::SystemTime {
    std::time::UNIX_EPOCH
        + time
            .duration_since(web_time::UNIX_EPOCH)
            .unwrap_or_default()
}

impl std::fmt::Display for Elapsed {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "the operation timed out")
    }
}

impl std::error::Error for Elapsed {}

#[cfg(not(target_family = "wasm"))]
pub async fn sleep(duration: Duration) {
    tokio::time::sleep(duration).await;
}

#[cfg(target_family = "wasm")]
pub async fn sleep(duration: Duration) {
    use wasm_bindgen::JsCast;
    use wasm_bindgen::prelude::Closure;

    let millis = i32::try_from(duration.as_millis()).unwrap_or(i32::MAX);
    let promise = js_sys::Promise::new(&mut |resolve, _reject| {
        let closure = Closure::once_into_js(move || {
            let _ = resolve.call0(&wasm_bindgen::JsValue::NULL);
        });
        let global = js_sys::global();
        let set_timeout = js_sys::Reflect::get(&global, &"setTimeout".into()).ok();
        if let Some(function) =
            set_timeout.and_then(|value| value.dyn_into::<js_sys::Function>().ok())
        {
            let _ = function.call2(&global, &closure, &millis.into());
        }
    });
    let _ = wasm_bindgen_futures::JsFuture::from(promise).await;
}

#[cfg(not(target_family = "wasm"))]
pub async fn timeout<F>(duration: Duration, future: F) -> Result<F::Output, Elapsed>
where
    F: Future,
{
    tokio::time::timeout(duration, future)
        .await
        .map_err(|_| Elapsed)
}

#[cfg(target_family = "wasm")]
pub async fn timeout<F>(duration: Duration, future: F) -> Result<F::Output, Elapsed>
where
    F: Future,
{
    let deadline = sleep(duration).boxed_local();
    match select(future.boxed_local(), deadline).await {
        Either::Left((value, _)) => Ok(value),
        Either::Right(((), _)) => Err(Elapsed),
    }
}

#[cfg(test)]
mod tests {
    use super::{Elapsed, sleep, timeout};
    use std::future::pending;
    use web_time::Duration;

    #[test]
    fn elapsed_describes_itself() {
        assert_eq!(Elapsed.to_string(), "the operation timed out");
    }

    #[test]
    fn a_timeout_over_a_send_future_stays_send() {
        fn require_send<T: Send>(_: T) {}
        require_send(timeout(Duration::from_secs(1), async {}));
    }

    #[tokio::test]
    async fn a_prompt_future_finishes_within_its_budget() {
        let outcome = timeout(Duration::from_secs(5), async { 42 }).await;
        assert_eq!(outcome, Ok(42));
    }

    #[tokio::test]
    async fn a_stalled_future_times_out() {
        let outcome = timeout(Duration::from_millis(10), pending::<()>()).await;
        assert_eq!(outcome, Err(Elapsed));
    }

    #[tokio::test]
    async fn sleeping_advances_the_clock() {
        let start = web_time::Instant::now();
        sleep(Duration::from_millis(20)).await;
        assert!(start.elapsed() >= Duration::from_millis(10));
    }
}
