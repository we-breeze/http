//! Pooled asynchronous HTTP client for Breeze services.
//!
//! A [`Client`] owns one reqwest connection pool. Create one client for each
//! transport policy (TLS identity, proxy, connect timeout, redirect and pool
//! settings), then share it across hosts. Use [`Endpoint`] for stable routes:
//! besides avoiding metric registration on the request path, it prevents query
//! values from becoming profile metric names.

mod client;
mod endpoint;
mod profile_metrics;
mod request;
mod response;

pub use client::{Client, ClientBuilder, DEFAULT_CONNECT_TIMEOUT, DEFAULT_READ_TIMEOUT};
pub use endpoint::Endpoint;
pub use request::{Request, RequestBuilder};
pub use response::Response;

pub use reqwest;
pub use reqwest::{Body, Error, Method, StatusCode, Url, Version};

/// Result returned by this SDK.
pub type Result<T> = std::result::Result<T, Error>;
