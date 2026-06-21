use crate::http::{Header, Http1Parser, HttpBody, ParsedResponse, ParserConfig};
use crate::{read_args, write_result};
use spectra_runtime::ffi::{
    SpectraHostCallContext, SpectraHostValue, HOST_STATUS_INVALID_ARGUMENT,
};
use std::collections::HashMap;
use std::fmt;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

const DEFAULT_TIMEOUT_MS: SpectraHostValue = 30_000;
const DEFAULT_MAX_REDIRECTS: usize = 10;
const DEFAULT_POOL_IDLE_MS: u64 = 30_000;

#[derive(Clone, Debug)]
pub struct ClientConfig {
    pub timeout: Duration,
    pub max_redirects: usize,
    pub pool_idle_timeout: Duration,
    pub max_header_bytes: usize,
    pub max_body_bytes: usize,
    pub max_chunk_bytes: usize,
    pub user_agent: String,
}

impl Default for ClientConfig {
    fn default() -> Self {
        Self {
            timeout: Duration::from_millis(DEFAULT_TIMEOUT_MS as u64),
            max_redirects: DEFAULT_MAX_REDIRECTS,
            pool_idle_timeout: Duration::from_millis(DEFAULT_POOL_IDLE_MS),
            max_header_bytes: 64 * 1024,
            max_body_bytes: 16 * 1024 * 1024,
            max_chunk_bytes: 8 * 1024 * 1024,
            user_agent: "spectra-api/0.1".to_string(),
        }
    }
}

impl ClientConfig {
    fn parser_config(&self) -> ParserConfig {
        ParserConfig {
            max_header_bytes: self.max_header_bytes,
            max_body_bytes: self.max_body_bytes,
            max_chunk_bytes: self.max_chunk_bytes,
        }
    }
}

#[derive(Clone, Debug)]
pub struct ClientRequest {
    pub method: String,
    pub url: String,
    pub headers: Vec<Header>,
    pub body: Vec<u8>,
}

impl ClientRequest {
    pub fn new(method: impl Into<String>, url: impl Into<String>) -> Self {
        Self {
            method: method.into(),
            url: url.into(),
            headers: Vec::new(),
            body: Vec::new(),
        }
    }

    pub fn with_body(mut self, body: Vec<u8>) -> Self {
        self.body = body;
        self
    }

    pub fn with_header(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.headers.push(Header {
            name: name.into(),
            value: value.into(),
        });
        self
    }
}

#[derive(Clone, Debug)]
pub struct ClientResponse {
    pub status_code: u16,
    pub reason: String,
    pub headers: Vec<Header>,
    pub body: HttpBody,
    pub final_url: String,
    pub redirect_count: usize,
    pub keep_alive: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ClientErrorKind {
    InvalidUrl,
    UnsupportedScheme,
    ConnectionFailed,
    Timeout,
    Protocol,
    RedirectLimit,
    MissingRedirectLocation,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ClientError {
    pub kind: ClientErrorKind,
    pub message: String,
}

impl ClientError {
    fn new(kind: ClientErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
        }
    }
}

impl fmt::Display for ClientError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for ClientError {}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
struct Authority {
    host: String,
    port: u16,
}

#[derive(Clone, Debug)]
struct ParsedUrl {
    authority: Authority,
    path_and_query: String,
}

struct PooledConnection {
    stream: TcpStream,
    idle_since: Instant,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct ClientStats {
    pub opened_connections: usize,
    pub reused_connections: usize,
    pub pooled_connections: usize,
    pub redirects_followed: usize,
}

pub struct HttpClient {
    config: ClientConfig,
    pool: Mutex<HashMap<Authority, Vec<PooledConnection>>>,
    stats: Mutex<ClientStats>,
}

impl HttpClient {
    pub fn new(config: ClientConfig) -> Self {
        Self {
            config,
            pool: Mutex::new(HashMap::new()),
            stats: Mutex::new(ClientStats::default()),
        }
    }

    pub fn get(&self, url: &str) -> Result<ClientResponse, ClientError> {
        self.request(ClientRequest::new("GET", url))
    }

    pub fn head(&self, url: &str) -> Result<ClientResponse, ClientError> {
        self.request(ClientRequest::new("HEAD", url))
    }

    pub fn delete(&self, url: &str) -> Result<ClientResponse, ClientError> {
        self.request(ClientRequest::new("DELETE", url))
    }

    pub fn post(&self, url: &str, body: Vec<u8>) -> Result<ClientResponse, ClientError> {
        self.request(ClientRequest::new("POST", url).with_body(body))
    }

    pub fn put(&self, url: &str, body: Vec<u8>) -> Result<ClientResponse, ClientError> {
        self.request(ClientRequest::new("PUT", url).with_body(body))
    }

    pub fn patch(&self, url: &str, body: Vec<u8>) -> Result<ClientResponse, ClientError> {
        self.request(ClientRequest::new("PATCH", url).with_body(body))
    }

    pub fn stats(&self) -> ClientStats {
        let mut stats = self.stats.lock().unwrap_or_else(|e| e.into_inner()).clone();
        stats.pooled_connections = self
            .pool
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .values()
            .map(Vec::len)
            .sum();
        stats
    }

    pub fn request(&self, request: ClientRequest) -> Result<ClientResponse, ClientError> {
        let mut current = request;
        for redirect_count in 0..=self.config.max_redirects {
            let parsed_url = parse_http_url(&current.url)?;
            let (response, reusable_stream) = self.send_once(&current, &parsed_url)?;
            if let Some(next_url) = redirect_target(&response, &current.url)? {
                if redirect_count == self.config.max_redirects {
                    return Err(ClientError::new(
                        ClientErrorKind::RedirectLimit,
                        "redirect limit exceeded",
                    ));
                }
                if let Some(stream) = reusable_stream {
                    self.put_connection(parsed_url.authority, stream);
                }
                current = redirected_request(current, next_url, response.status_code)?;
                self.stats
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .redirects_followed += 1;
                continue;
            }

            if let Some(stream) = reusable_stream {
                self.put_connection(parsed_url.authority, stream);
            }
            return Ok(ClientResponse {
                status_code: response.status_code,
                reason: response.reason,
                headers: response.headers,
                body: response.body,
                final_url: current.url,
                redirect_count,
                keep_alive: response.keep_alive,
            });
        }

        Err(ClientError::new(
            ClientErrorKind::RedirectLimit,
            "redirect loop exceeded configured limit",
        ))
    }

    fn send_once(
        &self,
        request: &ClientRequest,
        parsed_url: &ParsedUrl,
    ) -> Result<(ParsedResponse, Option<TcpStream>), ClientError> {
        let mut stream = self.take_or_connect(&parsed_url.authority)?;
        stream
            .set_read_timeout(Some(self.config.timeout))
            .map_err(|e| io_error(ClientErrorKind::ConnectionFailed, e))?;
        stream
            .set_write_timeout(Some(self.config.timeout))
            .map_err(|e| io_error(ClientErrorKind::ConnectionFailed, e))?;

        let wire = build_request_wire(request, parsed_url, &self.config);
        stream.write_all(&wire).map_err(map_io_error)?;
        stream.flush().map_err(map_io_error)?;

        if request.method.eq_ignore_ascii_case("HEAD") {
            let response = read_head_response(&mut stream, self.config.timeout)?;
            let reusable =
                response.keep_alive && !header_has_token(&response.headers, "connection", "close");
            if reusable {
                return Ok((response, Some(stream)));
            }
            return Ok((response, None));
        }

        let mut parser = Http1Parser::response_with_config(self.config.parser_config());
        let mut buf = [0_u8; 8192];
        loop {
            match stream.read(&mut buf) {
                Ok(0) => {
                    return Err(ClientError::new(
                        ClientErrorKind::Protocol,
                        "connection closed before a complete HTTP response was received",
                    ));
                }
                Ok(n) => {
                    parser.push(&buf[..n]);
                    match parser.parse_next_response() {
                        Ok(Some(response)) => {
                            let reusable = response.keep_alive
                                && !header_has_token(&response.headers, "connection", "close")
                                && parser.buffered_len() == 0;
                            if reusable {
                                return Ok((response, Some(stream)));
                            }
                            return Ok((response, None));
                        }
                        Ok(None) => {}
                        Err(error) => {
                            return Err(ClientError::new(
                                ClientErrorKind::Protocol,
                                format!("invalid HTTP response: {error}"),
                            ));
                        }
                    }
                }
                Err(error)
                    if matches!(
                        error.kind(),
                        std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                    ) =>
                {
                    return Err(ClientError::new(
                        ClientErrorKind::Timeout,
                        "HTTP client timed out waiting for response",
                    ));
                }
                Err(error) => return Err(io_error(ClientErrorKind::ConnectionFailed, error)),
            }
        }
    }

    fn take_or_connect(&self, authority: &Authority) -> Result<TcpStream, ClientError> {
        self.drop_expired_pool_entries();
        if let Some(stream) = self.take_connection(authority) {
            self.stats
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .reused_connections += 1;
            return Ok(stream);
        }
        let stream = TcpStream::connect((authority.host.as_str(), authority.port))
            .map_err(|e| io_error(ClientErrorKind::ConnectionFailed, e))?;
        self.stats
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .opened_connections += 1;
        Ok(stream)
    }

    fn take_connection(&self, authority: &Authority) -> Option<TcpStream> {
        self.pool
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get_mut(authority)
            .and_then(Vec::pop)
            .map(|conn| conn.stream)
    }

    fn put_connection(&self, authority: Authority, stream: TcpStream) {
        let _ = stream.set_read_timeout(None);
        let _ = stream.set_write_timeout(None);
        self.pool
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .entry(authority)
            .or_default()
            .push(PooledConnection {
                stream,
                idle_since: Instant::now(),
            });
    }

    fn drop_expired_pool_entries(&self) {
        let timeout = self.config.pool_idle_timeout;
        let now = Instant::now();
        self.pool
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .values_mut()
            .for_each(|connections| {
                connections.retain(|conn| now.duration_since(conn.idle_since) <= timeout);
            });
    }
}

fn read_head_response(
    stream: &mut TcpStream,
    timeout: Duration,
) -> Result<ParsedResponse, ClientError> {
    stream
        .set_read_timeout(Some(timeout))
        .map_err(|e| io_error(ClientErrorKind::ConnectionFailed, e))?;
    let mut raw = Vec::new();
    let mut buf = [0_u8; 1024];
    loop {
        match stream.read(&mut buf) {
            Ok(0) => {
                return Err(ClientError::new(
                    ClientErrorKind::Protocol,
                    "connection closed before complete HEAD response headers",
                ));
            }
            Ok(n) => {
                raw.extend_from_slice(&buf[..n]);
                if let Some(header_end) = raw.windows(4).position(|window| window == b"\r\n\r\n") {
                    return parse_head_response_headers(&raw[..header_end]);
                }
            }
            Err(error)
                if matches!(
                    error.kind(),
                    std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                ) =>
            {
                return Err(ClientError::new(
                    ClientErrorKind::Timeout,
                    "HTTP client timed out waiting for HEAD response",
                ));
            }
            Err(error) => return Err(io_error(ClientErrorKind::ConnectionFailed, error)),
        }
    }
}

fn parse_head_response_headers(raw: &[u8]) -> Result<ParsedResponse, ClientError> {
    let text = std::str::from_utf8(raw).map_err(|_| {
        ClientError::new(
            ClientErrorKind::Protocol,
            "HEAD response headers are not valid UTF-8",
        )
    })?;
    let mut lines = text.split("\r\n");
    let status_line = lines.next().unwrap_or_default();
    let mut parts = status_line.splitn(3, ' ');
    let version_text = parts.next().unwrap_or_default();
    let status_text = parts.next().unwrap_or_default();
    let reason = parts.next().unwrap_or_default().to_string();
    let version = parse_response_version(version_text)?;
    let status_code = status_text.parse::<u16>().map_err(|_| {
        ClientError::new(
            ClientErrorKind::Protocol,
            "HEAD response status code is invalid",
        )
    })?;
    let mut headers = Vec::new();
    for line in lines {
        if line.is_empty() {
            continue;
        }
        let Some((name, value)) = line.split_once(':') else {
            return Err(ClientError::new(
                ClientErrorKind::Protocol,
                "HEAD response header is missing ':'",
            ));
        };
        headers.push(Header {
            name: name.to_string(),
            value: value.trim().to_string(),
        });
    }
    let keep_alive = !header_has_token(&headers, "connection", "close")
        && (version.major > 1
            || (version.major == 1 && version.minor >= 1)
            || header_has_token(&headers, "connection", "keep-alive"));
    Ok(ParsedResponse {
        version,
        status_code,
        reason,
        headers,
        body: HttpBody::empty(),
        keep_alive,
    })
}

fn parse_response_version(text: &str) -> Result<crate::http::HttpVersion, ClientError> {
    let Some(rest) = text.strip_prefix("HTTP/") else {
        return Err(ClientError::new(
            ClientErrorKind::Protocol,
            "response version is missing HTTP/ prefix",
        ));
    };
    let Some((major, minor)) = rest.split_once('.') else {
        return Err(ClientError::new(
            ClientErrorKind::Protocol,
            "response version is malformed",
        ));
    };
    Ok(crate::http::HttpVersion {
        major: major.parse::<u8>().map_err(|_| {
            ClientError::new(
                ClientErrorKind::Protocol,
                "response major version is invalid",
            )
        })?,
        minor: minor.parse::<u8>().map_err(|_| {
            ClientError::new(
                ClientErrorKind::Protocol,
                "response minor version is invalid",
            )
        })?,
    })
}

fn parse_http_url(url: &str) -> Result<ParsedUrl, ClientError> {
    let Some(rest) = url.strip_prefix("http://") else {
        if url.contains("://") {
            return Err(ClientError::new(
                ClientErrorKind::UnsupportedScheme,
                "only http:// URLs are supported before TLS lands",
            ));
        }
        return Err(ClientError::new(
            ClientErrorKind::InvalidUrl,
            "URL must start with http://",
        ));
    };
    let (authority_text, path) = match rest.split_once('/') {
        Some((authority, path)) => (authority, format!("/{path}")),
        None => (rest, "/".to_string()),
    };
    if authority_text.is_empty() || authority_text.contains('@') {
        return Err(ClientError::new(
            ClientErrorKind::InvalidUrl,
            "URL authority is invalid",
        ));
    }
    let (host, port) = parse_authority(authority_text)?;
    if host.is_empty() {
        return Err(ClientError::new(
            ClientErrorKind::InvalidUrl,
            "URL host is empty",
        ));
    }
    Ok(ParsedUrl {
        authority: Authority { host, port },
        path_and_query: if path.is_empty() {
            "/".to_string()
        } else {
            path
        },
    })
}

fn parse_authority(authority: &str) -> Result<(String, u16), ClientError> {
    if authority.starts_with('[') {
        return Err(ClientError::new(
            ClientErrorKind::InvalidUrl,
            "IPv6 literal URLs are not supported yet",
        ));
    }
    match authority.rsplit_once(':') {
        Some((host, port)) if !port.is_empty() && port.bytes().all(|b| b.is_ascii_digit()) => {
            let port = port.parse::<u16>().map_err(|_| {
                ClientError::new(ClientErrorKind::InvalidUrl, "URL port is out of range")
            })?;
            Ok((host.to_string(), port))
        }
        Some((_, port)) if port.is_empty() || !port.bytes().all(|b| b.is_ascii_digit()) => Err(
            ClientError::new(ClientErrorKind::InvalidUrl, "URL port is invalid"),
        ),
        _ => Ok((authority.to_string(), 80)),
    }
}

fn build_request_wire(
    request: &ClientRequest,
    parsed_url: &ParsedUrl,
    config: &ClientConfig,
) -> Vec<u8> {
    let mut headers = request.headers.clone();
    upsert_header(
        &mut headers,
        "Host",
        &host_header_value(&parsed_url.authority),
    );
    upsert_header(&mut headers, "User-Agent", &config.user_agent);
    upsert_header(&mut headers, "Connection", "keep-alive");
    if request.body.is_empty() {
        remove_header(&mut headers, "Content-Length");
    } else {
        upsert_header(
            &mut headers,
            "Content-Length",
            &request.body.len().to_string(),
        );
    }

    let mut out = Vec::new();
    out.extend_from_slice(request.method.as_bytes());
    out.push(b' ');
    out.extend_from_slice(parsed_url.path_and_query.as_bytes());
    out.extend_from_slice(b" HTTP/1.1\r\n");
    for header in &headers {
        out.extend_from_slice(header.name.as_bytes());
        out.extend_from_slice(b": ");
        out.extend_from_slice(header.value.as_bytes());
        out.extend_from_slice(b"\r\n");
    }
    out.extend_from_slice(b"\r\n");
    out.extend_from_slice(&request.body);
    out
}

fn host_header_value(authority: &Authority) -> String {
    if authority.port == 80 {
        authority.host.clone()
    } else {
        format!("{}:{}", authority.host, authority.port)
    }
}

fn redirected_request(
    mut request: ClientRequest,
    location: String,
    status: u16,
) -> Result<ClientRequest, ClientError> {
    let original = parse_http_url(&request.url)?;
    request.url = resolve_location(&request.url, &location)?;
    match status {
        301 | 302 => {
            if request.method.eq_ignore_ascii_case("POST") {
                request.method = "GET".to_string();
                request.body.clear();
                remove_header(&mut request.headers, "Content-Length");
            }
        }
        303 => {
            if !request.method.eq_ignore_ascii_case("HEAD") {
                request.method = "GET".to_string();
                request.body.clear();
                remove_header(&mut request.headers, "Content-Length");
            }
        }
        307 | 308 => {}
        _ => {}
    }
    let next = parse_http_url(&request.url)?;
    if next.authority != original.authority {
        remove_header(&mut request.headers, "Host");
    }
    Ok(request)
}

fn redirect_target(
    response: &ParsedResponse,
    current_url: &str,
) -> Result<Option<String>, ClientError> {
    if !matches!(response.status_code, 301 | 302 | 303 | 307 | 308) {
        return Ok(None);
    }
    let Some(location) = header_value(&response.headers, "location") else {
        return Err(ClientError::new(
            ClientErrorKind::MissingRedirectLocation,
            "redirect response is missing Location header",
        ));
    };
    resolve_location(current_url, location).map(Some)
}

fn resolve_location(current_url: &str, location: &str) -> Result<String, ClientError> {
    if location.starts_with("http://") {
        return Ok(location.to_string());
    }
    if location.contains("://") {
        return Err(ClientError::new(
            ClientErrorKind::UnsupportedScheme,
            "redirect target uses an unsupported scheme",
        ));
    }
    let parsed = parse_http_url(current_url)?;
    if location.starts_with('/') {
        return Ok(format!(
            "http://{}{}",
            host_header_value(&parsed.authority),
            location
        ));
    }
    let base_path = parsed
        .path_and_query
        .rsplit_once('/')
        .map(|(base, _)| {
            if base.is_empty() {
                "/".to_string()
            } else {
                format!("{base}/")
            }
        })
        .unwrap_or_else(|| "/".to_string());
    Ok(format!(
        "http://{}{}{}",
        host_header_value(&parsed.authority),
        base_path,
        location
    ))
}

fn header_value<'a>(headers: &'a [Header], name: &str) -> Option<&'a str> {
    headers
        .iter()
        .find(|header| header.name.eq_ignore_ascii_case(name))
        .map(|header| header.value.as_str())
}

fn header_has_token(headers: &[Header], name: &str, token: &str) -> bool {
    header_value(headers, name)
        .map(|value| {
            value
                .split(',')
                .any(|part| part.trim().eq_ignore_ascii_case(token))
        })
        .unwrap_or(false)
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

fn map_io_error(error: std::io::Error) -> ClientError {
    if matches!(
        error.kind(),
        std::io::ErrorKind::TimedOut | std::io::ErrorKind::WouldBlock
    ) {
        ClientError::new(ClientErrorKind::Timeout, "HTTP client I/O timed out")
    } else {
        io_error(ClientErrorKind::ConnectionFailed, error)
    }
}

fn io_error(kind: ClientErrorKind, error: std::io::Error) -> ClientError {
    ClientError::new(kind, error.to_string())
}

struct ClientStore {
    next: SpectraHostValue,
    timeouts: HashMap<SpectraHostValue, SpectraHostValue>,
}

impl ClientStore {
    fn new() -> Self {
        Self {
            next: 1,
            timeouts: HashMap::new(),
        }
    }
}

fn store() -> &'static Mutex<ClientStore> {
    static STORE: OnceLock<Mutex<ClientStore>> = OnceLock::new();
    STORE.get_or_init(|| Mutex::new(ClientStore::new()))
}

pub extern "C" fn client_new(ctx: *mut SpectraHostCallContext) -> i32 {
    let mut store = store().lock().unwrap_or_else(|e| e.into_inner());
    let handle = store.next;
    store.next = store.next.saturating_add(1).max(1);
    store.timeouts.insert(handle, DEFAULT_TIMEOUT_MS);
    write_result(ctx, handle)
}

pub extern "C" fn client_timeout_ms(ctx: *mut SpectraHostCallContext) -> i32 {
    let Ok(args) = read_args(ctx, 1) else {
        return HOST_STATUS_INVALID_ARGUMENT;
    };
    let store = store().lock().unwrap_or_else(|e| e.into_inner());
    let Some(timeout) = store.timeouts.get(&args[0]).copied() else {
        return HOST_STATUS_INVALID_ARGUMENT;
    };
    write_result(ctx, timeout)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::server::{HttpServer, ServerConfig, ServerResponse};
    use std::io::Write;
    use std::net::TcpListener;
    use std::sync::Arc;
    use std::thread;

    fn start_client_test_server() -> HttpServer {
        HttpServer::start(
            ServerConfig {
                idle_timeout: Duration::from_millis(250),
                ..ServerConfig::default()
            },
            Arc::new(|request| match request.target.as_str() {
                "/echo" => ServerResponse::text(
                    200,
                    format!(
                        "{}:{}",
                        request.method,
                        String::from_utf8_lossy(&request.body.bytes())
                    ),
                ),
                "/large" => ServerResponse::bytes(200, request.body.bytes()),
                "/redirect-post" => redirect_response(302, "/landed"),
                "/redirect-preserve" => redirect_response(307, "/echo"),
                "/loop" => redirect_response(302, "/loop"),
                "/landed" => ServerResponse::text(
                    200,
                    format!("{}:{}", request.method, request.body.bytes().len()),
                ),
                "/slow" => {
                    thread::sleep(Duration::from_millis(120));
                    ServerResponse::text(200, "slow")
                }
                _ => ServerResponse::text(404, "missing"),
            }),
        )
        .expect("client test server starts")
    }

    fn redirect_response(status: u16, location: &str) -> ServerResponse {
        ServerResponse {
            status_code: status,
            reason: "Redirect".to_string(),
            headers: vec![Header {
                name: "Location".to_string(),
                value: location.to_string(),
            }],
            body: HttpBody::empty(),
            close: false,
        }
    }

    fn url(server: &HttpServer, path: &str) -> String {
        format!("http://{}{}", server.local_addr(), path)
    }

    #[test]
    fn client_supports_methods_and_arbitrary_bodies() {
        let mut server = start_client_test_server();
        let client = HttpClient::new(ClientConfig::default());

        let get = client.get(&url(&server, "/echo")).expect("GET");
        assert_eq!(get.body.bytes(), b"GET:");

        let post = client
            .post(&url(&server, "/echo"), b"post-body".to_vec())
            .expect("POST");
        assert_eq!(post.body.bytes(), b"POST:post-body");

        let put = client
            .put(&url(&server, "/echo"), b"put-body".to_vec())
            .expect("PUT");
        assert_eq!(put.body.bytes(), b"PUT:put-body");

        let patch = client
            .patch(&url(&server, "/echo"), b"patch-body".to_vec())
            .expect("PATCH");
        assert_eq!(patch.body.bytes(), b"PATCH:patch-body");

        let delete = client
            .request(
                ClientRequest::new("DELETE", url(&server, "/echo")).with_body(b"gone".to_vec()),
            )
            .expect("DELETE");
        assert_eq!(delete.body.bytes(), b"DELETE:gone");

        let head = client.head(&url(&server, "/echo")).expect("HEAD");
        assert_eq!(head.status_code, 200);
        assert!(head.body.bytes().is_empty());

        let stats = client.stats();
        assert!(stats.opened_connections >= 1);
        assert!(client.stats().pooled_connections >= 1);
        let _ = server.shutdown();
    }

    #[test]
    fn client_reuses_pooled_connection() {
        let mut server = start_client_test_server();
        let client = HttpClient::new(ClientConfig::default());
        client.get(&url(&server, "/echo")).expect("first GET");
        client.get(&url(&server, "/echo")).expect("second GET");
        let stats = client.stats();
        assert_eq!(stats.opened_connections, 1);
        assert!(stats.reused_connections >= 1);
        let _ = server.shutdown();
    }

    #[test]
    fn client_follows_redirects_with_method_semantics() {
        let mut server = start_client_test_server();
        let client = HttpClient::new(ClientConfig::default());

        let converted = client
            .post(&url(&server, "/redirect-post"), b"discarded".to_vec())
            .expect("302 POST redirect");
        assert_eq!(converted.status_code, 200);
        assert_eq!(converted.body.bytes(), b"GET:0");
        assert_eq!(converted.redirect_count, 1);

        let preserved = client
            .post(&url(&server, "/redirect-preserve"), b"kept".to_vec())
            .expect("307 POST redirect");
        assert_eq!(preserved.body.bytes(), b"POST:kept");
        assert_eq!(preserved.redirect_count, 1);

        let stats = client.stats();
        assert_eq!(stats.redirects_followed, 2);
        let _ = server.shutdown();
    }

    #[test]
    fn client_enforces_redirect_limit() {
        let mut server = start_client_test_server();
        let client = HttpClient::new(ClientConfig {
            max_redirects: 2,
            ..ClientConfig::default()
        });
        let err = client
            .get(&url(&server, "/loop"))
            .expect_err("redirect limit");
        assert_eq!(err.kind, ClientErrorKind::RedirectLimit);
        let _ = server.shutdown();
    }

    #[test]
    fn client_handles_large_bodies() {
        let mut server = start_client_test_server();
        let client = HttpClient::new(ClientConfig {
            max_body_bytes: 512 * 1024,
            ..ClientConfig::default()
        });
        let body = vec![b'x'; 256 * 1024];
        let response = client
            .post(&url(&server, "/large"), body.clone())
            .expect("large body");
        assert_eq!(response.body.bytes(), body);
        let _ = server.shutdown();
    }

    #[test]
    fn client_reports_explicit_timeout() {
        let mut server = start_client_test_server();
        let client = HttpClient::new(ClientConfig {
            timeout: Duration::from_millis(20),
            ..ClientConfig::default()
        });
        let err = client.get(&url(&server, "/slow")).expect_err("timeout");
        assert_eq!(err.kind, ClientErrorKind::Timeout);
        let _ = server.shutdown();
    }

    #[test]
    fn client_reports_connection_failure() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind unused port");
        let addr = listener.local_addr().expect("local addr");
        drop(listener);
        let client = HttpClient::new(ClientConfig {
            timeout: Duration::from_millis(50),
            ..ClientConfig::default()
        });
        let err = client
            .get(&format!("http://{addr}/missing"))
            .expect_err("connection failure");
        assert_eq!(err.kind, ClientErrorKind::ConnectionFailed);
    }

    #[test]
    fn client_reports_protocol_error() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind protocol server");
        let addr = listener.local_addr().expect("local addr");
        let join = thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept protocol test");
            stream
                .set_read_timeout(Some(Duration::from_secs(1)))
                .expect("set read timeout");
            let mut request = [0_u8; 512];
            let _ = stream.read(&mut request);
            stream
                .write_all(b"not-http\r\nContent-Length: 0\r\n\r\n")
                .expect("write invalid");
            stream.flush().expect("flush invalid response");
        });
        let client = HttpClient::new(ClientConfig::default());
        let err = client
            .get(&format!("http://{addr}/bad"))
            .expect_err("protocol error");
        assert_eq!(err.kind, ClientErrorKind::Protocol);
        join.join().expect("protocol server joined");
    }
}
