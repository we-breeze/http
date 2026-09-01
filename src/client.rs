use std::time::Duration;

use reqwest::header::HeaderMap;
use reqwest::{IntoUrl, Method};

use crate::endpoint::Endpoint;
use crate::profile_metrics::ProfileMetrics;
use crate::request::{Request, RequestBuilder};
use crate::{Response, Result};

/// api-commons `ApacheHttpClient()` connection timeout.
pub const DEFAULT_CONNECT_TIMEOUT: Duration = Duration::from_millis(400);

/// api-commons `ApacheHttpClient()` per-read socket timeout.
pub const DEFAULT_READ_TIMEOUT: Duration = Duration::from_millis(400);

/// A cheap-to-clone handle to one shared reqwest connection pool.
///
/// The pool already partitions connections by origin. A separate client is
/// useful only when transport-wide policy differs; it is not needed for every
/// host.
#[derive(Clone, Debug)]
pub struct Client {
    inner: reqwest::Client,
}

impl Client {
    /// Builds a client with Breeze's api-commons-compatible timeout defaults.
    ///
    /// Prefer [`Client::builder`] when the application has an explicit timeout
    /// or pool policy.
    ///
    /// # Panics
    ///
    /// Panics if the compiled TLS or resolver backend cannot construct its
    /// default configuration. Use [`Client::builder`] to handle that error.
    #[must_use]
    pub fn new() -> Self {
        Self::builder()
            .build()
            .expect("default Breeze HTTP client configuration must be valid")
    }

    pub fn builder() -> ClientBuilder {
        ClientBuilder::new()
    }

    /// Returns an endpoint whose default profile name is its URL without
    /// credentials, query parameters, or fragment.
    ///
    /// # Errors
    ///
    /// Returns an error when `url` is not a valid HTTP or HTTPS URL.
    pub fn endpoint<U>(&self, url: U) -> Result<Endpoint>
    where
        U: IntoUrl,
    {
        Ok(Endpoint::new(self.clone(), url.into_url()?, None))
    }

    /// Returns an endpoint with an explicit stable profile name.
    ///
    /// Use this for routes containing identifiers or other high-cardinality
    /// path components.
    ///
    /// # Errors
    ///
    /// Returns an error when `url` is not a valid HTTP or HTTPS URL.
    pub fn endpoint_named<U, N>(&self, url: U, profile_name: N) -> Result<Endpoint>
    where
        U: IntoUrl,
        N: Into<Box<str>>,
    {
        Ok(Endpoint::new(
            self.clone(),
            url.into_url()?,
            Some(profile_name.into()),
        ))
    }

    /// Creates a request for a dynamic URL.
    ///
    /// With the `metrics` feature this registers the URL metric while building
    /// the request. Reuse an [`Endpoint`] when the route is known in advance.
    ///
    /// # Errors
    ///
    /// Returns an error when `url` is not a valid HTTP or HTTPS URL.
    pub fn request<U>(&self, method: Method, url: U) -> Result<RequestBuilder>
    where
        U: IntoUrl,
    {
        let url = url.into_url()?;
        let profile = ProfileMetrics::for_url(&url);
        Ok(RequestBuilder::new(
            self.inner.request(method, url),
            profile,
        ))
    }

    /// Creates a profiled `GET` request.
    ///
    /// # Errors
    ///
    /// Returns an error when `url` is not a valid HTTP or HTTPS URL.
    pub fn get<U>(&self, url: U) -> Result<RequestBuilder>
    where
        U: IntoUrl,
    {
        self.request(Method::GET, url)
    }

    /// Creates a profiled `POST` request.
    ///
    /// # Errors
    ///
    /// Returns an error when `url` is not a valid HTTP or HTTPS URL.
    pub fn post<U>(&self, url: U) -> Result<RequestBuilder>
    where
        U: IntoUrl,
    {
        self.request(Method::POST, url)
    }

    /// Creates a profiled `PUT` request.
    ///
    /// # Errors
    ///
    /// Returns an error when `url` is not a valid HTTP or HTTPS URL.
    pub fn put<U>(&self, url: U) -> Result<RequestBuilder>
    where
        U: IntoUrl,
    {
        self.request(Method::PUT, url)
    }

    /// Creates a profiled `DELETE` request.
    ///
    /// # Errors
    ///
    /// Returns an error when `url` is not a valid HTTP or HTTPS URL.
    pub fn delete<U>(&self, url: U) -> Result<RequestBuilder>
    where
        U: IntoUrl,
    {
        self.request(Method::DELETE, url)
    }

    /// Executes a previously built profiled request through this client's pool.
    ///
    /// # Errors
    ///
    /// Returns a reqwest error when request execution fails.
    pub async fn execute(&self, request: Request) -> Result<Response> {
        let (request, profile) = request.into_parts();
        profile.execute(&self.inner, request).await
    }

    /// Accesses the underlying reqwest client for an integration that the
    /// wrapper does not expose. Requests sent through it are not profiled.
    #[must_use]
    pub fn as_reqwest(&self) -> &reqwest::Client {
        &self.inner
    }

    #[must_use]
    pub fn into_reqwest(self) -> reqwest::Client {
        self.inner
    }
}

impl Default for Client {
    fn default() -> Self {
        Self::new()
    }
}

impl From<reqwest::Client> for Client {
    fn from(inner: reqwest::Client) -> Self {
        Self { inner }
    }
}

impl AsRef<reqwest::Client> for Client {
    fn as_ref(&self) -> &reqwest::Client {
        &self.inner
    }
}

/// Builder for one shared transport policy and connection pool.
#[must_use]
pub struct ClientBuilder {
    inner: reqwest::ClientBuilder,
}

impl ClientBuilder {
    pub fn new() -> Self {
        Self {
            inner: reqwest::Client::builder()
                .connect_timeout(DEFAULT_CONNECT_TIMEOUT)
                .read_timeout(DEFAULT_READ_TIMEOUT),
        }
    }

    pub fn connect_timeout(mut self, timeout: Duration) -> Self {
        self.inner = self.inner.connect_timeout(timeout);
        self
    }

    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.inner = self.inner.timeout(timeout);
        self
    }

    /// Sets the timeout for each individual socket read. A successful read
    /// resets this timeout.
    pub fn read_timeout(mut self, timeout: Duration) -> Self {
        self.inner = self.inner.read_timeout(timeout);
        self
    }

    pub fn pool_idle_timeout<D>(mut self, timeout: D) -> Self
    where
        D: Into<Option<Duration>>,
    {
        self.inner = self.inner.pool_idle_timeout(timeout);
        self
    }

    pub fn pool_max_idle_per_host(mut self, max: usize) -> Self {
        self.inner = self.inner.pool_max_idle_per_host(max);
        self
    }

    pub fn tcp_keepalive<D>(mut self, keepalive: D) -> Self
    where
        D: Into<Option<Duration>>,
    {
        self.inner = self.inner.tcp_keepalive(keepalive);
        self
    }

    pub fn tcp_nodelay(mut self, enabled: bool) -> Self {
        self.inner = self.inner.tcp_nodelay(enabled);
        self
    }

    pub fn default_headers(mut self, headers: HeaderMap) -> Self {
        self.inner = self.inner.default_headers(headers);
        self
    }

    /// Applies any reqwest builder option not mirrored by this facade.
    pub fn configure<F>(mut self, configure: F) -> Self
    where
        F: FnOnce(reqwest::ClientBuilder) -> reqwest::ClientBuilder,
    {
        self.inner = configure(self.inner);
        self
    }

    /// Builds the shared client and connection pool.
    ///
    /// # Errors
    ///
    /// Returns an error when a TLS, proxy, resolver, or header option is invalid.
    pub fn build(self) -> Result<Client> {
        self.inner.build().map(Client::from)
    }
}

impl Default for ClientBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl From<Client> for reqwest::Client {
    fn from(client: Client) -> Self {
        client.inner
    }
}
