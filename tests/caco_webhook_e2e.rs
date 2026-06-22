//! End-to-end integration test for the feedback-cli -> caco webhook path.
//!
//! It stands up a mock caco `bead` webhook receiver on localhost that reads the
//! posted body exactly the way `caco-daemon`'s `dispatch_bead` handler does
//! (`title` / `description` / `type` / `priority` / `labels`), then drives
//! [`feedback_cli::WebhookSink`] in [`feedback_cli::WebhookPayload::CacoBead`]
//! mode and asserts the body maps to the intended bead — proving the contract
//! without a live caco. Standard-library only (no extra deps, no real network).

use std::io::{Read, Write};
use std::net::TcpListener;
use std::time::Duration;

use feedback_cli::{
    FeedbackError, FeedbackEvent, FeedbackSink, Metric, WebhookConfig, WebhookPayload, WebhookSink,
};

/// Serve `count` sequential requests, returning each full request text (headers
/// + body). Every request is answered with `status_line` and an empty body.
fn spawn_mock_caco_bead_hook(
    count: usize,
    status_line: &'static str,
) -> (String, std::thread::JoinHandle<Vec<String>>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind ephemeral port");
    let addr = listener.local_addr().expect("local addr");
    let url = format!("http://{addr}/hooks/proj/feedback");
    let handle = std::thread::spawn(move || {
        let mut requests = Vec::new();
        for _ in 0..count {
            let Ok((mut stream, _)) = listener.accept() else {
                break;
            };
            stream.set_read_timeout(Some(Duration::from_secs(5))).ok();
            let mut data = Vec::new();
            let mut tmp = [0u8; 1024];
            loop {
                match stream.read(&mut tmp) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => {
                        data.extend_from_slice(&tmp[..n]);
                        let text = String::from_utf8_lossy(&data);
                        if let Some(hdr_end) = text.find("\r\n\r\n") {
                            let body_len = text[..hdr_end]
                                .lines()
                                .find_map(|line| {
                                    let (name, value) = line.split_once(':')?;
                                    if name.trim().eq_ignore_ascii_case("content-length") {
                                        value.trim().parse::<usize>().ok()
                                    } else {
                                        None
                                    }
                                })
                                .unwrap_or(0);
                            if data.len() >= hdr_end + 4 + body_len {
                                break;
                            }
                        }
                    }
                }
            }
            requests.push(String::from_utf8_lossy(&data).to_string());
            let response =
                format!("{status_line}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n");
            let _ = stream.write_all(response.as_bytes());
            let _ = stream.flush();
        }
        requests
    });
    (url, handle)
}

/// Split a captured HTTP request into (headers, body).
fn split_request(request: &str) -> (&str, &str) {
    match request.split_once("\r\n\r\n") {
        Some((headers, body)) => (headers, body),
        None => (request, ""),
    }
}

#[test]
fn caco_bead_payload_creates_expected_bead_fields() {
    let (url, handle) = spawn_mock_caco_bead_hook(2, "HTTP/1.1 200 OK");

    let sink = WebhookSink::from_config(&WebhookConfig {
        url: url.clone(),
        token: Some("hook-tok".to_owned()),
        payload: WebhookPayload::CacoBead,
        ..WebhookConfig::default()
    })
    .expect("build webhook sink");

    // An error event -> a `bug` bead.
    sink.record(
        &FeedbackEvent::error("build", "linker failed")
            .with_detail("ld: symbol not found")
            .with_field("crate", "acme")
            .with_label("ci"),
    )
    .expect("error event delivered");

    // A perf event -> a `task` bead.
    sink.record(&FeedbackEvent::perf(
        "build",
        "slow link",
        Metric::new("link_ms", 4200.0),
    ))
    .expect("perf event delivered");

    let requests = handle.join().expect("server thread");
    assert_eq!(requests.len(), 2);

    // Request 1: error -> bug bead, the caco bead handler's fields are present.
    let (headers, body) = split_request(&requests[0]);
    assert!(headers.starts_with("POST /hooks/proj/feedback"));
    assert!(
        headers.contains("Authorization: Bearer hook-tok"),
        "missing/incorrect auth header: {headers}"
    );
    assert!(headers.contains("Content-Type: application/json"));
    assert!(body.contains(r#""title":"linker failed""#), "body: {body}");
    assert!(body.contains(r#""type":"bug""#), "body: {body}");
    assert!(body.contains(r#""priority":2"#), "body: {body}");
    assert!(
        body.contains(r#""labels":["feedback","kind:error","ci"]"#),
        "body: {body}"
    );
    // The structured description carries detail + context (component, field).
    assert!(body.contains("ld: symbol not found"), "body: {body}");
    assert!(body.contains("component: build"), "body: {body}");
    assert!(body.contains("crate: acme"), "body: {body}");

    // Request 2: perf -> task bead.
    let (_, body) = split_request(&requests[1]);
    assert!(body.contains(r#""title":"slow link""#), "body: {body}");
    assert!(body.contains(r#""type":"task""#), "body: {body}");
}

#[test]
fn event_payload_posts_native_feedback_event() {
    // The default payload mode posts the native FeedbackEvent JSON, which a caco
    // hook maps with `bead.title_from = "summary"`.
    let (url, handle) = spawn_mock_caco_bead_hook(1, "HTTP/1.1 200 OK");
    let sink = WebhookSink::from_config(&WebhookConfig {
        url,
        token: Some("t".to_owned()),
        // payload defaults to Event
        ..WebhookConfig::default()
    })
    .expect("build webhook sink");
    sink.record(&FeedbackEvent::error("svc", "boom"))
        .expect("delivered");
    let requests = handle.join().expect("server thread");
    let (_, body) = split_request(&requests[0]);
    assert!(body.contains(r#""kind":"error""#), "body: {body}");
    assert!(body.contains(r#""summary":"boom""#), "body: {body}");
}

#[test]
fn non_2xx_surfaces_as_http_error() {
    let (url, handle) = spawn_mock_caco_bead_hook(1, "HTTP/1.1 500 Internal Server Error");
    let sink = WebhookSink::from_config(&WebhookConfig {
        url,
        payload: WebhookPayload::CacoBead,
        ..WebhookConfig::default()
    })
    .expect("build webhook sink");
    let err = sink
        .record(&FeedbackEvent::error("svc", "boom"))
        .expect_err("500 must surface as an error");
    assert!(matches!(err, FeedbackError::Http(_)));
    let _ = handle.join();
}
