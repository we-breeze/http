use std::time::Duration;

use brz_http::{Client, DEFAULT_CONNECT_TIMEOUT, DEFAULT_READ_TIMEOUT};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

async fn read_request_head(stream: &mut TcpStream, pending: &mut Vec<u8>) -> String {
    loop {
        if let Some(end) = pending.windows(4).position(|window| window == b"\r\n\r\n") {
            let head = pending.drain(..end + 4).collect::<Vec<_>>();
            return String::from_utf8(head).unwrap();
        }

        let mut buffer = [0_u8; 1024];
        let read = stream.read(&mut buffer).await.unwrap();
        assert_ne!(read, 0, "client closed before sending the complete request");
        pending.extend_from_slice(&buffer[..read]);
    }
}

#[tokio::test]
async fn endpoints_on_one_client_reuse_the_origin_pool() {
    assert_eq!(DEFAULT_CONNECT_TIMEOUT, Duration::from_millis(400));
    assert_eq!(DEFAULT_READ_TIMEOUT, Duration::from_millis(400));

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        // Intentionally accept only one socket. The second request must reuse
        // it or the test times out waiting for a response.
        let (mut stream, _) = listener.accept().await.unwrap();
        let mut pending = Vec::new();
        for path in ["/first", "/second"] {
            let request = read_request_head(&mut stream, &mut pending).await;
            assert!(request.starts_with(&format!("GET {path} ")));
            stream
                .write_all(
                    b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: keep-alive\r\n\r\nok",
                )
                .await
                .unwrap();
        }
    });

    let client = Client::builder()
        .timeout(Duration::from_secs(1))
        .configure(reqwest::ClientBuilder::no_proxy)
        .build()
        .unwrap();
    let first = client.endpoint(format!("http://{address}/first")).unwrap();
    let second = client.endpoint(format!("http://{address}/second")).unwrap();

    assert_eq!(
        first.get().send().await.unwrap().bytes().await.unwrap(),
        "ok"
    );
    let request = second.get().build().unwrap();
    assert_eq!(
        client.execute(request).await.unwrap().text().await.unwrap(),
        "ok"
    );
    server.await.unwrap();
}

#[cfg(feature = "metrics")]
#[tokio::test]
async fn request_records_only_the_body_complete_metric() {
    use brz_metrics::Metric;

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        let mut pending = Vec::new();
        let request = read_request_head(&mut stream, &mut pending).await;
        assert!(request.starts_with("GET /status?id=42 "));
        stream
            .write_all(
                b"HTTP/1.1 503 Service Unavailable\r\nContent-Length: 7\r\nConnection: close\r\n\r\n",
            )
            .await
            .unwrap();
        tokio::time::sleep(Duration::from_millis(80)).await;
        stream.write_all(b"failure").await.unwrap();
    });

    let metric_name = format!(
        "http://breeze-http-profile-test-{}/status",
        std::process::id()
    );
    let client = Client::builder()
        .configure(reqwest::ClientBuilder::no_proxy)
        .build()
        .unwrap();
    let endpoint = client
        .endpoint_named(
            format!("http://{address}/status?id=42"),
            metric_name.clone(),
        )
        .unwrap();
    let complete_metric = Metric::http_all(&metric_name);
    let response = endpoint.get().send().await.unwrap();
    assert_eq!(response.status(), 503);
    assert_eq!(complete_metric.snapshot().total, 0);
    assert_eq!(response.text().await.unwrap(), "failure");

    let legacy_name = format!("all_{metric_name}");
    let mut matching_names = Vec::new();
    brz_metrics::visit(|name, kind, _| {
        if kind == "HTTP" && (name == metric_name || name == legacy_name) {
            matching_names.push(name.to_owned());
        }
    });
    assert_eq!(
        matching_names.as_slice(),
        std::slice::from_ref(&metric_name)
    );

    let complete_snapshot = complete_metric.snapshot();
    assert_eq!(complete_snapshot.total, 1);
    assert_eq!(complete_snapshot.success, 1);
    assert_eq!(complete_snapshot.failure, 0);
    assert!(
        complete_snapshot.elapsed_ns >= 50_000_000,
        "HTTP timing must include response-body consumption"
    );
    server.await.unwrap();
}
