pub type Headers = Vec<(String, String)>;

pub fn get<'a>(headers: &'a [(String, String)], name: &str) -> Option<&'a str> {
    headers
        .iter()
        .find(|(key, _)| key.eq_ignore_ascii_case(name))
        .map(|(_, value)| value.as_str())
}

pub fn set(headers: &mut Headers, name: &str, value: &str) {
    remove(headers, name);
    headers.push((name.to_ascii_lowercase(), value.to_owned()));
}

pub fn remove(headers: &mut Headers, name: &str) {
    headers.retain(|(key, _)| !key.eq_ignore_ascii_case(name));
}

pub fn contains(headers: &[(String, String)], name: &str) -> bool {
    get(headers, name).is_some()
}

pub const BROWSER_CONTROLLED: [&str; 8] = [
    "accept-encoding",
    "connection",
    "content-length",
    "host",
    "keep-alive",
    "te",
    "transfer-encoding",
    "user-agent",
];

pub fn strip_browser_controlled(headers: &mut Headers) {
    headers.retain(|(key, _)| {
        !BROWSER_CONTROLLED
            .iter()
            .any(|forbidden| key.eq_ignore_ascii_case(forbidden))
    });
}

#[cfg(test)]
mod tests {
    use super::{Headers, contains, get, remove, set, strip_browser_controlled};

    fn sample() -> Headers {
        vec![
            ("Accept".to_owned(), "application/json".to_owned()),
            ("User-Agent".to_owned(), "uv/0.12.3".to_owned()),
        ]
    }

    #[test]
    fn lookup_ignores_case() {
        assert_eq!(get(&sample(), "accept"), Some("application/json"));
    }

    #[test]
    fn lookup_reports_absence() {
        assert_eq!(get(&sample(), "range"), None);
    }

    #[test]
    fn setting_replaces_an_existing_value() {
        let mut headers = sample();
        set(&mut headers, "ACCEPT", "text/html");
        assert_eq!(get(&headers, "accept"), Some("text/html"));
        assert_eq!(headers.len(), 2);
    }

    #[test]
    fn setting_appends_a_new_value() {
        let mut headers = sample();
        set(&mut headers, "range", "bytes=0-0");
        assert_eq!(get(&headers, "Range"), Some("bytes=0-0"));
    }

    #[test]
    fn removal_ignores_case() {
        let mut headers = sample();
        remove(&mut headers, "user-agent");
        assert!(!contains(&headers, "User-Agent"));
    }

    #[test]
    fn strips_headers_the_browser_owns() {
        let mut headers = sample();
        set(&mut headers, "content-length", "10");
        strip_browser_controlled(&mut headers);
        assert!(!contains(&headers, "user-agent"));
        assert!(!contains(&headers, "content-length"));
        assert!(contains(&headers, "accept"));
    }

    #[test]
    fn keeps_headers_the_index_needs() {
        let mut headers = vec![
            (
                "accept".to_owned(),
                "application/vnd.pypi.simple.v1+json".to_owned(),
            ),
            ("authorization".to_owned(), "Bearer token".to_owned()),
            ("range".to_owned(), "bytes=0-1023".to_owned()),
        ];
        strip_browser_controlled(&mut headers);
        assert_eq!(headers.len(), 3);
    }
}
