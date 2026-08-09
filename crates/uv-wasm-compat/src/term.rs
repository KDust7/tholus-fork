use std::cell::RefCell;

pub const FALLBACK_WIDTH: u16 = 80;
pub const FALLBACK_HEIGHT: u16 = 24;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TermConfig {
    pub is_tty: bool,
    pub columns: u16,
    pub rows: u16,
}

impl Default for TermConfig {
    fn default() -> Self {
        Self { is_tty: false, columns: FALLBACK_WIDTH, rows: FALLBACK_HEIGHT }
    }
}

impl TermConfig {
    pub fn tty(columns: u16, rows: u16) -> Self {
        Self { is_tty: true, columns: columns.max(1), rows: rows.max(1) }
    }
}

thread_local! {
    static CURRENT: RefCell<TermConfig> = RefCell::new(TermConfig::default());
}

pub fn set(config: TermConfig) {
    CURRENT.with(|current| {
        *current.borrow_mut() = config;
    });
}

pub fn reset() {
    set(TermConfig::default());
}

pub fn current() -> TermConfig {
    CURRENT.with(|current| *current.borrow())
}

pub fn is_tty() -> bool {
    current().is_tty
}

pub fn columns() -> u16 {
    current().columns
}

pub fn rows() -> u16 {
    current().rows
}

pub fn resize(columns: u16, rows: u16) {
    CURRENT.with(|current| {
        let mut config = current.borrow_mut();
        config.columns = columns.max(1);
        config.rows = rows.max(1);
    });
}

#[cfg(test)]
mod tests {
    use super::{
        FALLBACK_HEIGHT, FALLBACK_WIDTH, TermConfig, columns, current, is_tty, reset, resize,
        rows, set,
    };

    #[test]
    fn defaults_to_a_non_terminal_of_conventional_size() {
        reset();
        assert!(!is_tty());
        assert_eq!(columns(), FALLBACK_WIDTH);
        assert_eq!(rows(), FALLBACK_HEIGHT);
    }

    #[test]
    fn a_terminal_reports_its_size() {
        set(TermConfig::tty(120, 40));
        assert!(is_tty());
        assert_eq!(columns(), 120);
        assert_eq!(rows(), 40);
        reset();
    }

    #[test]
    fn a_terminal_cannot_be_zero_sized() {
        set(TermConfig::tty(0, 0));
        assert_eq!(columns(), 1);
        assert_eq!(rows(), 1);
        reset();
    }

    #[test]
    fn resizing_keeps_the_terminal_flag() {
        set(TermConfig::tty(80, 24));
        resize(200, 50);
        assert_eq!(current(), TermConfig { is_tty: true, columns: 200, rows: 50 });
        reset();
    }

    #[test]
    fn resizing_clamps_to_a_usable_minimum() {
        set(TermConfig::tty(80, 24));
        resize(0, 0);
        assert_eq!(columns(), 1);
        reset();
    }
}
