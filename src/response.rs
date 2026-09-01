use std::fmt;
use std::ops::{Deref, DerefMut};

use bytes::Bytes;

use crate::Result;
use crate::profile_metrics::ResponseProfile;

/// Streaming HTTP response that completes the `all_` profile metric when its
/// body reaches EOF.
///
/// Read the body with [`Response::bytes`], [`Response::text`], or repeated
/// [`Response::chunk`] calls. Dropping a response before EOF records a failed
/// request, because api-commons treats body read failures as HTTP failures.
pub struct Response {
    inner: Option<reqwest::Response>,
    profile: ResponseProfile,
}

impl Response {
    #[cfg(feature = "metrics")]
    pub(crate) fn new(inner: reqwest::Response, profile: ResponseProfile) -> Self {
        Self {
            inner: Some(inner),
            profile,
        }
    }

    #[cfg(not(feature = "metrics"))]
    pub(crate) fn new_unprofiled(inner: reqwest::Response) -> Self {
        Self {
            inner: Some(inner),
            profile: ResponseProfile,
        }
    }

    /// Buffers the complete response body.
    ///
    /// # Errors
    ///
    /// Returns an error when the body cannot be read or decoded.
    pub async fn bytes(mut self) -> Result<Bytes> {
        let response = self.take_inner();
        let result = response.bytes().await;
        self.profile.finish(result.is_ok());
        result
    }

    /// Buffers and decodes the complete response body as text.
    ///
    /// # Errors
    ///
    /// Returns an error when the body cannot be read or decoded.
    pub async fn text(mut self) -> Result<String> {
        let response = self.take_inner();
        let result = response.text().await;
        self.profile.finish(result.is_ok());
        result
    }

    /// Buffers and decodes the complete response body as JSON.
    ///
    /// # Errors
    ///
    /// Returns an error when the body cannot be read or deserialized.
    #[cfg(feature = "json")]
    pub async fn json<T>(mut self) -> Result<T>
    where
        T: serde::de::DeserializeOwned,
    {
        let response = self.take_inner();
        let result = response.json().await;
        self.profile.finish(result.is_ok());
        result
    }

    /// Reads the next response-body chunk. A `None` result completes metrics
    /// successfully; an error completes them as failed.
    ///
    /// # Errors
    ///
    /// Returns an error when the next body chunk cannot be read.
    pub async fn chunk(&mut self) -> Result<Option<Bytes>> {
        let result = self.inner_mut().chunk().await;
        match &result {
            Ok(None) => self.profile.finish(true),
            Err(_) => self.profile.finish(false),
            Ok(Some(_)) => {}
        }
        result
    }

    /// Returns an error for HTTP 4xx/5xx while preserving api-commons metrics
    /// semantics: receiving an HTTP status is a successful transport request.
    ///
    /// # Errors
    ///
    /// Returns a reqwest status error for HTTP 4xx/5xx.
    pub fn error_for_status(mut self) -> Result<Self> {
        if let Err(error) = self.inner().error_for_status_ref() {
            self.profile.finish(true);
            return Err(error);
        }
        Ok(self)
    }

    /// Escapes to raw reqwest and completes profiling at response headers.
    /// Prefer the body methods above when `all_` timing must include the body.
    #[must_use]
    pub fn into_reqwest(mut self) -> reqwest::Response {
        self.profile.finish(true);
        self.take_inner()
    }

    fn inner(&self) -> &reqwest::Response {
        self.inner
            .as_ref()
            .expect("Breeze response body cannot be consumed twice")
    }

    fn inner_mut(&mut self) -> &mut reqwest::Response {
        self.inner
            .as_mut()
            .expect("Breeze response body cannot be consumed twice")
    }

    fn take_inner(&mut self) -> reqwest::Response {
        self.inner
            .take()
            .expect("Breeze response body cannot be consumed twice")
    }
}

impl Deref for Response {
    type Target = reqwest::Response;

    fn deref(&self) -> &Self::Target {
        self.inner()
    }
}

impl DerefMut for Response {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.inner_mut()
    }
}

impl fmt::Debug for Response {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.inner().fmt(formatter)
    }
}

impl From<Response> for reqwest::Response {
    fn from(response: Response) -> Self {
        response.into_reqwest()
    }
}
