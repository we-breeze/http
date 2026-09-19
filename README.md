# Breeze HTTP

High-performance asynchronous HTTP client for Breeze services. It is a thin
instrumented facade over `reqwest`: clients share reqwest's connection pool,
HTTP/2 implementation, DNS, and rustls transport instead of maintaining a
second transport stack.

## Ownership

Create one `Client` per transport policy, not one per host. Client-wide policy
includes TLS identity and roots, proxy, redirects, connect timeout, default
headers, DNS behavior, and pool settings. The same client can safely serve many
hosts; reqwest partitions pooled connections by origin.

```rust,no_run
use std::time::Duration;

use brz_http::Client;

# async fn example() -> Result<(), brz_http::Error> {
let client = Client::builder()
    .connect_timeout(Duration::from_secs(2))
    .read_timeout(Duration::from_secs(6))
    .pool_idle_timeout(Duration::from_secs(90))
    .pool_max_idle_per_host(16)
    .build()?;

let config = client.endpoint("http://config.example.com/api/config")?;
let response = config
    .get()
    .query(&[("service", "abtest")])
    .send()
    .await?;

let body = response.bytes().await?;
# let _ = body;
# Ok(())
# }
```

`Client` and `Endpoint` are cheap to clone. Keep them in application state and
reuse them. Use different clients only when their client-wide policies differ.
Per-request total timeout can be supplied through `RequestBuilder::timeout`.

The default transport timeouts match api-commons `ApacheHttpClient()`:

- connect timeout: 400 ms;
- per-read timeout: 400 ms, reset after each successful socket read;
- no total request timeout.

api-commons also waits at most 400 ms for its bounded connection pool. Reqwest
does not expose the same active-connection lease model, so Breeze does not
pretend `pool_max_idle_per_host` is an equivalent active-connection limit.

For an uncommon reqwest option, `ClientBuilder::configure` and
`RequestBuilder::configure` preserve the Breeze wrapper:

```rust,no_run
# use brz_http::Client;
let client = Client::builder()
    .configure(|builder| builder.redirect(brz_http::reqwest::redirect::Policy::none()))
    .build()?;
# Ok::<(), brz_http::Error>(())
```

## Metrics

Enable the optional feature in the consuming crate:

```toml
brz-http = "0.0.5"
```

Creating an `Endpoint` eagerly registers one ProfileUtil-compatible row:

- `HTTP`, name `<scheme>://<host>/<path>`, slow threshold 200 ms.

Query parameters, fragments, and URL credentials are excluded from the default
metric name. Use `Client::endpoint_named` for routes with dynamic path values.
The endpoint stores direct metric handles, so the send path performs no metric
registry lookup and allocates no metrics metadata. Recording adds relaxed
atomic counter updates only. Transport errors, body-read errors, and cancelled
requests increment `error_count`.

Timing includes response-body consumption. HTTP status codes alone do not fail
the metric when the body is consumed normally. Dropping a body before EOF,
calling `error_for_status` on a 4xx/5xx response, escaping to raw reqwest, or
encountering a body-read error records failure because completion cannot be
observed.

The `metrics` feature is opt-in. Without it, the metrics dependency and all
timing/counter work compile out.

Enable `slow-log` to emit requests taking at least 1 second to `breeze.slow`.
URL credentials, query parameters, and fragments are omitted; in-memory
request bodies are capped at 2 KiB. Lines contain component, method, URL,
status, elapsed time, request-body length, success, and finally the request body.

## Releases

CI runs formatting, Clippy, and tests. To publish, open **Actions → Publish → Run workflow** on `main`. Leave `retry_tag` empty to allocate the next `v0.0.x` tag. The workflow validates the code, commits the version, pushes the commit and tag atomically, and publishes to crates.io using the organization secret `CARGO_REGISTRY_TOKEN`.

If publication fails after the tag was pushed, rerun with that existing tag in `retry_tag`. A normal push or pull request does not publish. Historical tags retain their original version numbers; use new release tags for registry packages.

## License

Licensed under either [MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE), at your option.

## Crate naming

The package name is `brz-http`; the Rust library name is `brz_http`.
Use `brz_http::...` in Rust code. This replaces the previous `http`
library name. Existing explicit dependency aliases remain supported.

```toml
[dependencies]
brz-http = "0.0.5"
```
