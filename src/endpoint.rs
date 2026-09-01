use reqwest::{Method, Url};

use crate::client::Client;
use crate::profile_metrics::ProfileMetrics;
use crate::request::RequestBuilder;

/// A stable HTTP route backed by a shared [`Client`] connection pool.
///
/// Cloning an endpoint is cheap. Its metrics are registered once during
/// construction and copied directly into each request builder.
#[derive(Clone, Debug)]
pub struct Endpoint {
    client: Client,
    url: Url,
    profile: ProfileMetrics,
}

impl Endpoint {
    pub(crate) fn new(client: Client, url: Url, profile_name: Option<Box<str>>) -> Self {
        let profile = match profile_name {
            Some(name) => ProfileMetrics::for_name(&name),
            None => ProfileMetrics::for_url(&url),
        };
        Self {
            client,
            url,
            profile,
        }
    }

    #[must_use]
    pub fn url(&self) -> &Url {
        &self.url
    }

    #[must_use]
    pub fn client(&self) -> &Client {
        &self.client
    }

    pub fn request(&self, method: Method) -> RequestBuilder {
        RequestBuilder::new(
            self.client.as_reqwest().request(method, self.url.clone()),
            self.profile,
        )
    }

    pub fn get(&self) -> RequestBuilder {
        self.request(Method::GET)
    }

    pub fn post(&self) -> RequestBuilder {
        self.request(Method::POST)
    }

    pub fn put(&self) -> RequestBuilder {
        self.request(Method::PUT)
    }

    pub fn delete(&self) -> RequestBuilder {
        self.request(Method::DELETE)
    }
}
