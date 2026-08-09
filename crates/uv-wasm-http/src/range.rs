#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ContentRange {
    pub first: u64,
    pub last: u64,
    pub complete_length: Option<u64>,
}

pub fn parse_content_range(value: &str) -> Option<ContentRange> {
    let rest = value.trim().strip_prefix("bytes")?.trim_start();
    let (span, total) = rest.split_once('/')?;
    let (first, last) = span.trim().split_once('-')?;

    let first = first.trim().parse::<u64>().ok()?;
    let last = last.trim().parse::<u64>().ok()?;
    if last < first {
        return None;
    }

    let total = total.trim();
    let complete_length = if total == "*" { None } else { Some(total.parse::<u64>().ok()?) };

    Some(ContentRange { first, last, complete_length })
}

pub fn probe_range_header() -> (&'static str, &'static str) {
    ("range", "bytes=0-0")
}

#[cfg(test)]
mod tests {
    use super::{ContentRange, parse_content_range, probe_range_header};

    #[test]
    fn parses_a_complete_range() {
        assert_eq!(
            parse_content_range("bytes 0-0/11050"),
            Some(ContentRange { first: 0, last: 0, complete_length: Some(11050) })
        );
    }

    #[test]
    fn parses_a_mid_file_range() {
        assert_eq!(
            parse_content_range("bytes 200-1023/4096"),
            Some(ContentRange { first: 200, last: 1023, complete_length: Some(4096) })
        );
    }

    #[test]
    fn accepts_an_unknown_total_length() {
        assert_eq!(
            parse_content_range("bytes 0-99/*"),
            Some(ContentRange { first: 0, last: 99, complete_length: None })
        );
    }

    #[test]
    fn tolerates_surrounding_whitespace() {
        assert_eq!(
            parse_content_range("  bytes 0-0/10  "),
            Some(ContentRange { first: 0, last: 0, complete_length: Some(10) })
        );
    }

    #[test]
    fn rejects_a_missing_unit() {
        assert_eq!(parse_content_range("0-0/10"), None);
    }

    #[test]
    fn rejects_a_missing_total() {
        assert_eq!(parse_content_range("bytes 0-0"), None);
    }

    #[test]
    fn rejects_an_inverted_span() {
        assert_eq!(parse_content_range("bytes 10-0/100"), None);
    }

    #[test]
    fn rejects_a_non_numeric_span() {
        assert_eq!(parse_content_range("bytes a-b/100"), None);
    }

    #[test]
    fn rejects_a_non_numeric_total() {
        assert_eq!(parse_content_range("bytes 0-0/many"), None);
    }

    #[test]
    fn the_probe_asks_for_a_single_byte() {
        assert_eq!(probe_range_header(), ("range", "bytes=0-0"));
    }
}
