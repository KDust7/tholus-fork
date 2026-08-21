use std::cell::RefCell;
use std::fmt;
use std::future::Future;
use std::pin::Pin;

#[derive(Debug, Clone)]
pub struct HookRequest {
    pub venv: String,
    pub script: String,
    pub source_tree: String,
    pub env: Vec<(String, String)>,
    pub path: String,
}

#[derive(Debug, Clone, Default)]
pub struct HookOutput {
    pub stdout: Vec<String>,
    pub stderr: Vec<String>,
    pub code: i32,
}

impl HookOutput {
    pub fn success(&self) -> bool {
        self.code == 0
    }
}

#[derive(Debug)]
pub enum HookError {
    NoRuntimeAttached,
    Failed(String),
}

impl fmt::Display for HookError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoRuntimeAttached => f.write_str(
                "Building a source distribution requires a Python runtime, and none is attached; \
                 attach one with `attachPyodide` or install only wheels",
            ),
            Self::Failed(message) => write!(f, "The attached Python runtime failed: {message}"),
        }
    }
}

impl std::error::Error for HookError {}

pub type HookFuture = Pin<Box<dyn Future<Output = Result<HookOutput, HookError>>>>;

pub trait Pep517Runner {
    fn run(&self, request: HookRequest) -> HookFuture;
}

thread_local! {
    static RUNNER: RefCell<Option<Box<dyn Pep517Runner>>> = const { RefCell::new(None) };
}

pub fn set_runner(runner: Box<dyn Pep517Runner>) {
    RUNNER.with(|current| {
        *current.borrow_mut() = Some(runner);
    });
}

pub fn clear_runner() {
    RUNNER.with(|current| {
        *current.borrow_mut() = None;
    });
}

pub fn is_attached() -> bool {
    RUNNER.with(|current| current.borrow().is_some())
}

pub fn dispatch(request: HookRequest) -> Result<HookFuture, HookError> {
    RUNNER.with(|current| {
        let borrowed = current.borrow();
        let runner = borrowed.as_ref().ok_or(HookError::NoRuntimeAttached)?;
        Ok(runner.run(request))
    })
}

pub async fn run_hook(request: HookRequest) -> Result<HookOutput, HookError> {
    dispatch(request)?.await
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Echo;

    impl Pep517Runner for Echo {
        fn run(&self, request: HookRequest) -> HookFuture {
            Box::pin(async move {
                Ok(HookOutput {
                    stdout: vec![request.script],
                    stderr: vec![request.source_tree],
                    code: 0,
                })
            })
        }
    }

    struct Refuses;

    impl Pep517Runner for Refuses {
        fn run(&self, _request: HookRequest) -> HookFuture {
            Box::pin(async { Err(HookError::Failed("the build backend crashed".to_string())) })
        }
    }

    fn request() -> HookRequest {
        HookRequest {
            venv: "/build/.venv".to_string(),
            script: "print('hello')".to_string(),
            source_tree: "/src".to_string(),
            env: vec![("PYTHONHASHSEED".to_string(), "0".to_string())],
            path: "/build/.venv/bin".to_string(),
        }
    }

    fn block_on<F: Future>(future: F) -> F::Output {
        futures::executor::block_on(future)
    }

    #[test]
    fn refuses_to_build_when_no_runtime_is_attached() {
        clear_runner();
        assert!(!is_attached());
        let error = block_on(run_hook(request())).unwrap_err();
        assert!(matches!(error, HookError::NoRuntimeAttached));
        assert!(error.to_string().contains("requires a Python runtime"));
    }

    #[test]
    fn hands_the_script_and_the_source_tree_to_the_runtime() {
        set_runner(Box::new(Echo));
        let output = block_on(run_hook(request())).unwrap();
        assert!(output.success());
        assert_eq!(output.stdout, vec!["print('hello')".to_string()]);
        assert_eq!(output.stderr, vec!["/src".to_string()]);
        clear_runner();
    }

    #[test]
    fn reports_what_the_runtime_said_when_it_fails() {
        set_runner(Box::new(Refuses));
        let error = block_on(run_hook(request())).unwrap_err();
        assert!(error.to_string().contains("the build backend crashed"));
        clear_runner();
    }

    #[test]
    fn a_cleared_runner_is_not_attached() {
        set_runner(Box::new(Echo));
        assert!(is_attached());
        clear_runner();
        assert!(!is_attached());
    }
}
