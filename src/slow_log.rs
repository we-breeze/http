use std::time::{Duration, Instant};

use reqwest::{Request, RequestBuilder};

const SLOW_REQUEST_THRESHOLD: Duration = Duration::from_secs(1);
const MAX_BODY_BYTES: usize = 2 * 1024;

pub(crate) struct Observation {
    started: Instant,
    request: Option<RequestSnapshot>,
    finished: bool,
}

struct RequestSnapshot {
    method: reqwest::Method,
    url: String,
    body: Box<str>,
    body_len: Option<usize>,
}

impl Observation {
    pub(crate) fn builder(builder: &RequestBuilder) -> Self {
        Self {
            started: Instant::now(),
            request: builder
                .try_clone()
                .and_then(|builder| builder.build().ok())
                .map(|request| RequestSnapshot::new(&request)),
            finished: false,
        }
    }

    pub(crate) fn request(request: &Request) -> Self {
        Self {
            started: Instant::now(),
            request: Some(RequestSnapshot::new(request)),
            finished: false,
        }
    }

    pub(crate) fn finish(mut self, result: &reqwest::Result<crate::Response>) {
        self.log(
            result.is_ok(),
            result
                .as_ref()
                .ok()
                .map(|response| response.status().as_u16()),
        );
        self.finished = true;
    }

    fn log(&self, success: bool, status: Option<u16>) {
        let elapsed = self.started.elapsed();
        if elapsed < SLOW_REQUEST_THRESHOLD {
            return;
        }
        tracing::warn!(
            target: "breeze.slow",
            "{}",
            SlowLogLine {
                request: self.request.as_ref(),
                status,
                elapsed_ms: elapsed.as_millis(),
                success,
            },
        );
    }
}

impl Drop for Observation {
    fn drop(&mut self) {
        if !self.finished {
            self.log(false, None);
        }
    }
}

impl RequestSnapshot {
    fn new(request: &Request) -> Self {
        let mut url = request.url().clone();
        let _ = url.set_username("");
        let _ = url.set_password(None);
        url.set_query(None);
        url.set_fragment(None);
        let body = request.body().and_then(reqwest::Body::as_bytes);
        Self {
            method: request.method().clone(),
            url: url.to_string(),
            body: body.map(truncate_detail).unwrap_or_default(),
            body_len: body.map(<[u8]>::len),
        }
    }
}

fn truncate_detail(bytes: &[u8]) -> Box<str> {
    String::from_utf8_lossy(&bytes[..bytes.len().min(MAX_BODY_BYTES)]).into()
}

struct OptionalStatus(Option<u16>);

impl std::fmt::Display for OptionalStatus {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.0 {
            Some(status) => status.fmt(formatter),
            None => formatter.write_str("-"),
        }
    }
}

struct OptionalLength(Option<usize>);

impl std::fmt::Display for OptionalLength {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.0 {
            Some(length) => length.fmt(formatter),
            None => formatter.write_str("-"),
        }
    }
}

struct SlowLogLine<'a> {
    request: Option<&'a RequestSnapshot>,
    status: Option<u16>,
    elapsed_ms: u128,
    success: bool,
}

impl std::fmt::Display for SlowLogLine<'_> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Some(request) = self.request else {
            return write!(
                formatter,
                "http - - {} {}ms - {} -",
                OptionalStatus(self.status),
                self.elapsed_ms,
                self.success,
            );
        };
        write!(
            formatter,
            "http {} {} {} {}ms {} {} {}",
            request.method,
            request.url,
            OptionalStatus(self.status),
            self.elapsed_ms,
            OptionalLength(request.body_len),
            self.success,
            request.body,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slow_http_line_is_positional_and_keeps_body_last() {
        let request = RequestSnapshot {
            method: reqwest::Method::POST,
            url: "https://example.test/v1/quota".to_string(),
            body: r#"{"user_name":"sifang"}"#.into(),
            body_len: Some(22),
        };
        assert_eq!(
            SlowLogLine {
                request: Some(&request),
                status: Some(200),
                elapsed_ms: 1083,
                success: true,
            }
            .to_string(),
            r#"http POST https://example.test/v1/quota 200 1083ms 22 true {"user_name":"sifang"}"#,
        );
    }
}
