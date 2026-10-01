use std::{
    io::{BufRead, BufReader, Read, Write},
    net::TcpListener,
    sync::mpsc,
    thread,
    time::Duration,
};

use domain::secret::Secret;
use infrastructure::{config::OtlpConfig, telemetry::Otlp};
use opentelemetry::{
    metrics::MeterProvider,
    trace::{Tracer, TracerProvider},
};

/// A collector that answers `200` to every request and reports its request line and headers.
fn collector() -> (String, mpsc::Receiver<Vec<String>>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = format!("http://{}", listener.local_addr().unwrap());
    let (sender, receiver) = mpsc::channel();
    thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { return };
            let mut reader = BufReader::new(stream.try_clone().unwrap());
            let mut head = Vec::new();
            let mut length = 0;
            loop {
                let mut line = String::new();
                if reader.read_line(&mut line).unwrap_or(0) == 0 {
                    break;
                }
                let line = line.trim_end().to_owned();
                if line.is_empty() {
                    break;
                }
                if let Some(value) = line.to_ascii_lowercase().strip_prefix("content-length:") {
                    length = value.trim().parse().unwrap_or(0);
                }
                head.push(line);
            }
            let mut body = vec![0; length];
            let _ = reader.read_exact(&mut body);
            let _ = stream.write_all(b"HTTP/1.1 200 OK\r\ncontent-length: 0\r\n\r\n");
            if sender.send(head).is_err() {
                return;
            }
        }
    });
    (address, receiver)
}

#[test]
fn spans_and_metrics_reach_the_collector_with_the_configured_headers() {
    let (address, requests) = collector();
    let otlp = Otlp::start(&OtlpConfig {
        endpoint: address.parse().unwrap(),
        headers: vec![("x-api-key".to_owned(), Secret::new("k3y"))],
        service_name: "test".to_owned(),
        sample_ratio: 1.0,
    })
    .unwrap();

    otlp.tracer_provider.tracer("test").in_span("work", |_| {});
    otlp.meter_provider
        .meter("test")
        .u64_counter("things")
        .build()
        .add(1, &[]);
    otlp.shutdown();

    let mut paths = Vec::new();
    while let Ok(head) = requests.recv_timeout(Duration::from_secs(5)) {
        assert!(
            head.iter()
                .any(|line| line.eq_ignore_ascii_case("x-api-key: k3y")),
            "{head:?}"
        );
        paths.push(head[0].clone());
        if paths.len() == 2 {
            break;
        }
    }
    paths.sort();
    assert_eq!(
        paths,
        ["POST /v1/metrics HTTP/1.1", "POST /v1/traces HTTP/1.1"]
    );
}

#[test]
fn shutting_down_without_a_collector_neither_fails_nor_hangs() {
    // Nothing listens on a port that was just freed.
    let address = {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        format!("http://{}", listener.local_addr().unwrap())
    };
    let otlp = Otlp::start(&OtlpConfig {
        endpoint: address.parse().unwrap(),
        headers: Vec::new(),
        service_name: "test".to_owned(),
        sample_ratio: 1.0,
    })
    .unwrap();
    otlp.tracer_provider.tracer("test").in_span("work", |_| {});

    let (done, finished) = mpsc::channel();
    thread::spawn(move || {
        otlp.shutdown();
        let _ = done.send(());
    });
    assert!(finished.recv_timeout(Duration::from_secs(20)).is_ok());
}
