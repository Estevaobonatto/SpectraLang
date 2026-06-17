use crate::http::{
    BodyChunk, Header, Http1Parser, HttpBody, HttpVersion, ParseErrorKind, ParsedRequest,
    ParsedResponse, ParserConfig,
};
use crate::{read_args, write_result};
use spectra_runtime::ffi::{
    SpectraHostCallContext, SpectraHostValue, HOST_STATUS_INVALID_ARGUMENT,
};
use std::collections::HashMap;
use std::fmt;
use std::io::{Read, Write};
use std::net::{Shutdown, SocketAddr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

pub const SERVER_STATE_CREATED: SpectraHostValue = 1;
pub const SERVER_STATE_RUNNING: SpectraHostValue = 2;
pub const SERVER_STATE_STOPPED: SpectraHostValue = 3;

const DEFAULT_READ_TIMEOUT_MS: u64 = 5_000;
const DEFAULT_IDLE_TIMEOUT_MS: u64 = 30_000;
const DEFAULT_MAX_CONNECTIONS: usize = 10_000;

pub type Handler = Arc<dyn Fn(ParsedRequest) -> ServerResponse + Send + Sync + 'static>;

#[derive(Clone, Debug)]
pub struct ServerConfig {
    pub bind_addr: SocketAddr,
    pub max_header_bytes: usize,
    pub max_body_bytes: usize,
    pub max_chunk_bytes: usize,
    pub read_timeout: Duration,
    pub idle_timeout: Duration,
    pub max_connections: usize,
    pub poll_interval: Duration,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            bind_addr: "127.0.0.1:0"
                .parse()
                .expect("default server bind address is valid"),
            max_header_bytes: 64 * 1024,
            max_body_bytes: 16 * 1024 * 1024,
            max_chunk_bytes: 8 * 1024 * 1024,
            read_timeout: Duration::from_millis(DEFAULT_READ_TIMEOUT_MS),
            idle_timeout: Duration::from_millis(DEFAULT_IDLE_TIMEOUT_MS),
            max_connections: DEFAULT_MAX_CONNECTIONS,
            poll_interval: Duration::from_millis(1),
        }
    }
}

impl ServerConfig {
    fn parser_config(&self) -> ParserConfig {
        ParserConfig {
            max_header_bytes: self.max_header_bytes,
            max_body_bytes: self.max_body_bytes,
            max_chunk_bytes: self.max_chunk_bytes,
        }
    }
}

#[derive(Clone, Debug)]
pub struct ServerResponse {
    pub status_code: u16,
    pub reason: String,
    pub headers: Vec<Header>,
    pub body: HttpBody,
    pub close: bool,
}

impl ServerResponse {
    pub fn text(status_code: u16, body: impl Into<String>) -> Self {
        let body = body.into();
        Self {
            status_code,
            reason: reason_for_status(status_code).to_string(),
            headers: vec![Header {
                name: "Content-Type".to_string(),
                value: "text/plain; charset=utf-8".to_string(),
            }],
            body: HttpBody::from_bytes(body.into_bytes()),
            close: false,
        }
    }

    pub fn bytes(status_code: u16, body: Vec<u8>) -> Self {
        Self {
            status_code,
            reason: reason_for_status(status_code).to_string(),
            headers: Vec::new(),
            body: HttpBody::from_bytes(body),
            close: false,
        }
    }

    pub fn chunked(status_code: u16, chunks: Vec<Vec<u8>>) -> Self {
        Self {
            status_code,
            reason: reason_for_status(status_code).to_string(),
            headers: Vec::new(),
            body: HttpBody {
                chunks: chunks
                    .into_iter()
                    .map(|data| BodyChunk {
                        data,
                        extension: None,
                    })
                    .collect(),
                trailers: Vec::new(),
                chunked: true,
            },
            close: false,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ServerStats {
    pub accepted_connections: usize,
    pub completed_requests: usize,
    pub rejected_connections: usize,
    pub body_limit_violations: usize,
    pub timeouts: usize,
    pub parse_errors: usize,
    pub closed_connections: usize,
    pub peak_connections: usize,
    pub active_connections: usize,
}

#[derive(Debug)]
pub enum ServerError {
    Io(std::io::Error),
    AlreadyStopped,
}

impl fmt::Display for ServerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ServerError::Io(error) => write!(f, "server I/O error: {error}"),
            ServerError::AlreadyStopped => write!(f, "server is already stopped"),
        }
    }
}

impl std::error::Error for ServerError {}

impl From<std::io::Error> for ServerError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value)
    }
}

pub struct HttpServer {
    local_addr: SocketAddr,
    shutdown: Arc<AtomicBool>,
    stats: Arc<Mutex<ServerStats>>,
    join: Option<JoinHandle<()>>,
}

impl HttpServer {
    pub fn start(config: ServerConfig, handler: Handler) -> Result<Self, ServerError> {
        let listener = TcpListener::bind(config.bind_addr)?;
        listener.set_nonblocking(true)?;
        let local_addr = listener.local_addr()?;
        let shutdown = Arc::new(AtomicBool::new(false));
        let stats = Arc::new(Mutex::new(ServerStats::default()));
        let loop_shutdown = Arc::clone(&shutdown);
        let loop_stats = Arc::clone(&stats);
        let join = thread::Builder::new()
            .name("spectra-api-http1-server".to_string())
            .spawn(move || run_accept_loop(listener, config, handler, loop_shutdown, loop_stats))?;

        Ok(Self {
            local_addr,
            shutdown,
            stats,
            join: Some(join),
        })
    }

    pub fn local_addr(&self) -> SocketAddr {
        self.local_addr
    }

    pub fn stats(&self) -> ServerStats {
        self.stats.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }

    pub fn shutdown(&mut self) -> Result<ServerStats, ServerError> {
        self.shutdown.store(true, Ordering::SeqCst);
        if let Some(join) = self.join.take() {
            let _ = TcpStream::connect(self.local_addr);
            let _ = join.join();
            return Ok(self.stats());
        }
        Err(ServerError::AlreadyStopped)
    }
}

impl Drop for HttpServer {
    fn drop(&mut self) {
        let _ = self.shutdown();
    }
}

struct Connection {
    stream: TcpStream,
    parser: Http1Parser,
    write_buf: Vec<u8>,
    write_pos: usize,
    accepted_at: Instant,
    last_activity: Instant,
    close_after_write: bool,
}

impl Connection {
    fn new(stream: TcpStream, parser_config: ParserConfig, now: Instant) -> std::io::Result<Self> {
        stream.set_nonblocking(true)?;
        stream.set_nodelay(true)?;
        Ok(Self {
            stream,
            parser: Http1Parser::request_with_config(parser_config),
            write_buf: Vec::new(),
            write_pos: 0,
            accepted_at: now,
            last_activity: now,
            close_after_write: false,
        })
    }

    fn has_pending_write(&self) -> bool {
        self.write_pos < self.write_buf.len()
    }
}

fn run_accept_loop(
    listener: TcpListener,
    config: ServerConfig,
    handler: Handler,
    shutdown: Arc<AtomicBool>,
    stats: Arc<Mutex<ServerStats>>,
) {
    let mut connections = Vec::<Connection>::new();
    let parser_config = config.parser_config();

    while !shutdown.load(Ordering::SeqCst) {
        accept_ready_connections(&listener, &config, &parser_config, &stats, &mut connections);
        service_connections(&config, &handler, &stats, &mut connections);
        thread::sleep(config.poll_interval);
    }

    for connection in connections.drain(..) {
        let _ = connection.stream.shutdown(Shutdown::Both);
        record_close(&stats);
    }
}

fn accept_ready_connections(
    listener: &TcpListener,
    config: &ServerConfig,
    parser_config: &ParserConfig,
    stats: &Arc<Mutex<ServerStats>>,
    connections: &mut Vec<Connection>,
) {
    loop {
        match listener.accept() {
            Ok((stream, _)) => {
                if connections.len() >= config.max_connections {
                    let _ = stream.shutdown(Shutdown::Both);
                    let mut stats = stats.lock().unwrap_or_else(|e| e.into_inner());
                    stats.rejected_connections += 1;
                    continue;
                }
                match Connection::new(stream, parser_config.clone(), Instant::now()) {
                    Ok(connection) => {
                        connections.push(connection);
                        let mut stats = stats.lock().unwrap_or_else(|e| e.into_inner());
                        stats.accepted_connections += 1;
                        stats.active_connections = connections.len();
                        stats.peak_connections = stats.peak_connections.max(connections.len());
                    }
                    Err(_) => {
                        let mut stats = stats.lock().unwrap_or_else(|e| e.into_inner());
                        stats.rejected_connections += 1;
                    }
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => break,
            Err(_) => break,
        }
    }
}

fn service_connections(
    config: &ServerConfig,
    handler: &Handler,
    stats: &Arc<Mutex<ServerStats>>,
    connections: &mut Vec<Connection>,
) {
    let mut idx = 0usize;
    while idx < connections.len() {
        let action = service_connection(config, handler, stats, &mut connections[idx]);
        if action == ConnectionAction::Close {
            let connection = connections.swap_remove(idx);
            let _ = connection.stream.shutdown(Shutdown::Both);
            record_close(stats);
        } else {
            idx += 1;
        }
    }
    stats
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .active_connections = connections.len();
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ConnectionAction {
    Keep,
    Close,
}

fn service_connection(
    config: &ServerConfig,
    handler: &Handler,
    stats: &Arc<Mutex<ServerStats>>,
    connection: &mut Connection,
) -> ConnectionAction {
    if write_pending(connection).is_err() {
        return ConnectionAction::Close;
    }
    if connection.close_after_write && !connection.has_pending_write() {
        return ConnectionAction::Close;
    }

    let now = Instant::now();
    if connection.parser.buffered_len() > 0
        && now.duration_since(connection.last_activity) > config.read_timeout
    {
        record_timeout(stats);
        queue_error_response(connection, 408, "request timeout");
        return if connection.has_pending_write() {
            ConnectionAction::Keep
        } else {
            ConnectionAction::Close
        };
    }
    if connection.parser.buffered_len() == 0
        && now.duration_since(connection.last_activity) > config.idle_timeout
    {
        record_timeout(stats);
        return ConnectionAction::Close;
    }
    if connection.parser.buffered_len() == 0
        && now.duration_since(connection.accepted_at) > config.idle_timeout
    {
        record_timeout(stats);
        return ConnectionAction::Close;
    }

    let mut read_buf = [0_u8; 8192];
    loop {
        match connection.stream.read(&mut read_buf) {
            Ok(0) => return ConnectionAction::Close,
            Ok(n) => {
                connection.last_activity = Instant::now();
                connection.parser.push(&read_buf[..n]);
            }
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => break,
            Err(_) => return ConnectionAction::Close,
        }
    }

    loop {
        match connection.parser.parse_next_request() {
            Ok(Some(request)) => {
                let close = !request.keep_alive;
                let method = request.method.clone();
                let response = handler(request);
                queue_response(connection, response, method == "HEAD", close);
                stats
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .completed_requests += 1;
                if write_pending(connection).is_err() {
                    return ConnectionAction::Close;
                }
                if connection.close_after_write {
                    break;
                }
            }
            Ok(None) => break,
            Err(error) => {
                if error.kind == ParseErrorKind::BodyTooLarge {
                    stats
                        .lock()
                        .unwrap_or_else(|e| e.into_inner())
                        .body_limit_violations += 1;
                    queue_error_response(connection, 413, "payload too large");
                } else {
                    stats.lock().unwrap_or_else(|e| e.into_inner()).parse_errors += 1;
                    queue_error_response(connection, 400, "bad request");
                }
                break;
            }
        }
    }

    if connection.close_after_write && !connection.has_pending_write() {
        ConnectionAction::Close
    } else {
        ConnectionAction::Keep
    }
}

fn write_pending(connection: &mut Connection) -> std::io::Result<()> {
    while connection.write_pos < connection.write_buf.len() {
        match connection
            .stream
            .write(&connection.write_buf[connection.write_pos..])
        {
            Ok(0) => break,
            Ok(n) => {
                connection.write_pos += n;
                connection.last_activity = Instant::now();
            }
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => break,
            Err(error) => return Err(error),
        }
    }
    if connection.write_pos >= connection.write_buf.len() {
        connection.write_buf.clear();
        connection.write_pos = 0;
    }
    Ok(())
}

fn queue_response(
    connection: &mut Connection,
    response: ServerResponse,
    head_only: bool,
    request_close: bool,
) {
    let close = response.close || request_close;
    let wire = response_to_wire(response, head_only, close);
    connection.write_buf.extend_from_slice(&wire);
    connection.close_after_write = close;
}

fn queue_error_response(connection: &mut Connection, status_code: u16, message: &str) {
    let mut response = ServerResponse::text(status_code, message.to_string());
    response.close = true;
    queue_response(connection, response, false, true);
}

fn response_to_wire(mut response: ServerResponse, head_only: bool, close: bool) -> Vec<u8> {
    upsert_header(
        &mut response.headers,
        "Connection",
        if close { "close" } else { "keep-alive" },
    );
    if response.body.chunked {
        remove_header(&mut response.headers, "Content-Length");
        upsert_header(&mut response.headers, "Transfer-Encoding", "chunked");
    } else {
        remove_header(&mut response.headers, "Transfer-Encoding");
        upsert_header(
            &mut response.headers,
            "Content-Length",
            &response.body.bytes().len().to_string(),
        );
    }

    let body = if head_only {
        HttpBody::empty()
    } else {
        response.body
    };
    let parsed = ParsedResponse {
        version: HttpVersion::HTTP_11,
        status_code: response.status_code,
        reason: response.reason,
        headers: response.headers,
        body,
        keep_alive: !close,
    };
    serialize_response_for_server(&parsed)
}

fn serialize_response_for_server(response: &ParsedResponse) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(response.version.to_string().as_bytes());
    out.push(b' ');
    out.extend_from_slice(response.status_code.to_string().as_bytes());
    out.push(b' ');
    out.extend_from_slice(response.reason.as_bytes());
    out.extend_from_slice(b"\r\n");
    for header in &response.headers {
        out.extend_from_slice(header.name.as_bytes());
        out.extend_from_slice(b": ");
        out.extend_from_slice(header.value.as_bytes());
        out.extend_from_slice(b"\r\n");
    }
    out.extend_from_slice(b"\r\n");
    if response.body.chunked {
        for chunk in &response.body.chunks {
            out.extend_from_slice(format!("{:X}\r\n", chunk.data.len()).as_bytes());
            out.extend_from_slice(&chunk.data);
            out.extend_from_slice(b"\r\n");
        }
        out.extend_from_slice(b"0\r\n\r\n");
    } else {
        out.extend_from_slice(&response.body.bytes());
    }
    out
}

fn upsert_header(headers: &mut Vec<Header>, name: &str, value: &str) {
    if let Some(header) = headers
        .iter_mut()
        .find(|header| header.name.eq_ignore_ascii_case(name))
    {
        header.value = value.to_string();
    } else {
        headers.push(Header {
            name: name.to_string(),
            value: value.to_string(),
        });
    }
}

fn remove_header(headers: &mut Vec<Header>, name: &str) {
    headers.retain(|header| !header.name.eq_ignore_ascii_case(name));
}

fn record_timeout(stats: &Arc<Mutex<ServerStats>>) {
    stats.lock().unwrap_or_else(|e| e.into_inner()).timeouts += 1;
}

fn record_close(stats: &Arc<Mutex<ServerStats>>) {
    let mut stats = stats.lock().unwrap_or_else(|e| e.into_inner());
    stats.closed_connections += 1;
    stats.active_connections = stats.active_connections.saturating_sub(1);
}

fn reason_for_status(status_code: u16) -> &'static str {
    match status_code {
        200 => "OK",
        201 => "Created",
        204 => "No Content",
        400 => "Bad Request",
        404 => "Not Found",
        408 => "Request Timeout",
        413 => "Payload Too Large",
        500 => "Internal Server Error",
        _ => "Status",
    }
}

#[cfg(test)]
#[derive(Debug)]
struct ConnectionLimiter {
    max: usize,
    active: usize,
    peak: usize,
    rejected: usize,
}

#[cfg(test)]
impl ConnectionLimiter {
    fn new(max: usize) -> Self {
        Self {
            max,
            active: 0,
            peak: 0,
            rejected: 0,
        }
    }

    fn try_open(&mut self) -> bool {
        if self.active >= self.max {
            self.rejected += 1;
            return false;
        }
        self.active += 1;
        self.peak = self.peak.max(self.active);
        true
    }

    fn close(&mut self) {
        self.active = self.active.saturating_sub(1);
    }
}

struct ServerStore {
    next: SpectraHostValue,
    states: HashMap<SpectraHostValue, SpectraHostValue>,
}

impl ServerStore {
    fn new() -> Self {
        Self {
            next: 1,
            states: HashMap::new(),
        }
    }
}

fn store() -> &'static Mutex<ServerStore> {
    static STORE: OnceLock<Mutex<ServerStore>> = OnceLock::new();
    STORE.get_or_init(|| Mutex::new(ServerStore::new()))
}

pub extern "C" fn server_new(ctx: *mut SpectraHostCallContext) -> i32 {
    let mut store = store().lock().unwrap_or_else(|e| e.into_inner());
    let handle = store.next;
    store.next = store.next.saturating_add(1).max(1);
    store.states.insert(handle, SERVER_STATE_CREATED);
    write_result(ctx, handle)
}

pub extern "C" fn server_state(ctx: *mut SpectraHostCallContext) -> i32 {
    let Ok(args) = read_args(ctx, 1) else {
        return HOST_STATUS_INVALID_ARGUMENT;
    };
    let store = store().lock().unwrap_or_else(|e| e.into_inner());
    let Some(state) = store.states.get(&args[0]).copied() else {
        return HOST_STATUS_INVALID_ARGUMENT;
    };
    write_result(ctx, state)
}

pub extern "C" fn server_shutdown(ctx: *mut SpectraHostCallContext) -> i32 {
    let Ok(args) = read_args(ctx, 1) else {
        return HOST_STATUS_INVALID_ARGUMENT;
    };
    let mut store = store().lock().unwrap_or_else(|e| e.into_inner());
    let Some(state) = store.states.get_mut(&args[0]) else {
        return HOST_STATUS_INVALID_ARGUMENT;
    };
    *state = SERVER_STATE_STOPPED;
    write_result(ctx, 1)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::http::parse_response;
    use std::io::{Read, Write};

    fn start_test_server(config: ServerConfig) -> HttpServer {
        HttpServer::start(
            config,
            Arc::new(
                |request| match (request.method.as_str(), request.target.as_str()) {
                    ("GET", "/hello") => ServerResponse::text(200, "hello"),
                    ("POST", "/echo") => ServerResponse::bytes(200, request.body.bytes()),
                    ("GET", "/chunked") => {
                        ServerResponse::chunked(200, vec![b"alpha".to_vec(), b"beta".to_vec()])
                    }
                    ("HEAD", "/hello") => ServerResponse::text(200, "hello"),
                    _ => ServerResponse::text(404, "missing"),
                },
            ),
        )
        .expect("server starts")
    }

    fn request(addr: SocketAddr, raw: &[u8]) -> Vec<u8> {
        let mut stream = TcpStream::connect(addr).expect("connect test server");
        stream
            .set_read_timeout(Some(Duration::from_secs(2)))
            .expect("set read timeout");
        stream.write_all(raw).expect("write request");
        let mut response = Vec::new();
        stream.read_to_end(&mut response).expect("read response");
        response
    }

    #[test]
    fn end_to_end_get_post_chunked_and_head() {
        let mut server = start_test_server(ServerConfig {
            idle_timeout: Duration::from_millis(50),
            ..ServerConfig::default()
        });
        let addr = server.local_addr();

        let get = request(
            addr,
            b"GET /hello HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n",
        );
        let get = parse_response(&get).expect("GET response parses");
        assert_eq!(get.status_code, 200);
        assert_eq!(get.body.bytes(), b"hello");

        let post = request(
            addr,
            b"POST /echo HTTP/1.1\r\nHost: localhost\r\nContent-Length: 7\r\nConnection: close\r\n\r\npayload",
        );
        let post = parse_response(&post).expect("POST response parses");
        assert_eq!(post.status_code, 200);
        assert_eq!(post.body.bytes(), b"payload");

        let chunked = request(
            addr,
            b"GET /chunked HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n",
        );
        let chunked = parse_response(&chunked).expect("chunked response parses");
        assert!(chunked.body.chunked);
        assert_eq!(chunked.body.bytes(), b"alphabeta");

        let head = request(
            addr,
            b"HEAD /hello HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n",
        );
        let head_text = String::from_utf8(head).expect("HEAD response is utf-8");
        assert!(head_text.starts_with("HTTP/1.1 200 OK\r\n"));
        assert!(head_text.contains("Content-Length: 5\r\n"));
        assert!(head_text.ends_with("\r\n\r\n"));

        let stats = server.shutdown().expect("shutdown");
        assert_eq!(stats.completed_requests, 4);
        assert_eq!(stats.active_connections, 0);
    }

    #[test]
    fn body_limit_violation_returns_413_and_cleans_up() {
        let mut server = start_test_server(ServerConfig {
            max_body_bytes: 4,
            idle_timeout: Duration::from_millis(50),
            ..ServerConfig::default()
        });
        let raw = request(
            server.local_addr(),
            b"POST /echo HTTP/1.1\r\nHost: localhost\r\nContent-Length: 7\r\nConnection: close\r\n\r\npayload",
        );
        let response = parse_response(&raw).expect("413 response parses");
        assert_eq!(response.status_code, 413);

        let stats = server.shutdown().expect("shutdown");
        assert_eq!(stats.body_limit_violations, 1);
        assert_eq!(stats.active_connections, 0);
    }

    #[test]
    fn slowloris_timeout_closes_connection() {
        let mut server = start_test_server(ServerConfig {
            read_timeout: Duration::from_millis(40),
            idle_timeout: Duration::from_millis(500),
            poll_interval: Duration::from_millis(1),
            ..ServerConfig::default()
        });
        let mut stream = TcpStream::connect(server.local_addr()).expect("connect");
        stream
            .write_all(b"GET /hello HTTP/1.1\r\nHost")
            .expect("partial write");
        thread::sleep(Duration::from_millis(120));
        let mut response = Vec::new();
        let _ = stream.read_to_end(&mut response);
        assert!(
            response.is_empty() || String::from_utf8_lossy(&response).contains("408"),
            "unexpected timeout response: {:?}",
            String::from_utf8_lossy(&response)
        );

        let stats = server.shutdown().expect("shutdown");
        assert!(stats.timeouts >= 1);
        assert_eq!(stats.active_connections, 0);
    }

    #[test]
    fn parse_error_returns_400_and_cleans_up() {
        let mut server = start_test_server(ServerConfig {
            idle_timeout: Duration::from_millis(50),
            ..ServerConfig::default()
        });
        let raw = request(
            server.local_addr(),
            b"GET /bad HTTP/1.1\r\nBad Header: value\r\nConnection: close\r\n\r\n",
        );
        let response = parse_response(&raw).expect("400 response parses");
        assert_eq!(response.status_code, 400);

        let stats = server.shutdown().expect("shutdown");
        assert_eq!(stats.parse_errors, 1);
        assert_eq!(stats.active_connections, 0);
    }

    #[test]
    fn connection_limiter_survives_10k_concurrent_slots_without_threads() {
        let mut limiter = ConnectionLimiter::new(10_000);
        for _ in 0..10_000 {
            assert!(limiter.try_open());
        }
        assert!(!limiter.try_open());
        assert_eq!(limiter.peak, 10_000);
        assert_eq!(limiter.rejected, 1);
        for _ in 0..10_000 {
            limiter.close();
        }
        assert_eq!(limiter.active, 0);
    }
}
