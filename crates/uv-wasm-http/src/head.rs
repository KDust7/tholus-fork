use crate::headers;
use crate::range::{parse_content_range, probe_range_header};
use crate::{Method, TransportRequest, TransportResponse};

pub fn rewrite_head_as_range(request: &TransportRequest) -> Option<TransportRequest> {
    if request.method != Method::Head {
        return None;
    }

    let mut rewritten = request.clone();
    rewritten.method = Method::Get;
    let (name, value) = probe_range_header();
    headers::set(&mut rewritten.headers, name, value);
    Some(rewritten)
}

pub fn restore_head_response(response: TransportResponse) -> TransportResponse {
    let TransportResponse {
        status,
        mut headers,
        ..
    } = response;

    if status == 206 {
        let length = headers::get(&headers, "content-range")
            .and_then(parse_content_range)
            .and_then(|range| range.complete_length);
        match length {
            Some(length) => headers::set(&mut headers, "content-length", &length.to_string()),
            None => headers::remove(&mut headers, "content-length"),
        }
        return TransportResponse {
            status: 200,
            headers,
            body: Vec::new(),
        };
    }

    headers::remove(&mut headers, "content-length");
    TransportResponse {
        status,
        headers,
        body: Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::{restore_head_response, rewrite_head_as_range};
    use crate::headers;
    use crate::{Method, TransportRequest, TransportResponse};

    fn head_request() -> TransportRequest {
        TransportRequest {
            method: Method::Head,
            url: "https://files.pythonhosted.org/packages/x/rich.whl".to_owned(),
            headers: vec![("accept".to_owned(), "*/*".to_owned())],
            body: None,
        }
    }

    fn response(status: u16, headers: Vec<(String, String)>) -> TransportResponse {
        TransportResponse {
            status,
            headers,
            body: b"partial".to_vec(),
        }
    }

    #[test]
    fn a_head_becomes_a_single_byte_get() {
        let rewritten = rewrite_head_as_range(&head_request()).expect("should rewrite");
        assert_eq!(rewritten.method, Method::Get);
        assert_eq!(headers::get(&rewritten.headers, "range"), Some("bytes=0-0"));
    }

    #[test]
    fn the_rewrite_keeps_the_url_and_other_headers() {
        let rewritten = rewrite_head_as_range(&head_request()).expect("should rewrite");
        assert_eq!(rewritten.url, head_request().url);
        assert_eq!(headers::get(&rewritten.headers, "accept"), Some("*/*"));
    }

    #[test]
    fn a_get_is_left_alone() {
        let mut request = head_request();
        request.method = Method::Get;
        assert!(rewrite_head_as_range(&request).is_none());
    }

    #[test]
    fn an_existing_range_header_is_replaced() {
        let mut request = head_request();
        headers::set(&mut request.headers, "range", "bytes=100-200");
        let rewritten = rewrite_head_as_range(&request).expect("should rewrite");
        assert_eq!(headers::get(&rewritten.headers, "range"), Some("bytes=0-0"));
    }

    #[test]
    fn a_partial_response_becomes_a_head_response() {
        let restored = restore_head_response(response(
            206,
            vec![("content-range".to_owned(), "bytes 0-0/11050".to_owned())],
        ));
        assert_eq!(restored.status, 200);
        assert!(restored.body.is_empty());
        assert_eq!(
            headers::get(&restored.headers, "content-length"),
            Some("11050")
        );
    }

    #[test]
    fn an_unknown_total_length_drops_content_length() {
        let restored = restore_head_response(response(
            206,
            vec![
                ("content-range".to_owned(), "bytes 0-0/*".to_owned()),
                ("content-length".to_owned(), "1".to_owned()),
            ],
        ));
        assert_eq!(headers::get(&restored.headers, "content-length"), None);
    }

    #[test]
    fn a_server_that_ignored_the_range_still_yields_a_head_response() {
        let restored = restore_head_response(response(
            200,
            vec![("content-length".to_owned(), "11050".to_owned())],
        ));
        assert_eq!(restored.status, 200);
        assert!(restored.body.is_empty());
        assert_eq!(headers::get(&restored.headers, "content-length"), None);
    }

    #[test]
    fn an_error_status_passes_through() {
        let restored = restore_head_response(response(404, Vec::new()));
        assert_eq!(restored.status, 404);
        assert!(restored.body.is_empty());
    }

    #[test]
    fn a_malformed_content_range_drops_content_length() {
        let restored = restore_head_response(response(
            206,
            vec![("content-range".to_owned(), "garbage".to_owned())],
        ));
        assert_eq!(restored.status, 200);
        assert_eq!(headers::get(&restored.headers, "content-length"), None);
    }

    #[test]
    fn accept_ranges_survives_the_restore() {
        let restored = restore_head_response(response(
            206,
            vec![
                ("content-range".to_owned(), "bytes 0-0/10".to_owned()),
                ("accept-ranges".to_owned(), "bytes".to_owned()),
            ],
        ));
        assert_eq!(
            headers::get(&restored.headers, "accept-ranges"),
            Some("bytes")
        );
    }
}
