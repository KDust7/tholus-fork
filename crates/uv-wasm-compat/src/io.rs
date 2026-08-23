#![expect(
    clippy::missing_const_for_thread_local,
    reason = "the initialisers already use const blocks; clippy does not see through thread_local!"
)]

use std::cell::{Cell, RefCell};
use std::io::Write;

use anstream::ColorChoice;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stream {
    Stdout,
    Stderr,
}

pub trait Sink {
    fn write(&mut self, stream: Stream, bytes: &[u8]);
}

thread_local! {
    static SINK: RefCell<Option<Box<dyn Sink>>> = const { RefCell::new(None) };
    static WRITING: Cell<bool> = const { Cell::new(false) };
}

pub fn set_sink(sink: Box<dyn Sink>) {
    SINK.with(|current| {
        *current.borrow_mut() = Some(sink);
    });
}

pub fn clear_sink() {
    SINK.with(|current| {
        *current.borrow_mut() = None;
    });
}

pub fn is_captured() -> bool {
    SINK.with(|current| current.borrow().is_some()) || WRITING.with(Cell::get)
}

pub fn color_choice() -> ColorChoice {
    match ColorChoice::global() {
        ColorChoice::Auto => {
            if crate::term::is_tty() {
                ColorChoice::Always
            } else {
                ColorChoice::Never
            }
        }
        explicit => explicit,
    }
}

pub fn write_str(stream: Stream, text: &str) {
    let Some(mut sink) = SINK.with(|current| current.borrow_mut().take()) else {
        if !WRITING.with(Cell::get) {
            write_to_std(stream, text);
        }
        return;
    };
    WRITING.with(|writing| writing.set(true));
    write_to_sink(sink.as_mut(), stream, text);
    WRITING.with(|writing| writing.set(false));
    SINK.with(|current| {
        *current.borrow_mut() = Some(sink);
    });
}

pub fn stdout(text: &str) {
    write_str(Stream::Stdout, text);
}

pub fn stderr(text: &str) {
    write_str(Stream::Stderr, text);
}

#[derive(Debug, Clone, Copy, Default)]
pub struct StdoutWriter;

impl Write for StdoutWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        write_str(Stream::Stdout, &String::from_utf8_lossy(buf));
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct LogWriter;

impl Write for LogWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let text = String::from_utf8_lossy(buf);
        write_str(
            Stream::Stderr,
            &anstream::adapter::strip_str(&text).to_string(),
        );
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

fn write_to_sink(sink: &mut dyn Sink, stream: Stream, text: &str) {
    sink.write(stream, &adapt(text));
}

fn adapt(text: &str) -> Vec<u8> {
    let mut adapted = Vec::with_capacity(text.len());
    {
        let mut writer = anstream::AutoStream::new(&mut adapted, color_choice());
        let _ = writer.write_all(text.as_bytes());
        let _ = writer.flush();
    }
    adapted
}

fn write_to_std(stream: Stream, text: &str) {
    match stream {
        Stream::Stdout => anstream::print!("{text}"),
        Stream::Stderr => anstream::eprint!("{text}"),
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::rc::Rc;
    use std::sync::{Mutex, MutexGuard, OnceLock};

    use anstream::ColorChoice;

    use super::{Sink, Stream, clear_sink, is_captured, set_sink, stderr, stdout, write_str};
    use crate::term::{TermConfig, reset, set};

    const STYLED: &str = "\u{1b}[1m\u{1b}[33mwarning\u{1b}[39m\u{1b}[0m: disk is full\n";
    const PLAIN: &str = "warning: disk is full\n";

    #[derive(Debug, Default)]
    struct Recorded {
        stdout: Vec<u8>,
        stderr: Vec<u8>,
    }

    #[derive(Debug, Clone, Default)]
    struct Recorder(Rc<RefCell<Recorded>>);

    impl Recorder {
        fn stdout(&self) -> String {
            String::from_utf8(self.0.borrow().stdout.clone()).unwrap()
        }

        fn stderr(&self) -> String {
            String::from_utf8(self.0.borrow().stderr.clone()).unwrap()
        }
    }

    impl Sink for Recorder {
        fn write(&mut self, stream: Stream, bytes: &[u8]) {
            let mut recorded = self.0.borrow_mut();
            match stream {
                Stream::Stdout => recorded.stdout.extend_from_slice(bytes),
                Stream::Stderr => recorded.stderr.extend_from_slice(bytes),
            }
        }
    }

    #[derive(Debug, Clone, Default)]
    struct Reentrant(Recorder);

    impl Sink for Reentrant {
        fn write(&mut self, stream: Stream, bytes: &[u8]) {
            self.0.write(stream, bytes);
            write_str(Stream::Stdout, "nested");
        }
    }

    fn exclusive() -> MutexGuard<'static, ()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        let guard = LOCK
            .get_or_init(|| Mutex::new(()))
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        ColorChoice::Auto.write_global();
        reset();
        clear_sink();
        guard
    }

    fn record() -> Recorder {
        let recorder = Recorder::default();
        set_sink(Box::new(recorder.clone()));
        recorder
    }

    #[test]
    fn output_is_not_captured_until_a_sink_is_installed() {
        let _guard = exclusive();
        assert!(!is_captured());
        set_sink(Box::new(Recorder::default()));
        assert!(is_captured());
        clear_sink();
        assert!(!is_captured());
    }

    #[test]
    fn a_non_terminal_strips_the_escape_sequences() {
        let _guard = exclusive();
        let recorder = record();
        stderr(STYLED);
        clear_sink();
        assert_eq!(recorder.stderr(), PLAIN);
    }

    #[test]
    fn a_terminal_keeps_the_escape_sequences() {
        let _guard = exclusive();
        set(TermConfig::tty(80, 24));
        let recorder = record();
        stderr(STYLED);
        clear_sink();
        reset();
        assert_eq!(recorder.stderr(), STYLED);
    }

    #[test]
    fn an_explicit_choice_overrides_the_terminal() {
        let _guard = exclusive();
        set(TermConfig::tty(80, 24));
        ColorChoice::Never.write_global();
        let recorder = record();
        stderr(STYLED);
        clear_sink();
        reset();
        ColorChoice::Auto.write_global();
        assert_eq!(recorder.stderr(), PLAIN);
    }

    #[test]
    fn an_explicit_choice_colors_a_non_terminal() {
        let _guard = exclusive();
        ColorChoice::Always.write_global();
        let recorder = record();
        stdout(STYLED);
        clear_sink();
        ColorChoice::Auto.write_global();
        assert_eq!(recorder.stdout(), STYLED);
    }

    #[test]
    fn the_streams_stay_apart() {
        let _guard = exclusive();
        let recorder = record();
        stdout("out");
        stderr("err");
        clear_sink();
        assert_eq!(recorder.stdout(), "out");
        assert_eq!(recorder.stderr(), "err");
    }

    #[test]
    fn writes_accumulate_in_order() {
        let _guard = exclusive();
        let recorder = record();
        stdout("Resolved ");
        stdout("3 packages\n");
        clear_sink();
        assert_eq!(recorder.stdout(), "Resolved 3 packages\n");
    }

    #[test]
    fn a_sink_that_writes_while_writing_does_not_recurse() {
        let _guard = exclusive();
        let reentrant = Reentrant::default();
        set_sink(Box::new(reentrant.clone()));
        stdout("outer");
        clear_sink();
        assert_eq!(reentrant.0.stdout(), "outer");
        assert!(!is_captured());
    }

    #[test]
    fn clearing_the_sink_stops_capture() {
        let _guard = exclusive();
        let recorder = record();
        stdout("before");
        clear_sink();
        stdout("after");
        assert_eq!(recorder.stdout(), "before");
    }
}
