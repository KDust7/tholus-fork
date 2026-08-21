#[cfg(not(target_family = "wasm"))]
pub use tokio::process::{Child, ChildStderr, ChildStdin, ChildStdout, Command};

#[cfg(target_family = "wasm")]
pub use browser::{Child, ChildStderr, ChildStdin, ChildStdout, Command};

#[cfg(target_family = "wasm")]
mod browser {
    use std::ffi::{OsStr, OsString};
    use std::io;
    use std::path::Path;
    use std::process::{ExitStatus, Output, Stdio};

    fn unsupported(program: &OsStr) -> io::Error {
        io::Error::new(
            io::ErrorKind::Unsupported,
            format!(
                "`{}` cannot be started: the browser has no process to run it in",
                program.to_string_lossy()
            ),
        )
    }

    #[derive(Debug)]
    pub struct Command {
        program: OsString,
    }

    impl Command {
        pub fn new(program: impl AsRef<OsStr>) -> Self {
            Self {
                program: program.as_ref().to_os_string(),
            }
        }

        pub fn arg(&mut self, _arg: impl AsRef<OsStr>) -> &mut Self {
            self
        }

        pub fn args<I, S>(&mut self, _args: I) -> &mut Self
        where
            I: IntoIterator<Item = S>,
            S: AsRef<OsStr>,
        {
            self
        }

        pub fn env(&mut self, _key: impl AsRef<OsStr>, _value: impl AsRef<OsStr>) -> &mut Self {
            self
        }

        pub fn envs<I, K, V>(&mut self, _vars: I) -> &mut Self
        where
            I: IntoIterator<Item = (K, V)>,
            K: AsRef<OsStr>,
            V: AsRef<OsStr>,
        {
            self
        }

        pub fn env_remove(&mut self, _key: impl AsRef<OsStr>) -> &mut Self {
            self
        }

        pub fn env_clear(&mut self) -> &mut Self {
            self
        }

        pub fn current_dir(&mut self, _dir: impl AsRef<Path>) -> &mut Self {
            self
        }

        pub fn stdin(&mut self, _cfg: impl Into<Stdio>) -> &mut Self {
            self
        }

        pub fn stdout(&mut self, _cfg: impl Into<Stdio>) -> &mut Self {
            self
        }

        pub fn stderr(&mut self, _cfg: impl Into<Stdio>) -> &mut Self {
            self
        }

        pub fn kill_on_drop(&mut self, _kill: bool) -> &mut Self {
            self
        }

        pub fn spawn(&mut self) -> io::Result<Child> {
            Err(unsupported(&self.program))
        }

        pub async fn status(&mut self) -> io::Result<ExitStatus> {
            Err(unsupported(&self.program))
        }

        pub async fn output(&mut self) -> io::Result<Output> {
            Err(unsupported(&self.program))
        }
    }

    impl From<std::process::Command> for Command {
        fn from(command: std::process::Command) -> Self {
            Self {
                program: command.get_program().to_os_string(),
            }
        }
    }

    pub enum Child {}

    impl Child {
        pub async fn wait(&mut self) -> io::Result<ExitStatus> {
            match *self {}
        }

        pub async fn kill(&mut self) -> io::Result<()> {
            match *self {}
        }

        pub fn start_kill(&mut self) -> io::Result<()> {
            match *self {}
        }

        pub fn id(&self) -> Option<u32> {
            match *self {}
        }
    }

    pub enum ChildStdin {}

    pub enum ChildStdout {}

    pub enum ChildStderr {}
}

#[cfg(not(target_family = "wasm"))]
pub use std::process::ExitStatus;

#[cfg(target_family = "wasm")]
pub use status::ExitStatus;

#[cfg(target_family = "wasm")]
mod status {
    use std::fmt;

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
    pub struct ExitStatus(i32);

    impl ExitStatus {
        pub fn from_code(code: i32) -> Self {
            Self(code)
        }

        pub fn success(self) -> bool {
            self.0 == 0
        }

        pub fn code(self) -> Option<i32> {
            Some(self.0)
        }
    }

    impl fmt::Display for ExitStatus {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(f, "exit status: {}", self.0)
        }
    }
}
