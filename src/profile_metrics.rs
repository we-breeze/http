use reqwest::Url;

#[cfg(feature = "metrics")]
use std::time::{Duration, Instant};

#[cfg(feature = "metrics")]
use brz_metrics::Metric;

use crate::response::Response;

#[cfg(feature = "metrics")]
#[derive(Clone, Copy, Debug)]
pub(crate) struct ProfileMetrics {
    endpoint: Metric,
    whole: Metric,
}

#[cfg(not(feature = "metrics"))]
#[derive(Clone, Copy, Debug)]
pub(crate) struct ProfileMetrics;

impl ProfileMetrics {
    #[cfg(feature = "metrics")]
    pub(crate) fn for_url(url: &Url) -> Self {
        Self::for_name(&stable_profile_name(url))
    }

    #[cfg(not(feature = "metrics"))]
    pub(crate) fn for_url(_url: &Url) -> Self {
        Self
    }

    #[cfg(feature = "metrics")]
    pub(crate) fn for_name(name: &str) -> Self {
        let whole_name = format!("all_{name}");
        Self {
            endpoint: Metric::http(name),
            whole: Metric::http_all(&whole_name),
        }
    }

    #[cfg(not(feature = "metrics"))]
    pub(crate) fn for_name(_name: &str) -> Self {
        Self
    }

    #[cfg(feature = "metrics")]
    pub(crate) async fn send(self, request: reqwest::RequestBuilder) -> reqwest::Result<Response> {
        let mut attempt = ProfileAttempt::new(self);
        match request.send().await {
            Ok(response) => Ok(Response::new(response, attempt.response_profile())),
            Err(error) => {
                attempt.finish(false);
                Err(error)
            }
        }
    }

    #[cfg(not(feature = "metrics"))]
    #[inline]
    pub(crate) async fn send(self, request: reqwest::RequestBuilder) -> reqwest::Result<Response> {
        request.send().await.map(Response::new_unprofiled)
    }

    #[cfg(feature = "metrics")]
    pub(crate) async fn execute(
        self,
        client: &reqwest::Client,
        request: reqwest::Request,
    ) -> reqwest::Result<Response> {
        let mut attempt = ProfileAttempt::new(self);
        match client.execute(request).await {
            Ok(response) => Ok(Response::new(response, attempt.response_profile())),
            Err(error) => {
                attempt.finish(false);
                Err(error)
            }
        }
    }

    #[cfg(not(feature = "metrics"))]
    #[inline]
    pub(crate) async fn execute(
        self,
        client: &reqwest::Client,
        request: reqwest::Request,
    ) -> reqwest::Result<Response> {
        client.execute(request).await.map(Response::new_unprofiled)
    }
}

#[cfg(feature = "metrics")]
fn stable_profile_name(url: &Url) -> String {
    let mut profile_url = url.clone();
    let _ = profile_url.set_username("");
    let _ = profile_url.set_password(None);
    profile_url.set_query(None);
    profile_url.set_fragment(None);
    profile_url.to_string()
}

#[cfg(feature = "metrics")]
struct ProfileAttempt {
    metrics: ProfileMetrics,
    started: Instant,
    finished: bool,
}

#[cfg(feature = "metrics")]
impl ProfileAttempt {
    #[inline]
    fn new(metrics: ProfileMetrics) -> Self {
        Self {
            metrics,
            started: Instant::now(),
            finished: false,
        }
    }

    #[inline]
    fn finish(&mut self, success: bool) {
        let elapsed = self.started.elapsed();
        // api-commons leaves its first-stage timestamp at request start when
        // no response headers arrive, producing a zero-duration endpoint row.
        let endpoint_elapsed = if success { elapsed } else { Duration::ZERO };
        self.metrics.endpoint.record(endpoint_elapsed, success);
        self.metrics.whole.record(elapsed, success);
        self.finished = true;
    }

    #[inline]
    fn response_profile(&mut self) -> ResponseProfile {
        let profile = ResponseProfile {
            metrics: self.metrics,
            started: self.started,
            headers_elapsed: self.started.elapsed(),
            finished: false,
        };
        self.finished = true;
        profile
    }
}

#[cfg(feature = "metrics")]
pub(crate) struct ResponseProfile {
    metrics: ProfileMetrics,
    started: Instant,
    headers_elapsed: Duration,
    finished: bool,
}

#[cfg(not(feature = "metrics"))]
pub(crate) struct ResponseProfile;

impl ResponseProfile {
    #[cfg(feature = "metrics")]
    #[inline]
    pub(crate) fn finish(&mut self, success: bool) {
        if self.finished {
            return;
        }
        self.metrics.endpoint.record(self.headers_elapsed, success);
        self.metrics.whole.record(self.started.elapsed(), success);
        self.finished = true;
    }

    #[cfg(not(feature = "metrics"))]
    #[inline]
    // Keep the same method API as the metrics-enabled implementation.
    #[allow(clippy::unused_self)]
    pub(crate) fn finish(&mut self, _success: bool) {}
}

#[cfg(feature = "metrics")]
impl Drop for ResponseProfile {
    fn drop(&mut self) {
        if !self.finished {
            self.metrics.endpoint.record(self.headers_elapsed, false);
            self.metrics.whole.record(self.started.elapsed(), false);
        }
    }
}

#[cfg(feature = "metrics")]
impl Drop for ProfileAttempt {
    fn drop(&mut self) {
        if !self.finished {
            let elapsed = self.started.elapsed();
            self.metrics.endpoint.record(Duration::ZERO, false);
            self.metrics.whole.record(elapsed, false);
        }
    }
}

#[cfg(all(test, feature = "metrics"))]
mod tests {
    use super::*;

    #[test]
    fn stable_name_excludes_secrets_query_and_fragment() {
        let url = Url::parse("http://user:secret@example.com/api/items?id=42#part").unwrap();
        assert_eq!(stable_profile_name(&url), "http://example.com/api/items");
    }

    #[test]
    fn response_completion_records_only_once() {
        let name = format!("http-profile-completion-test-{}", std::process::id());
        let metrics = ProfileMetrics::for_name(&name);
        let mut response = ResponseProfile {
            metrics,
            started: Instant::now(),
            headers_elapsed: Duration::from_millis(1),
            finished: false,
        };

        response.finish(true);
        response.finish(false);

        assert_eq!(metrics.endpoint.snapshot().total, 1);
        assert_eq!(metrics.endpoint.snapshot().failure, 0);
        assert_eq!(metrics.whole.snapshot().total, 1);
        assert_eq!(metrics.whole.snapshot().failure, 0);
    }
}
