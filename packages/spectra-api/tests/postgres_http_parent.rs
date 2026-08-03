use spectra_api::server::{Handler, HttpServer, ServerConfig, ServerResponse};
use spectra_db::postgres::{PostgresConfig, PostgresConnection};
use spectra_runtime::tracing::{self, SpanKind};
use std::env;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::sync::Arc;

#[test]
#[ignore = "requires PostgreSQL 16 and an external OTLP collector"]
fn postgres_span_is_child_of_real_http_server_span() {
    let url = env::var("SPECTRA_POSTGRES_URL").expect("SPECTRA_POSTGRES_URL");
    let endpoint = env::var("SPECTRA_R2505_OTLP_ENDPOINT").expect("SPECTRA_R2505_OTLP_ENDPOINT");
    let config = tracing::config_new(&endpoint, "spectralang-r2505").unwrap();
    tracing::config_start(config).unwrap();
    let postgres = PostgresConfig::from_url(&url).unwrap();
    let connection = PostgresConnection::open(postgres).unwrap();
    let handler: Handler = Arc::new(move |_| {
        let success = connection.health_check().is_ok();
        ServerResponse::text(if success { 200 } else { 500 }, "postgres")
    });
    let mut server = HttpServer::start(ServerConfig::default(), handler).unwrap();
    let root = tracing::begin_external_span(SpanKind::Client, "test.request").unwrap();
    let traceparent = tracing::inject(root).unwrap();
    let mut stream = TcpStream::connect(server.local_addr()).unwrap();
    write!(stream, "GET /postgres HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\ntraceparent: {traceparent}\r\n\r\n").unwrap();
    let mut response = String::new();
    stream.read_to_string(&mut response).unwrap();
    let _ = tracing::span_end(root);
    server.shutdown().unwrap();
    tracing::flush().unwrap();
    tracing::config_shutdown(config).unwrap();
    assert!(response.starts_with("HTTP/1.1 200"), "{response}");
}
