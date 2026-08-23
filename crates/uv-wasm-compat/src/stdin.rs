#![expect(
    clippy::missing_const_for_thread_local,
    reason = "the initialisers already use const blocks; clippy does not see through thread_local!"
)]

use std::cell::RefCell;
use std::io::{BufRead, Cursor, Result};

thread_local! {
    static BUFFER: RefCell<Option<Cursor<Vec<u8>>>> = const { RefCell::new(None) };
}

pub fn set(bytes: Vec<u8>) {
    BUFFER.with(|buffer| {
        *buffer.borrow_mut() = Some(Cursor::new(bytes));
    });
}

pub fn reset() {
    BUFFER.with(|buffer| {
        *buffer.borrow_mut() = None;
    });
}

pub fn is_set() -> bool {
    BUFFER.with(|buffer| buffer.borrow().is_some())
}

#[derive(Debug, Clone, Copy)]
pub struct Stdin(());

pub fn stdin() -> Stdin {
    Stdin(())
}

impl Stdin {
    pub fn read_to_end(&self, buffer: &mut Vec<u8>) -> Result<usize> {
        with_source(|source| source.read_to_end(buffer))
    }

    pub fn read_to_string(&self, buffer: &mut String) -> Result<usize> {
        with_source(|source| source.read_to_string(buffer))
    }

    pub fn read_line(&self, buffer: &mut String) -> Result<usize> {
        with_source(|source| source.read_line(buffer))
    }
}

fn with_source<T>(read: impl FnOnce(&mut dyn BufRead) -> Result<T>) -> Result<T> {
    BUFFER.with(|buffer| match buffer.borrow_mut().as_mut() {
        Some(cursor) => read(cursor),
        None => host(read),
    })
}

#[cfg(not(target_family = "wasm"))]
fn host<T>(read: impl FnOnce(&mut dyn BufRead) -> Result<T>) -> Result<T> {
    read(&mut std::io::stdin().lock())
}

#[cfg(target_family = "wasm")]
fn host<T>(_read: impl FnOnce(&mut dyn BufRead) -> Result<T>) -> Result<T> {
    Err(std::io::Error::other(
        "this command reads from standard input, which the browser does not have; supply it with the invocation",
    ))
}

const _: () = {
    const fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<Stdin>();
};

#[cfg(test)]
mod tests {
    use super::{is_set, reset, set, stdin};

    fn read_all() -> String {
        let mut text = String::new();
        stdin().read_to_string(&mut text).unwrap();
        text
    }

    #[test]
    fn an_installed_buffer_is_read_verbatim() {
        set(b"anyio\nidna\n".to_vec());
        assert_eq!(read_all(), "anyio\nidna\n");
        reset();
    }

    #[test]
    fn a_second_read_sees_end_of_input() {
        set(b"anyio\n".to_vec());
        assert_eq!(read_all(), "anyio\n");
        assert_eq!(read_all(), "");
        reset();
    }

    #[test]
    fn read_line_stops_at_the_newline() {
        set(b"first\nsecond\n".to_vec());
        let mut line = String::new();
        stdin().read_line(&mut line).unwrap();
        assert_eq!(line, "first\n");
        assert_eq!(read_all(), "second\n");
        reset();
    }

    #[test]
    fn read_to_end_yields_the_raw_bytes() {
        set(vec![0x00, 0xff, 0x10]);
        let mut bytes = Vec::new();
        stdin().read_to_end(&mut bytes).unwrap();
        assert_eq!(bytes, vec![0x00, 0xff, 0x10]);
        reset();
    }

    #[test]
    fn invalid_utf8_is_rejected_as_a_string() {
        set(vec![0xff, 0xfe]);
        let mut text = String::new();
        let error = stdin().read_to_string(&mut text).unwrap_err();
        assert_eq!(error.kind(), std::io::ErrorKind::InvalidData);
        reset();
    }

    #[test]
    fn an_empty_buffer_is_not_the_absence_of_one() {
        reset();
        assert!(!is_set());
        set(Vec::new());
        assert!(is_set());
        assert_eq!(read_all(), "");
        reset();
        assert!(!is_set());
    }
}
