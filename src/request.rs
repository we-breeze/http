use std::fmt;
use std::ops::{Deref, DerefMut};
use std::time::Duration;

use http_types::Error as HttpError;
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use reqwest::{Body, Version};
use serde::Serialize;

use crate::profile_metrics::ProfileMetrics;
use crate::{Response, Result};

/// A request builder that preserves HTTP profile instrumentation.
#[must_use = "request builders do nothing until sent"]
pub struct RequestBuilder {
    inner: reqwest::RequestBuilder,
    profile: ProfileMetrics,
}

impl RequestBuilder {
    pub(crate) fn new(inner: reqwest::RequestBuilder, profile: ProfileMetrics) -> Self {
        Self { inner, profile }
    }

    pub fn header<K, V>(mut self, key: K, value: V) -> Self
    where
        HeaderName: TryFrom<K>,
        <HeaderName as TryFrom<K>>::Error: Into<HttpError>,
        HeaderValue: TryFrom<V>,
        <HeaderValue as TryFrom<V>>::Error: Into<HttpError>,
    {
        self.inner = self.inner.header(key, value);
        self
    }

    pub fn headers(mut self, headers: HeaderMap) -> Self {
        self.inner = self.inner.headers(headers);
        self
    }

    pub fn basic_auth<U, P>(mut self, username: U, password: Option<P>) -> Self
    where
        U: fmt::Display,
        P: fmt::Display,
    {
        self.inner = self.inner.basic_auth(username, password);
        self
    }

    pub fn bearer_auth<T>(mut self, token: T) -> Self
    where
        T: fmt::Display,
    {
        self.inner = self.inner.bearer_auth(token);
        self
    }

    pub fn body<T>(mut self, body: T) -> Self
    where
        T: Into<Body>,
    {
        self.inner = self.inner.body(body);
        self
    }

    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.inner = self.inner.timeout(timeout);
        self
    }

    pub fn query<T>(mut self, query: &T) -> Self
    where
        T: Serialize + ?Sized,
    {
        self.inner = self.inner.query(query);
        self
    }

    pub fn form<T>(mut self, form: &T) -> Self
    where
        T: Serialize + ?Sized,
    {
        self.inner = self.inner.form(form);
        self
    }

    #[cfg(feature = "json")]
    pub fn json<T>(mut self, json: &T) -> Self
    where
        T: Serialize + ?Sized,
    {
        self.inner = self.inner.json(json);
        self
    }

    pub fn version(mut self, version: Version) -> Self {
        self.inner = self.inner.version(version);
        self
    }

    /// Applies an arbitrary reqwest request-builder transformation without
    /// losing this SDK's profile instrumentation.
    pub fn configure<F>(mut self, configure: F) -> Self
    where
        F: FnOnce(reqwest::RequestBuilder) -> reqwest::RequestBuilder,
    {
        self.inner = configure(self.inner);
        self
    }

    /// Builds a request that retains its profile handles for [`Client::execute`](crate::Client::execute).
    ///
    /// # Errors
    ///
    /// Returns an error when a URL, header, or serialized request option is invalid.
    pub fn build(self) -> Result<Request> {
        self.inner
            .build()
            .map(|inner| Request::new(inner, self.profile))
    }

    /// Sends one logical request. The endpoint timing is captured at response
    /// headers and the `all_` timing completes when the response body reaches EOF.
    /// Transport/body errors and cancellation are failures. HTTP status codes
    /// alone do not mark the transport request as failed, matching api-commons.
    ///
    /// # Errors
    ///
    /// Returns a reqwest error when building or sending the request fails.
    pub async fn send(self) -> Result<Response> {
        self.profile.send(self.inner).await
    }

    pub fn try_clone(&self) -> Option<Self> {
        Some(Self {
            inner: self.inner.try_clone()?,
            profile: self.profile,
        })
    }
}

/// A built reqwest request carrying its stable profile handles.
pub struct Request {
    inner: reqwest::Request,
    profile: ProfileMetrics,
}

impl Request {
    fn new(inner: reqwest::Request, profile: ProfileMetrics) -> Self {
        Self { inner, profile }
    }

    pub fn try_clone(&self) -> Option<Self> {
        Some(Self {
            inner: self.inner.try_clone()?,
            profile: self.profile,
        })
    }

    /// Removes the instrumentation wrapper. Executing the returned request
    /// through raw reqwest will not produce Breeze HTTP metrics.
    pub fn into_reqwest(self) -> reqwest::Request {
        self.inner
    }

    pub(crate) fn into_parts(self) -> (reqwest::Request, ProfileMetrics) {
        (self.inner, self.profile)
    }
}

impl Deref for Request {
    type Target = reqwest::Request;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl DerefMut for Request {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}
