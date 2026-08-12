use indicatif::ProgressDrawTarget;

#[cfg(not(target_family = "wasm"))]
fn stderr_target() -> ProgressDrawTarget {
    ProgressDrawTarget::stderr()
}

#[cfg(target_family = "wasm")]
fn stderr_target() -> ProgressDrawTarget {
    if uv_wasm_compat::term::is_tty() {
        ProgressDrawTarget::term_like(Box::new(BrowserTerm))
    } else {
        ProgressDrawTarget::hidden()
    }
}

#[cfg(target_family = "wasm")]
#[derive(Debug)]
struct BrowserTerm;

#[cfg(target_family = "wasm")]
impl BrowserTerm {
    fn cursor(&self, count: usize, code: char) -> std::io::Result<()> {
        if count > 0 {
            uv_wasm_compat::io::stderr(&format!("\x1b[{count}{code}"));
        }
        Ok(())
    }
}

#[cfg(target_family = "wasm")]
impl indicatif::TermLike for BrowserTerm {
    fn width(&self) -> u16 {
        uv_wasm_compat::term::columns()
    }

    fn height(&self) -> u16 {
        uv_wasm_compat::term::rows()
    }

    fn move_cursor_up(&self, n: usize) -> std::io::Result<()> {
        self.cursor(n, 'A')
    }

    fn move_cursor_down(&self, n: usize) -> std::io::Result<()> {
        self.cursor(n, 'B')
    }

    fn move_cursor_right(&self, n: usize) -> std::io::Result<()> {
        self.cursor(n, 'C')
    }

    fn move_cursor_left(&self, n: usize) -> std::io::Result<()> {
        self.cursor(n, 'D')
    }

    fn write_line(&self, s: &str) -> std::io::Result<()> {
        uv_wasm_compat::io::stderr(&format!("{s}\n"));
        Ok(())
    }

    fn write_str(&self, s: &str) -> std::io::Result<()> {
        uv_wasm_compat::io::stderr(s);
        Ok(())
    }

    fn clear_line(&self) -> std::io::Result<()> {
        uv_wasm_compat::io::stderr("\r\x1b[2K");
        Ok(())
    }

    fn flush(&self) -> std::io::Result<()> {
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Printer {
    /// A printer that suppresses all output.
    Silent,
    /// A printer that suppresses most output, but preserves "important" stdout.
    Quiet,
    /// A printer that prints to standard streams (e.g., stdout).
    Default,
    /// A printer that prints all output, including debug messages.
    Verbose,
    /// A printer that prints to standard streams, excluding all progress outputs
    NoProgress,
}

impl Printer {
    /// Create a printer from the global output settings.
    pub(crate) fn new(quiet: u8, verbose: u8, no_progress: bool) -> Self {
        if quiet == 1 {
            Self::Quiet
        } else if quiet > 1 {
            Self::Silent
        } else if verbose > 0 {
            Self::Verbose
        } else if no_progress {
            Self::NoProgress
        } else {
            Self::Default
        }
    }

    /// Return the [`ProgressDrawTarget`] for this printer.
    pub(crate) fn target(self) -> ProgressDrawTarget {
        match self {
            Self::Silent => ProgressDrawTarget::hidden(),
            Self::Quiet => ProgressDrawTarget::hidden(),
            Self::Default => stderr_target(),
            // Confusingly, hide the progress bar when in verbose mode.
            // Otherwise, it gets interleaved with debug messages.
            Self::Verbose => ProgressDrawTarget::hidden(),
            Self::NoProgress => ProgressDrawTarget::hidden(),
        }
    }

    /// Return the [`Stdout`] for this printer.
    #[allow(dead_code, reason = "to be adopted incrementally")]
    pub(crate) fn stdout_important(self) -> Stdout {
        match self {
            Self::Silent => Stdout::Disabled,
            Self::Quiet => Stdout::Enabled,
            Self::Default => Stdout::Enabled,
            Self::Verbose => Stdout::Enabled,
            Self::NoProgress => Stdout::Enabled,
        }
    }

    /// Return the [`Stdout`] for this printer.
    pub(crate) fn stdout(self) -> Stdout {
        match self {
            Self::Silent => Stdout::Disabled,
            Self::Quiet => Stdout::Disabled,
            Self::Default => Stdout::Enabled,
            Self::Verbose => Stdout::Enabled,
            Self::NoProgress => Stdout::Enabled,
        }
    }

    /// Return the [`Stderr`] for this printer.
    #[allow(dead_code)] // Only used with the optional self-update feature.
    pub(crate) fn stderr_important(self) -> Stderr {
        match self {
            Self::Silent => Stderr::Disabled,
            Self::Quiet => Stderr::Enabled,
            Self::Default => Stderr::Enabled,
            Self::Verbose => Stderr::Enabled,
            Self::NoProgress => Stderr::Enabled,
        }
    }

    /// Return the [`Stderr`] for this printer.
    pub(crate) fn stderr(self) -> Stderr {
        match self {
            Self::Silent => Stderr::Disabled,
            Self::Quiet => Stderr::Disabled,
            Self::Default => Stderr::Enabled,
            Self::Verbose => Stderr::Enabled,
            Self::NoProgress => Stderr::Enabled,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Stdout {
    Enabled,
    Disabled,
}

impl std::fmt::Write for Stdout {
    fn write_str(&mut self, s: &str) -> std::fmt::Result {
        match self {
            Self::Enabled => {
                uv_wasm_compat::io::stdout(s);
            }
            Self::Disabled => {}
        }

        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Stderr {
    Enabled,
    Disabled,
}

impl std::fmt::Write for Stderr {
    fn write_str(&mut self, s: &str) -> std::fmt::Result {
        match self {
            Self::Enabled => {
                uv_wasm_compat::io::stderr(s);
            }
            Self::Disabled => {}
        }

        Ok(())
    }
}
