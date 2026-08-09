use std::future::Future;
use std::pin::Pin;

pub mod head;
pub mod headers;
pub mod range;

pub use head::{restore_head_response, rewrite_head_as_range};
pub use headers::Headers;
pub use range::{ContentRange, parse_content_range};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Method {
    Get,
    Head,
    Post,
    Put,
}

impl Method {
    pub fn as_str(self) -> &'static str {
        match self {
            Method::Get => "GET",
            Method::Head => "HEAD",
            Method::Post => "POST",
            Method::Put => "PUT",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransportRequest {
    pub method: Method,
    pub url: String,
    pub headers: Headers,
    pub body: Option<Vec<u8>>,
}

impl TransportRequest {
    pub fn get(url: impl Into<String>) -> Self {
        Self { method: Method::Get, url: url.into(), headers: Vec::new(), body: None }
    }

    pub fn head(url: impl Into<String>) -> Self {
        Self { method: Method::Head, url: url.into(), headers: Vec::new(), body: None }
    }

    pub fn with_header(mut self, name: &str, value: &str) -> Self {
        headers::set(&mut self.headers, name, value);
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransportResponse {
    pub status: u16,
    pub headers: Headers,
    pub body: Vec<u8>,
}

impl TransportResponse {
    pub fn is_success(&self) -> bool {
        (200..300).contains(&self.status)
    }

    pub fn content_length(&self) -> Option<u64> {
        headers::get(&self.headers, "content-length").and_then(|value| value.trim().parse().ok())
    }

    pub fn supports_ranges(&self) -> bool {
        self.status == 206
            || headers::get(&self.headers, "accept-ranges")
                .is_some_and(|value| value.eq_ignore_ascii_case("bytes"))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TransportError {
    Network(String),
    Cancelled,
    Unsupported(String),
}

impl std::fmt::Display for TransportError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TransportError::Network(detail) => write!(formatter, "network request failed: {detail}"),
            TransportError::Cancelled => write!(formatter, "the request was cancelled"),
            TransportError::Unsupported(detail) => {
                write!(formatter, "the transport does not support this request: {detail}")
            }
        }
    }
}

impl std::error::Error for TransportError {}

pub type TransportFuture<'a> =
    Pin<Box<dyn Future<Output = Result<TransportResponse, TransportError>> + 'a>>;

pub trait Transport {
    fn send(&self, request: TransportRequest) -> TransportFuture<'_>;
}

pub async fn send_with_head_support<T: Transport + ?Sized>(
    transport: &T,
    request: TransportRequest,
) -> Result<TransportResponse, TransportError> {
    match rewrite_head_as_range(&request) {
        Some(rewritten) => {
            let response = transport.send(rewritten).await?;
            Ok(restore_head_response(response))
        }
        None => transport.send(request).await,
    }
}

#[cfg(test)]
mod tests {
    use super::{
        Method, Transport, TransportError, TransportFuture, TransportRequest, TransportResponse,
        headers, send_with_head_support,
    };
    use std::cell::RefCell;

    struct Recording {
        response: TransportResponse,
        seen: RefCell<Vec<TransportRequest>>,
    }

    impl Transport for Recording {
        fn send(&self, request: TransportRequest) -> TransportFuture<'_> {
            self.seen.borrow_mut().push(request);
            let response = self.response.clone();
            Box::pin(async move { Ok(response) })
        }
    }

    fn recording(status: u16, headers: Vec<(String, String)>) -> Recording {
        Recording {
            response: TransportResponse { status, headers, body: b"x".to_vec() },
            seen: RefCell::new(Vec::new()),
        }
    }

    #[test]
    fn methods_render_as_uppercase() {
        assert_eq!(Method::Get.as_str(), "GET");
        assert_eq!(Method::Head.as_str(), "HEAD");
        assert_eq!(Method::Post.as_str(), "POST");
        assert_eq!(Method::Put.as_str(), "PUT");
    }

    #[test]
    fn requests_build_fluently() {
        let request = TransportRequest::get("https://pypi.org/simple/rich/")
            .with_header("accept", "application/vnd.pypi.simple.v1+json");
        assert_eq!(request.method, Method::Get);
        assert_eq!(headers::get(&request.headers, "Accept"), Some("application/vnd.pypi.simple.v1+json"));
    }

    #[test]
    fn success_covers_the_two_hundreds() {
        let ok = TransportResponse { status: 204, headers: Vec::new(), body: Vec::new() };
        assert!(ok.is_success());
        let missing = TransportResponse { status: 404, headers: Vec::new(), body: Vec::new() };
        assert!(!missing.is_success());
    }

    #[test]
    fn content_length_is_parsed() {
        let response = TransportResponse {
            status: 200,
            headers: vec![("content-length".to_owned(), " 4096 ".to_owned())],
            body: Vec::new(),
        };
        assert_eq!(response.content_length(), Some(4096));
    }

    #[test]
    fn a_missing_content_length_is_absent() {
        let response = TransportResponse { status: 200, headers: Vec::new(), body: Vec::new() };
        assert_eq!(response.content_length(), None);
    }

    #[test]
    fn range_support_is_detected_from_accept_ranges() {
        let response = TransportResponse {
            status: 200,
            headers: vec![("accept-ranges".to_owned(), "Bytes".to_owned())],
            body: Vec::new(),
        };
        assert!(response.supports_ranges());
    }

    #[test]
    fn a_partial_response_implies_range_support() {
        let response = TransportResponse { status: 206, headers: Vec::new(), body: Vec::new() };
        assert!(response.supports_ranges());
    }

    #[test]
    fn errors_describe_themselves() {
        assert!(TransportError::Network("offline".to_owned()).to_string().contains("offline"));
        assert_eq!(TransportError::Cancelled.to_string(), "the request was cancelled");
        assert!(TransportError::Unsupported("streams".to_owned()).to_string().contains("streams"));
    }

    #[tokio::test]
    async fn a_head_is_sent_as_a_ranged_get() {
        let transport = recording(
            206,
            vec![("content-range".to_owned(), "bytes 0-0/11050".to_owned())],
        );
        let response =
            send_with_head_support(&transport, TransportRequest::head("https://example.invalid/a"))
                .await
                .expect("should succeed");

        let sent = transport.seen.borrow();
        assert_eq!(sent[0].method, Method::Get);
        assert_eq!(headers::get(&sent[0].headers, "range"), Some("bytes=0-0"));
        assert_eq!(response.status, 200);
        assert_eq!(response.content_length(), Some(11050));
        assert!(response.body.is_empty());
    }

    #[tokio::test]
    async fn a_get_passes_through_untouched() {
        let transport = recording(200, Vec::new());
        let response =
            send_with_head_support(&transport, TransportRequest::get("https://example.invalid/a"))
                .await
                .expect("should succeed");

        assert_eq!(transport.seen.borrow()[0].method, Method::Get);
        assert_eq!(response.body, b"x".to_vec());
    }
}
