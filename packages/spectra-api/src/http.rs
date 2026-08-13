use crate::{alloc_spectra_string, read_args, read_spectra_string, write_result};
use crate::handles::ApiHandleTable;
use spectra_runtime::ffi::{
    SpectraHostCallContext, SpectraHostValue, HOST_STATUS_INVALID_ARGUMENT,
};
use spectra_runtime::handles::HandleKind;
use std::fmt;
use std::sync::{Mutex, OnceLock};

pub const METHOD_GET: SpectraHostValue = 1;
pub const METHOD_HEAD: SpectraHostValue = 2;
pub const METHOD_POST: SpectraHostValue = 3;
pub const METHOD_PUT: SpectraHostValue = 4;
pub const METHOD_PATCH: SpectraHostValue = 5;
pub const METHOD_DELETE: SpectraHostValue = 6;
pub const METHOD_OPTIONS: SpectraHostValue = 7;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Method {
    Get,
    Head,
    Post,
    Put,
    Patch,
    Delete,
    Options,
}

impl Method {
    pub fn from_code(code: SpectraHostValue) -> Option<Self> {
        match code {
            METHOD_GET => Some(Self::Get),
            METHOD_HEAD => Some(Self::Head),
            METHOD_POST => Some(Self::Post),
            METHOD_PUT => Some(Self::Put),
            METHOD_PATCH => Some(Self::Patch),
            METHOD_DELETE => Some(Self::Delete),
            METHOD_OPTIONS => Some(Self::Options),
            _ => None,
        }
    }

    pub fn code(self) -> SpectraHostValue {
        match self {
            Self::Get => METHOD_GET,
            Self::Head => METHOD_HEAD,
            Self::Post => METHOD_POST,
            Self::Put => METHOD_PUT,
            Self::Patch => METHOD_PATCH,
            Self::Delete => METHOD_DELETE,
            Self::Options => METHOD_OPTIONS,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Get => "GET",
            Self::Head => "HEAD",
            Self::Post => "POST",
            Self::Put => "PUT",
            Self::Patch => "PATCH",
            Self::Delete => "DELETE",
            Self::Options => "OPTIONS",
        }
    }

    pub fn allows_body(self) -> bool {
        matches!(self, Self::Post | Self::Put | Self::Patch)
    }

    pub fn is_safe(self) -> bool {
        matches!(self, Self::Get | Self::Head | Self::Options)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Status {
    code: u16,
}

impl Status {
    pub fn new(code: u16) -> Result<Self, HttpTypeError> {
        if (100..=599).contains(&code) {
            Ok(Self { code })
        } else {
            Err(HttpTypeError::InvalidStatus(code))
        }
    }

    pub fn code(self) -> u16 {
        self.code
    }

    pub fn reason(self) -> &'static str {
        status_reason_phrase(self.code as SpectraHostValue)
    }

    pub fn class(self) -> u16 {
        self.code / 100
    }

    pub fn is_success(self) -> bool {
        (200..=299).contains(&self.code)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum HttpTypeError {
    InvalidMethod(SpectraHostValue),
    InvalidStatus(u16),
    InvalidHeaderName(String),
    InvalidHeaderValue(String),
    InvalidCookieName(String),
    InvalidCookieValue(String),
    InvalidPath(String),
}

impl fmt::Display for HttpTypeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidMethod(code) => write!(f, "invalid HTTP method code {code}"),
            Self::InvalidStatus(code) => write!(f, "invalid HTTP status code {code}"),
            Self::InvalidHeaderName(name) => write!(f, "invalid HTTP header name {name:?}"),
            Self::InvalidHeaderValue(value) => write!(f, "invalid HTTP header value {value:?}"),
            Self::InvalidCookieName(name) => write!(f, "invalid HTTP cookie name {name:?}"),
            Self::InvalidCookieValue(value) => write!(f, "invalid HTTP cookie value {value:?}"),
            Self::InvalidPath(path) => write!(f, "invalid HTTP request path {path:?}"),
        }
    }
}

impl std::error::Error for HttpTypeError {}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Header {
    pub name: String,
    pub value: String,
}

impl Header {
    pub fn new(name: impl Into<String>, value: impl Into<String>) -> Result<Self, HttpTypeError> {
        let name = name.into();
        let value = value.into();
        if !is_valid_header_name(&name) {
            return Err(HttpTypeError::InvalidHeaderName(name));
        }
        if !is_valid_header_value(&value) {
            return Err(HttpTypeError::InvalidHeaderValue(value));
        }
        Ok(Self { name, value })
    }

    pub fn name_eq(&self, name: &str) -> bool {
        self.name.eq_ignore_ascii_case(name)
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Headers {
    entries: Vec<Header>,
}

impl Headers {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn from_vec(entries: Vec<Header>) -> Self {
        Self { entries }
    }

    pub fn insert(
        &mut self,
        name: impl Into<String>,
        value: impl Into<String>,
    ) -> Result<(), HttpTypeError> {
        let header = Header::new(name, value)?;
        if let Some(existing) = self
            .entries
            .iter_mut()
            .find(|existing| existing.name_eq(&header.name))
        {
            existing.value = header.value;
        } else {
            self.entries.push(header);
        }
        Ok(())
    }

    pub fn get(&self, name: &str) -> Option<&str> {
        self.entries
            .iter()
            .find(|header| header.name_eq(name))
            .map(|header| header.value.as_str())
    }

    pub fn contains(&self, name: &str) -> bool {
        self.get(name).is_some()
    }

    pub fn remove(&mut self, name: &str) -> bool {
        let old_len = self.entries.len();
        self.entries.retain(|header| !header.name_eq(name));
        self.entries.len() != old_len
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = &Header> {
        self.entries.iter()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Cookie {
    pub name: String,
    pub value: String,
}

impl Cookie {
    pub fn new(name: impl Into<String>, value: impl Into<String>) -> Result<Self, HttpTypeError> {
        let name = name.into();
        let value = value.into();
        if !is_valid_cookie_name(&name) {
            return Err(HttpTypeError::InvalidCookieName(name));
        }
        if !is_valid_cookie_value(&value) {
            return Err(HttpTypeError::InvalidCookieValue(value));
        }
        Ok(Self { name, value })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Request {
    pub method: Method,
    pub path: String,
    pub headers: Headers,
    pub body: Vec<u8>,
}

impl Request {
    pub fn new(method: Method, path: impl Into<String>) -> Result<Self, HttpTypeError> {
        let path = path.into();
        if !is_valid_request_path(&path) {
            return Err(HttpTypeError::InvalidPath(path));
        }
        Ok(Self {
            method,
            path,
            headers: Headers::new(),
            body: Vec::new(),
        })
    }

    pub fn with_header(
        mut self,
        name: impl Into<String>,
        value: impl Into<String>,
    ) -> Result<Self, HttpTypeError> {
        self.headers.insert(name, value)?;
        Ok(self)
    }

    pub fn with_body(mut self, body: Vec<u8>) -> Self {
        self.body = body;
        self
    }

    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers.get(name)
    }

    pub fn cookie(&self, name: &str) -> Option<String> {
        cookie_value_from_header(self.header("cookie")?, name)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Response {
    pub status: Status,
    pub headers: Headers,
    pub body: Vec<u8>,
}

impl Response {
    pub fn new(status: Status) -> Self {
        Self {
            status,
            headers: Headers::new(),
            body: Vec::new(),
        }
    }

    pub fn with_header(
        mut self,
        name: impl Into<String>,
        value: impl Into<String>,
    ) -> Result<Self, HttpTypeError> {
        self.headers.insert(name, value)?;
        Ok(self)
    }

    pub fn with_body(mut self, body: Vec<u8>) -> Self {
        self.body = body;
        self
    }

    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers.get(name)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HttpVersion {
    pub major: u8,
    pub minor: u8,
}

impl HttpVersion {
    pub const HTTP_10: Self = Self { major: 1, minor: 0 };
    pub const HTTP_11: Self = Self { major: 1, minor: 1 };
}

impl fmt::Display for HttpVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "HTTP/{}.{}", self.major, self.minor)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BodyChunk {
    pub data: Vec<u8>,
    pub extension: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct HttpBody {
    pub chunks: Vec<BodyChunk>,
    pub trailers: Vec<Header>,
    pub chunked: bool,
}

impl HttpBody {
    pub fn empty() -> Self {
        Self::default()
    }

    pub fn from_bytes(bytes: Vec<u8>) -> Self {
        if bytes.is_empty() {
            Self::empty()
        } else {
            Self {
                chunks: vec![BodyChunk {
                    data: bytes,
                    extension: None,
                }],
                trailers: Vec::new(),
                chunked: false,
            }
        }
    }

    pub fn bytes(&self) -> Vec<u8> {
        let len = self.chunks.iter().map(|chunk| chunk.data.len()).sum();
        let mut out = Vec::with_capacity(len);
        for chunk in &self.chunks {
            out.extend_from_slice(&chunk.data);
        }
        out
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParsedRequest {
    pub method: String,
    pub target: String,
    pub version: HttpVersion,
    pub headers: Vec<Header>,
    pub body: HttpBody,
    pub keep_alive: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParsedResponse {
    pub version: HttpVersion,
    pub status_code: u16,
    pub reason: String,
    pub headers: Vec<Header>,
    pub body: HttpBody,
    pub keep_alive: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ParseErrorKind {
    Incomplete,
    InvalidStartLine,
    InvalidMethod,
    InvalidTarget,
    InvalidVersion,
    InvalidStatus,
    InvalidHeader,
    HeaderTooLarge,
    BodyTooLarge,
    BodyLengthMismatch,
    InvalidChunkSize,
    InvalidChunkTerminator,
    UnsupportedTransferEncoding,
    ObsoleteLineFolding,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParseError {
    pub kind: ParseErrorKind,
    pub position: usize,
    pub message: String,
}

impl ParseError {
    fn new(kind: ParseErrorKind, position: usize, message: impl Into<String>) -> Self {
        Self {
            kind,
            position,
            message: message.into(),
        }
    }
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} at byte {}", self.message, self.position)
    }
}

impl std::error::Error for ParseError {}

#[derive(Clone, Debug)]
pub struct ParserConfig {
    pub max_header_bytes: usize,
    pub max_body_bytes: usize,
    pub max_chunk_bytes: usize,
}

impl Default for ParserConfig {
    fn default() -> Self {
        Self {
            max_header_bytes: 64 * 1024,
            max_body_bytes: 16 * 1024 * 1024,
            max_chunk_bytes: 8 * 1024 * 1024,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ParserMode {
    Request,
    Response,
}

pub struct Http1Parser {
    mode: ParserMode,
    config: ParserConfig,
    buffer: Vec<u8>,
    consumed: usize,
}

impl Http1Parser {
    pub fn request() -> Self {
        Self::request_with_config(ParserConfig::default())
    }

    pub fn response() -> Self {
        Self::response_with_config(ParserConfig::default())
    }

    pub fn request_with_config(config: ParserConfig) -> Self {
        Self::new(ParserMode::Request, config)
    }

    pub fn response_with_config(config: ParserConfig) -> Self {
        Self::new(ParserMode::Response, config)
    }

    fn new(mode: ParserMode, config: ParserConfig) -> Self {
        Self {
            mode,
            config,
            buffer: Vec::new(),
            consumed: 0,
        }
    }

    pub fn push(&mut self, bytes: &[u8]) {
        self.buffer.extend_from_slice(bytes);
    }

    pub fn buffered_len(&self) -> usize {
        self.buffer.len()
    }

    pub fn parse_next_request(&mut self) -> Result<Option<ParsedRequest>, ParseError> {
        if self.mode != ParserMode::Request {
            return Err(ParseError::new(
                ParseErrorKind::InvalidStartLine,
                self.consumed,
                "parser is configured for HTTP responses",
            ));
        }
        let Some(message) = self.parse_next_message()? else {
            return Ok(None);
        };
        match message {
            ParsedMessage::Request(request) => Ok(Some(request)),
            ParsedMessage::Response(_) => unreachable!("request parser returned a response"),
        }
    }

    pub fn parse_next_response(&mut self) -> Result<Option<ParsedResponse>, ParseError> {
        if self.mode != ParserMode::Response {
            return Err(ParseError::new(
                ParseErrorKind::InvalidStartLine,
                self.consumed,
                "parser is configured for HTTP requests",
            ));
        }
        let Some(message) = self.parse_next_message()? else {
            return Ok(None);
        };
        match message {
            ParsedMessage::Response(response) => Ok(Some(response)),
            ParsedMessage::Request(_) => unreachable!("response parser returned a request"),
        }
    }

    fn parse_next_message(&mut self) -> Result<Option<ParsedMessage>, ParseError> {
        let Some(header_end) = find_header_end(&self.buffer) else {
            if self.buffer.len() > self.config.max_header_bytes {
                return Err(ParseError::new(
                    ParseErrorKind::HeaderTooLarge,
                    self.consumed + self.config.max_header_bytes,
                    "HTTP header section exceeds configured limit",
                ));
            }
            return Ok(None);
        };
        if header_end > self.config.max_header_bytes {
            return Err(ParseError::new(
                ParseErrorKind::HeaderTooLarge,
                self.consumed + self.config.max_header_bytes,
                "HTTP header section exceeds configured limit",
            ));
        }

        let (start_line, headers) =
            parse_head(&self.buffer[..header_end], self.consumed, self.mode)?;
        let body_start = header_end + 4;
        let body_meta = BodyMeta::from_headers(&headers, self.consumed)?;
        let Some((body, consumed_body_bytes)) = parse_body(
            &self.buffer,
            body_start,
            self.consumed,
            &self.config,
            &body_meta,
        )?
        else {
            return Ok(None);
        };

        let total_len = body_start + consumed_body_bytes;
        let keep_alive = determine_keep_alive(&start_line.version, &headers);
        let parsed = match (self.mode, start_line.kind) {
            (ParserMode::Request, StartLineKind::Request { method, target, .. }) => {
                ParsedMessage::Request(ParsedRequest {
                    method,
                    target,
                    version: start_line.version,
                    headers,
                    body,
                    keep_alive,
                })
            }
            (
                ParserMode::Response,
                StartLineKind::Response {
                    status_code,
                    reason,
                },
            ) => ParsedMessage::Response(ParsedResponse {
                version: start_line.version,
                status_code,
                reason,
                headers,
                body,
                keep_alive,
            }),
            _ => unreachable!("parser mode and start-line kind diverged"),
        };

        self.buffer.drain(..total_len);
        self.consumed += total_len;
        Ok(Some(parsed))
    }
}

enum ParsedMessage {
    Request(ParsedRequest),
    Response(ParsedResponse),
}

struct StartLine {
    version: HttpVersion,
    kind: StartLineKind,
}

enum StartLineKind {
    Request { method: String, target: String },
    Response { status_code: u16, reason: String },
}

enum BodyMeta {
    Empty,
    ContentLength(usize),
    Chunked,
}

struct HttpStore {
    requests: ApiHandleTable<Request>,
    responses: ApiHandleTable<Response>,
    headers: ApiHandleTable<Header>,
    cookies: ApiHandleTable<Cookie>,
}

impl HttpStore {
    fn new() -> Self {
        Self {
            requests: ApiHandleTable::new(HandleKind::ApiHttpRequest),
            responses: ApiHandleTable::new(HandleKind::ApiHttpResponse),
            headers: ApiHandleTable::new(HandleKind::ApiHttpHeader),
            cookies: ApiHandleTable::new(HandleKind::ApiHttpCookie),
        }
    }

    fn request_handle(&mut self, request: Request) -> SpectraHostValue {
        self.requests.insert(request)
    }

    fn response_handle(&mut self, response: Response) -> SpectraHostValue {
        self.responses.insert(response)
    }

    fn header_handle(&mut self, header: Header) -> SpectraHostValue {
        self.headers.insert(header)
    }

    fn cookie_handle(&mut self, cookie: Cookie) -> SpectraHostValue {
        self.cookies.insert(cookie)
    }
}

fn store() -> &'static Mutex<HttpStore> {
    static STORE: OnceLock<Mutex<HttpStore>> = OnceLock::new();
    STORE.get_or_init(|| Mutex::new(HttpStore::new()))
}

pub(crate) fn store_response(response: Response) -> SpectraHostValue {
    let mut store = store().lock().unwrap_or_else(|e| e.into_inner());
    store.response_handle(response)
}

pub(crate) fn store_request(request: Request) -> SpectraHostValue {
    let mut store = store().lock().unwrap_or_else(|e| e.into_inner());
    store.request_handle(request)
}

pub(crate) fn clone_request(handle: SpectraHostValue) -> Option<Request> {
    let store = store().lock().unwrap_or_else(|e| e.into_inner());
    store.requests.get(&handle).cloned()
}

pub(crate) fn clone_response(handle: SpectraHostValue) -> Option<Response> {
    let store = store().lock().unwrap_or_else(|e| e.into_inner());
    store.responses.get(&handle).cloned()
}

pub fn parse_request(bytes: &[u8]) -> Result<ParsedRequest, ParseError> {
    let mut parser = Http1Parser::request();
    parser.push(bytes);
    match parser.parse_next_request()? {
        Some(request) if parser.buffered_len() == 0 => Ok(request),
        Some(_) => Err(ParseError::new(
            ParseErrorKind::InvalidStartLine,
            bytes.len() - parser.buffered_len(),
            "input contains trailing bytes after one HTTP request",
        )),
        None => Err(ParseError::new(
            ParseErrorKind::Incomplete,
            bytes.len(),
            "incomplete HTTP request",
        )),
    }
}

pub fn parse_response(bytes: &[u8]) -> Result<ParsedResponse, ParseError> {
    let mut parser = Http1Parser::response();
    parser.push(bytes);
    match parser.parse_next_response()? {
        Some(response) if parser.buffered_len() == 0 => Ok(response),
        Some(_) => Err(ParseError::new(
            ParseErrorKind::InvalidStartLine,
            bytes.len() - parser.buffered_len(),
            "input contains trailing bytes after one HTTP response",
        )),
        None => Err(ParseError::new(
            ParseErrorKind::Incomplete,
            bytes.len(),
            "incomplete HTTP response",
        )),
    }
}

pub fn serialize_request(request: &ParsedRequest) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(request.method.as_bytes());
    out.push(b' ');
    out.extend_from_slice(request.target.as_bytes());
    out.push(b' ');
    out.extend_from_slice(request.version.to_string().as_bytes());
    out.extend_from_slice(b"\r\n");
    write_headers(&mut out, &request.headers);
    out.extend_from_slice(b"\r\n");
    write_body(&mut out, &request.body);
    out
}

pub fn serialize_response(response: &ParsedResponse) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(response.version.to_string().as_bytes());
    out.push(b' ');
    out.extend_from_slice(response.status_code.to_string().as_bytes());
    out.push(b' ');
    out.extend_from_slice(response.reason.as_bytes());
    out.extend_from_slice(b"\r\n");
    write_headers(&mut out, &response.headers);
    out.extend_from_slice(b"\r\n");
    write_body(&mut out, &response.body);
    out
}

fn find_header_end(bytes: &[u8]) -> Option<usize> {
    bytes.windows(4).position(|window| window == b"\r\n\r\n")
}

fn parse_head(
    bytes: &[u8],
    absolute_base: usize,
    mode: ParserMode,
) -> Result<(StartLine, Vec<Header>), ParseError> {
    let mut line_start = 0usize;
    let Some(first_line_end) = find_crlf(bytes, line_start) else {
        return Err(ParseError::new(
            ParseErrorKind::InvalidStartLine,
            absolute_base,
            "HTTP start line is missing CRLF",
        ));
    };
    let start_line = parse_start_line(&bytes[line_start..first_line_end], absolute_base, mode)?;
    line_start = first_line_end + 2;

    let mut headers = Vec::new();
    while line_start < bytes.len() {
        let line_end = find_crlf(bytes, line_start).unwrap_or(bytes.len());
        let line = &bytes[line_start..line_end];
        if line.is_empty() {
            break;
        }
        if line[0] == b' ' || line[0] == b'\t' {
            return Err(ParseError::new(
                ParseErrorKind::ObsoleteLineFolding,
                absolute_base + line_start,
                "obsolete folded HTTP headers are rejected",
            ));
        }
        headers.push(parse_header(line, absolute_base + line_start)?);
        line_start = if line_end == bytes.len() {
            bytes.len()
        } else {
            line_end + 2
        };
    }

    Ok((start_line, headers))
}

fn find_crlf(bytes: &[u8], start: usize) -> Option<usize> {
    bytes
        .get(start..)?
        .windows(2)
        .position(|window| window == b"\r\n")
        .map(|offset| start + offset)
}

fn parse_start_line(
    line: &[u8],
    absolute_position: usize,
    mode: ParserMode,
) -> Result<StartLine, ParseError> {
    let line_text = std::str::from_utf8(line).map_err(|_| {
        ParseError::new(
            ParseErrorKind::InvalidStartLine,
            absolute_position,
            "HTTP start line must be valid ASCII/UTF-8",
        )
    })?;
    match mode {
        ParserMode::Request => parse_request_line(line_text, absolute_position),
        ParserMode::Response => parse_status_line(line_text, absolute_position),
    }
}

fn parse_request_line(line: &str, absolute_position: usize) -> Result<StartLine, ParseError> {
    let mut parts = line.split(' ');
    let method = parts.next().unwrap_or_default();
    let target = parts.next().unwrap_or_default();
    let version = parts.next().unwrap_or_default();
    if parts.next().is_some() || method.is_empty() || target.is_empty() || version.is_empty() {
        return Err(ParseError::new(
            ParseErrorKind::InvalidStartLine,
            absolute_position,
            "HTTP request line must be METHOD target HTTP-version",
        ));
    }
    if !is_token(method) {
        return Err(ParseError::new(
            ParseErrorKind::InvalidMethod,
            absolute_position,
            "HTTP method contains invalid token characters",
        ));
    }
    if target.bytes().any(|b| b <= b' ' || b == 0x7f) {
        return Err(ParseError::new(
            ParseErrorKind::InvalidTarget,
            absolute_position + method.len() + 1,
            "HTTP request target contains invalid whitespace or control bytes",
        ));
    }
    let version = parse_version(version, absolute_position + method.len() + target.len() + 2)?;
    Ok(StartLine {
        version,
        kind: StartLineKind::Request {
            method: method.to_string(),
            target: target.to_string(),
        },
    })
}

fn parse_status_line(line: &str, absolute_position: usize) -> Result<StartLine, ParseError> {
    let mut parts = line.splitn(3, ' ');
    let version_text = parts.next().unwrap_or_default();
    let status_text = parts.next().unwrap_or_default();
    let reason = parts.next().unwrap_or_default();
    if version_text.is_empty() || status_text.is_empty() {
        return Err(ParseError::new(
            ParseErrorKind::InvalidStartLine,
            absolute_position,
            "HTTP status line must be HTTP-version status-code reason",
        ));
    }
    let version = parse_version(version_text, absolute_position)?;
    if status_text.len() != 3 || !status_text.bytes().all(|b| b.is_ascii_digit()) {
        return Err(ParseError::new(
            ParseErrorKind::InvalidStatus,
            absolute_position + version_text.len() + 1,
            "HTTP status code must contain exactly three digits",
        ));
    }
    let status_code: u16 = status_text.parse().map_err(|_| {
        ParseError::new(
            ParseErrorKind::InvalidStatus,
            absolute_position + version_text.len() + 1,
            "HTTP status code is outside the supported range",
        )
    })?;
    if !(100..=999).contains(&status_code) {
        return Err(ParseError::new(
            ParseErrorKind::InvalidStatus,
            absolute_position + version_text.len() + 1,
            "HTTP status code is outside the supported range",
        ));
    }
    if !is_reason_phrase(reason) {
        return Err(ParseError::new(
            ParseErrorKind::InvalidStatus,
            absolute_position + version_text.len() + status_text.len() + 2,
            "HTTP reason phrase contains invalid control bytes",
        ));
    }
    Ok(StartLine {
        version,
        kind: StartLineKind::Response {
            status_code,
            reason: reason.to_string(),
        },
    })
}

fn parse_version(text: &str, position: usize) -> Result<HttpVersion, ParseError> {
    let Some(rest) = text.strip_prefix("HTTP/") else {
        return Err(ParseError::new(
            ParseErrorKind::InvalidVersion,
            position,
            "HTTP version must start with HTTP/",
        ));
    };
    let Some((major, minor)) = rest.split_once('.') else {
        return Err(ParseError::new(
            ParseErrorKind::InvalidVersion,
            position,
            "HTTP version must contain major and minor numbers",
        ));
    };
    if major.is_empty()
        || minor.is_empty()
        || !major.bytes().all(|b| b.is_ascii_digit())
        || !minor.bytes().all(|b| b.is_ascii_digit())
    {
        return Err(ParseError::new(
            ParseErrorKind::InvalidVersion,
            position,
            "HTTP version numbers must be decimal digits",
        ));
    }
    let major: u8 = major.parse().map_err(|_| {
        ParseError::new(
            ParseErrorKind::InvalidVersion,
            position,
            "HTTP major version is too large",
        )
    })?;
    let minor: u8 = minor.parse().map_err(|_| {
        ParseError::new(
            ParseErrorKind::InvalidVersion,
            position,
            "HTTP minor version is too large",
        )
    })?;
    Ok(HttpVersion { major, minor })
}

fn parse_header(line: &[u8], absolute_position: usize) -> Result<Header, ParseError> {
    let Some(colon) = line.iter().position(|byte| *byte == b':') else {
        return Err(ParseError::new(
            ParseErrorKind::InvalidHeader,
            absolute_position,
            "HTTP header line is missing ':'",
        ));
    };
    let name = bytes_to_ascii(&line[..colon], absolute_position)?;
    if !is_valid_header_name(&name) {
        return Err(ParseError::new(
            ParseErrorKind::InvalidHeader,
            absolute_position,
            "HTTP header field-name is invalid",
        ));
    }
    let mut value_start = colon + 1;
    while value_start < line.len() && matches!(line[value_start], b' ' | b'\t') {
        value_start += 1;
    }
    let mut value_end = line.len();
    while value_end > value_start && matches!(line[value_end - 1], b' ' | b'\t') {
        value_end -= 1;
    }
    let value = bytes_to_http_value(
        &line[value_start..value_end],
        absolute_position + value_start,
    )?;
    if !is_valid_header_value(&value) {
        return Err(ParseError::new(
            ParseErrorKind::InvalidHeader,
            absolute_position + value_start,
            "HTTP header field-value contains invalid bytes",
        ));
    }
    Ok(Header { name, value })
}

impl BodyMeta {
    fn from_headers(headers: &[Header], absolute_base: usize) -> Result<Self, ParseError> {
        let transfer_encoding = header_values(headers, "transfer-encoding");
        if !transfer_encoding.is_empty() {
            let codings = transfer_encoding
                .iter()
                .flat_map(|value| value.split(','))
                .map(|part| part.trim().to_ascii_lowercase())
                .filter(|part| !part.is_empty())
                .collect::<Vec<_>>();
            if codings.last().map(|coding| coding.as_str()) != Some("chunked") {
                return Err(ParseError::new(
                    ParseErrorKind::UnsupportedTransferEncoding,
                    absolute_base,
                    "only chunked transfer-coding is supported for HTTP/1.1 messages",
                ));
            }
            if codings.iter().any(|coding| coding != "chunked")
                || codings[..codings.len().saturating_sub(1)]
                    .iter()
                    .any(|coding| coding == "chunked")
            {
                return Err(ParseError::new(
                    ParseErrorKind::UnsupportedTransferEncoding,
                    absolute_base,
                    "only a single final chunked transfer-coding is supported",
                ));
            }
            return Ok(Self::Chunked);
        }

        let lengths = header_values(headers, "content-length");
        if lengths.is_empty() {
            return Ok(Self::Empty);
        }
        let mut parsed = None;
        for value in lengths {
            let length = value.parse::<usize>().map_err(|_| {
                ParseError::new(
                    ParseErrorKind::BodyLengthMismatch,
                    absolute_base,
                    "Content-Length must be a non-negative decimal integer",
                )
            })?;
            if let Some(existing) = parsed {
                if existing != length {
                    return Err(ParseError::new(
                        ParseErrorKind::BodyLengthMismatch,
                        absolute_base,
                        "conflicting Content-Length values are rejected",
                    ));
                }
            }
            parsed = Some(length);
        }
        Ok(Self::ContentLength(parsed.unwrap_or(0)))
    }
}

fn header_values<'a>(headers: &'a [Header], name: &str) -> Vec<&'a str> {
    headers
        .iter()
        .filter(|header| header.name.eq_ignore_ascii_case(name))
        .map(|header| header.value.as_str())
        .collect()
}

fn parse_body(
    buffer: &[u8],
    body_start: usize,
    absolute_base: usize,
    config: &ParserConfig,
    meta: &BodyMeta,
) -> Result<Option<(HttpBody, usize)>, ParseError> {
    match meta {
        BodyMeta::Empty => Ok(Some((HttpBody::empty(), 0))),
        BodyMeta::ContentLength(length) => {
            if *length > config.max_body_bytes {
                return Err(ParseError::new(
                    ParseErrorKind::BodyTooLarge,
                    absolute_base + body_start,
                    "HTTP body exceeds configured limit",
                ));
            }
            let available = buffer.len().saturating_sub(body_start);
            if available < *length {
                return Ok(None);
            }
            let bytes = buffer[body_start..body_start + length].to_vec();
            Ok(Some((HttpBody::from_bytes(bytes), *length)))
        }
        BodyMeta::Chunked => parse_chunked_body(buffer, body_start, absolute_base, config),
    }
}

fn parse_chunked_body(
    buffer: &[u8],
    body_start: usize,
    absolute_base: usize,
    config: &ParserConfig,
) -> Result<Option<(HttpBody, usize)>, ParseError> {
    let mut cursor = body_start;
    let mut body = HttpBody {
        chunks: Vec::new(),
        trailers: Vec::new(),
        chunked: true,
    };
    let mut total_data = 0usize;

    loop {
        let Some(size_line_end) = find_crlf(buffer, cursor) else {
            return Ok(None);
        };
        let size_line = &buffer[cursor..size_line_end];
        let (size, extension) = parse_chunk_size_line(size_line, absolute_base + cursor)?;
        cursor = size_line_end + 2;
        if size > config.max_chunk_bytes {
            return Err(ParseError::new(
                ParseErrorKind::BodyTooLarge,
                absolute_base + cursor,
                "HTTP chunk exceeds configured per-chunk limit",
            ));
        }
        total_data = total_data.saturating_add(size);
        if total_data > config.max_body_bytes {
            return Err(ParseError::new(
                ParseErrorKind::BodyTooLarge,
                absolute_base + cursor,
                "HTTP chunked body exceeds configured body limit",
            ));
        }
        if size == 0 {
            let Some((trailers, consumed_trailers)) =
                parse_trailer_section(buffer, cursor, absolute_base)?
            else {
                return Ok(None);
            };
            body.trailers = trailers;
            let consumed = cursor + consumed_trailers - body_start;
            return Ok(Some((body, consumed)));
        }
        if buffer.len() < cursor + size + 2 {
            return Ok(None);
        }
        if &buffer[cursor + size..cursor + size + 2] != b"\r\n" {
            return Err(ParseError::new(
                ParseErrorKind::InvalidChunkTerminator,
                absolute_base + cursor + size,
                "HTTP chunk data must be followed by CRLF",
            ));
        }
        body.chunks.push(BodyChunk {
            data: buffer[cursor..cursor + size].to_vec(),
            extension,
        });
        cursor += size + 2;
    }
}

fn parse_trailer_section(
    buffer: &[u8],
    cursor: usize,
    absolute_base: usize,
) -> Result<Option<(Vec<Header>, usize)>, ParseError> {
    let mut headers = Vec::new();
    let mut line_start = cursor;
    loop {
        let Some(line_end) = find_crlf(buffer, line_start) else {
            return Ok(None);
        };
        let line = &buffer[line_start..line_end];
        if line.is_empty() {
            return Ok(Some((headers, line_end + 2 - cursor)));
        }
        if line[0] == b' ' || line[0] == b'\t' {
            return Err(ParseError::new(
                ParseErrorKind::ObsoleteLineFolding,
                absolute_base + line_start,
                "obsolete folded HTTP trailers are rejected",
            ));
        }
        headers.push(parse_header(line, absolute_base + line_start)?);
        line_start = line_end + 2;
    }
}

fn parse_chunk_size_line(
    line: &[u8],
    absolute_position: usize,
) -> Result<(usize, Option<String>), ParseError> {
    let text = std::str::from_utf8(line).map_err(|_| {
        ParseError::new(
            ParseErrorKind::InvalidChunkSize,
            absolute_position,
            "HTTP chunk-size line must be valid ASCII/UTF-8",
        )
    })?;
    let (size_text, extension) = match text.split_once(';') {
        Some((size, ext)) => (size.trim(), Some(ext.trim().to_string())),
        None => (text.trim(), None),
    };
    if size_text.is_empty() || !size_text.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(ParseError::new(
            ParseErrorKind::InvalidChunkSize,
            absolute_position,
            "HTTP chunk-size must be hexadecimal",
        ));
    }
    let size = usize::from_str_radix(size_text, 16).map_err(|_| {
        ParseError::new(
            ParseErrorKind::InvalidChunkSize,
            absolute_position,
            "HTTP chunk-size is too large",
        )
    })?;
    Ok((size, extension.filter(|value| !value.is_empty())))
}

fn determine_keep_alive(version: &HttpVersion, headers: &[Header]) -> bool {
    let connection_tokens = header_values(headers, "connection")
        .into_iter()
        .flat_map(|value| value.split(','))
        .map(|token| token.trim().to_ascii_lowercase())
        .collect::<Vec<_>>();
    if connection_tokens.iter().any(|token| token == "close") {
        return false;
    }
    if connection_tokens.iter().any(|token| token == "keep-alive") {
        return true;
    }
    version.major > 1 || (version.major == 1 && version.minor >= 1)
}

fn bytes_to_ascii(bytes: &[u8], absolute_position: usize) -> Result<String, ParseError> {
    if bytes.iter().any(|byte| !byte.is_ascii()) {
        return Err(ParseError::new(
            ParseErrorKind::InvalidHeader,
            absolute_position,
            "HTTP header field-name must be ASCII",
        ));
    }
    std::str::from_utf8(bytes)
        .map(|value| value.to_string())
        .map_err(|_| {
            ParseError::new(
                ParseErrorKind::InvalidHeader,
                absolute_position,
                "HTTP header field-name is not valid text",
            )
        })
}

fn bytes_to_http_value(bytes: &[u8], absolute_position: usize) -> Result<String, ParseError> {
    let mut out = String::new();
    for (idx, byte) in bytes.iter().copied().enumerate() {
        if byte == b'\t' || byte == b' ' || (0x21..=0x7e).contains(&byte) || byte >= 0x80 {
            out.push(char::from(byte));
        } else {
            return Err(ParseError::new(
                ParseErrorKind::InvalidHeader,
                absolute_position + idx,
                "HTTP header field-value contains a disallowed control byte",
            ));
        }
    }
    Ok(out)
}

fn is_token(value: &str) -> bool {
    !value.is_empty() && value.bytes().all(is_tchar)
}

fn is_tchar(byte: u8) -> bool {
    byte.is_ascii_alphanumeric()
        || matches!(
            byte,
            b'!' | b'#'
                | b'$'
                | b'%'
                | b'&'
                | b'\''
                | b'*'
                | b'+'
                | b'-'
                | b'.'
                | b'^'
                | b'_'
                | b'`'
                | b'|'
                | b'~'
        )
}

fn is_reason_phrase(value: &str) -> bool {
    value
        .bytes()
        .all(|byte| byte == b'\t' || byte == b' ' || (0x21..=0x7e).contains(&byte) || byte >= 0x80)
}

fn write_headers(out: &mut Vec<u8>, headers: &[Header]) {
    for header in headers {
        out.extend_from_slice(header.name.as_bytes());
        out.extend_from_slice(b": ");
        out.extend_from_slice(header.value.as_bytes());
        out.extend_from_slice(b"\r\n");
    }
}

fn write_body(out: &mut Vec<u8>, body: &HttpBody) {
    if body.chunked {
        for chunk in &body.chunks {
            out.extend_from_slice(format!("{:X}", chunk.data.len()).as_bytes());
            if let Some(extension) = &chunk.extension {
                out.push(b';');
                out.extend_from_slice(extension.as_bytes());
            }
            out.extend_from_slice(b"\r\n");
            out.extend_from_slice(&chunk.data);
            out.extend_from_slice(b"\r\n");
        }
        out.extend_from_slice(b"0\r\n");
        write_headers(out, &body.trailers);
        out.extend_from_slice(b"\r\n");
    } else {
        for chunk in &body.chunks {
            out.extend_from_slice(&chunk.data);
        }
    }
}

pub extern "C" fn method_name(ctx: *mut SpectraHostCallContext) -> i32 {
    let Ok(args) = read_args(ctx, 1) else {
        return HOST_STATUS_INVALID_ARGUMENT;
    };
    write_result(ctx, alloc_spectra_string(method_label(args[0])))
}

pub extern "C" fn method_allows_body(ctx: *mut SpectraHostCallContext) -> i32 {
    let Ok(args) = read_args(ctx, 1) else {
        return HOST_STATUS_INVALID_ARGUMENT;
    };
    let allows = matches!(args[0], METHOD_POST | METHOD_PUT | METHOD_PATCH);
    write_result(ctx, allows as SpectraHostValue)
}

pub extern "C" fn method_is_safe(ctx: *mut SpectraHostCallContext) -> i32 {
    let Ok(args) = read_args(ctx, 1) else {
        return HOST_STATUS_INVALID_ARGUMENT;
    };
    let safe = matches!(args[0], METHOD_GET | METHOD_HEAD | METHOD_OPTIONS);
    write_result(ctx, safe as SpectraHostValue)
}

macro_rules! const_host_call {
    ($name:ident, $value:expr) => {
        pub extern "C" fn $name(ctx: *mut SpectraHostCallContext) -> i32 {
            write_result(ctx, $value)
        }
    };
}

const_host_call!(method_get, METHOD_GET);
const_host_call!(method_head, METHOD_HEAD);
const_host_call!(method_post, METHOD_POST);
const_host_call!(method_put, METHOD_PUT);
const_host_call!(method_patch, METHOD_PATCH);
const_host_call!(method_delete, METHOD_DELETE);
const_host_call!(method_options, METHOD_OPTIONS);
const_host_call!(status_continue, 100);
const_host_call!(status_switching_protocols, 101);
const_host_call!(status_ok, 200);
const_host_call!(status_created, 201);
const_host_call!(status_accepted, 202);
const_host_call!(status_no_content, 204);
const_host_call!(status_moved_permanently, 301);
const_host_call!(status_found, 302);
const_host_call!(status_not_modified, 304);
const_host_call!(status_bad_request, 400);
const_host_call!(status_unauthorized, 401);
const_host_call!(status_forbidden, 403);
const_host_call!(status_not_found, 404);
const_host_call!(status_method_not_allowed, 405);
const_host_call!(status_conflict, 409);
const_host_call!(status_unsupported_media_type, 415);
const_host_call!(status_unprocessable_content, 422);
const_host_call!(status_too_many_requests, 429);
const_host_call!(status_internal_server_error, 500);
const_host_call!(status_bad_gateway, 502);
const_host_call!(status_service_unavailable, 503);
const_host_call!(status_gateway_timeout, 504);

pub extern "C" fn status_reason(ctx: *mut SpectraHostCallContext) -> i32 {
    let Ok(args) = read_args(ctx, 1) else {
        return HOST_STATUS_INVALID_ARGUMENT;
    };
    write_result(ctx, alloc_spectra_string(status_reason_phrase(args[0])))
}

pub extern "C" fn status_class(ctx: *mut SpectraHostCallContext) -> i32 {
    let Ok(args) = read_args(ctx, 1) else {
        return HOST_STATUS_INVALID_ARGUMENT;
    };
    let status = args[0];
    let class = if (100..=599).contains(&status) {
        status / 100
    } else {
        0
    };
    write_result(ctx, class)
}

pub extern "C" fn status_is_success(ctx: *mut SpectraHostCallContext) -> i32 {
    let Ok(args) = read_args(ctx, 1) else {
        return HOST_STATUS_INVALID_ARGUMENT;
    };
    write_result(ctx, ((200..=299).contains(&args[0])) as SpectraHostValue)
}

pub extern "C" fn status(ctx: *mut SpectraHostCallContext) -> i32 {
    let Ok(args) = read_args(ctx, 1) else {
        return HOST_STATUS_INVALID_ARGUMENT;
    };
    let Ok(code) = u16::try_from(args[0]) else {
        return HOST_STATUS_INVALID_ARGUMENT;
    };
    if Status::new(code).is_err() {
        return HOST_STATUS_INVALID_ARGUMENT;
    }
    write_result(ctx, args[0])
}

pub extern "C" fn header_name_is_valid(ctx: *mut SpectraHostCallContext) -> i32 {
    let Ok(args) = read_args(ctx, 1) else {
        return HOST_STATUS_INVALID_ARGUMENT;
    };
    let valid = read_spectra_string(args[0])
        .map(|name| is_valid_header_name(&name))
        .unwrap_or(false);
    write_result(ctx, valid as SpectraHostValue)
}

pub extern "C" fn header_value_is_valid(ctx: *mut SpectraHostCallContext) -> i32 {
    let Ok(args) = read_args(ctx, 1) else {
        return HOST_STATUS_INVALID_ARGUMENT;
    };
    let valid = read_spectra_string(args[0])
        .map(|value| is_valid_header_value(&value))
        .unwrap_or(false);
    write_result(ctx, valid as SpectraHostValue)
}

pub extern "C" fn request_new(ctx: *mut SpectraHostCallContext) -> i32 {
    let Ok(args) = read_args(ctx, 1) else {
        return HOST_STATUS_INVALID_ARGUMENT;
    };
    let Some(method) = Method::from_code(args[0]) else {
        return HOST_STATUS_INVALID_ARGUMENT;
    };
    let Ok(request) = Request::new(method, "/") else {
        return HOST_STATUS_INVALID_ARGUMENT;
    };
    let mut store = store().lock().unwrap_or_else(|e| e.into_inner());
    write_result(ctx, store.request_handle(request))
}

pub extern "C" fn request(ctx: *mut SpectraHostCallContext) -> i32 {
    let Ok(args) = read_args(ctx, 2) else {
        return HOST_STATUS_INVALID_ARGUMENT;
    };
    let Some(method) = Method::from_code(args[0]) else {
        return HOST_STATUS_INVALID_ARGUMENT;
    };
    let Some(path) = read_spectra_string(args[1]) else {
        return HOST_STATUS_INVALID_ARGUMENT;
    };
    let Ok(request) = Request::new(method, path) else {
        return HOST_STATUS_INVALID_ARGUMENT;
    };
    let mut store = store().lock().unwrap_or_else(|e| e.into_inner());
    write_result(ctx, store.request_handle(request))
}

pub extern "C" fn request_method(ctx: *mut SpectraHostCallContext) -> i32 {
    let Ok(args) = read_args(ctx, 1) else {
        return HOST_STATUS_INVALID_ARGUMENT;
    };
    let store = store().lock().unwrap_or_else(|e| e.into_inner());
    let Some(request) = store.requests.get(&args[0]) else {
        return HOST_STATUS_INVALID_ARGUMENT;
    };
    write_result(ctx, request.method.code())
}

pub extern "C" fn request_path(ctx: *mut SpectraHostCallContext) -> i32 {
    let Ok(args) = read_args(ctx, 1) else {
        return HOST_STATUS_INVALID_ARGUMENT;
    };
    let store = store().lock().unwrap_or_else(|e| e.into_inner());
    let Some(request) = store.requests.get(&args[0]) else {
        return HOST_STATUS_INVALID_ARGUMENT;
    };
    write_result(ctx, alloc_spectra_string(&request.path))
}

pub extern "C" fn request_header(ctx: *mut SpectraHostCallContext) -> i32 {
    let Ok(args) = read_args(ctx, 2) else {
        return HOST_STATUS_INVALID_ARGUMENT;
    };
    let Some(name) = read_spectra_string(args[1]) else {
        return HOST_STATUS_INVALID_ARGUMENT;
    };
    let store = store().lock().unwrap_or_else(|e| e.into_inner());
    let Some(request) = store.requests.get(&args[0]) else {
        return HOST_STATUS_INVALID_ARGUMENT;
    };
    write_result(
        ctx,
        alloc_spectra_string(request.header(&name).unwrap_or("")),
    )
}

pub extern "C" fn request_with_header(ctx: *mut SpectraHostCallContext) -> i32 {
    let Ok(args) = read_args(ctx, 3) else {
        return HOST_STATUS_INVALID_ARGUMENT;
    };
    let (Some(name), Some(value)) = (read_spectra_string(args[1]), read_spectra_string(args[2]))
    else {
        return HOST_STATUS_INVALID_ARGUMENT;
    };
    let request = {
        let store = store().lock().unwrap_or_else(|e| e.into_inner());
        let Some(request) = store.requests.get(&args[0]).cloned() else {
            return HOST_STATUS_INVALID_ARGUMENT;
        };
        request
    };
    let Ok(request) = request.with_header(name, value) else {
        return HOST_STATUS_INVALID_ARGUMENT;
    };
    let mut store = store().lock().unwrap_or_else(|e| e.into_inner());
    write_result(ctx, store.request_handle(request))
}

pub extern "C" fn request_cookie(ctx: *mut SpectraHostCallContext) -> i32 {
    let Ok(args) = read_args(ctx, 2) else {
        return HOST_STATUS_INVALID_ARGUMENT;
    };
    let Some(name) = read_spectra_string(args[1]) else {
        return HOST_STATUS_INVALID_ARGUMENT;
    };
    let store = store().lock().unwrap_or_else(|e| e.into_inner());
    let Some(request) = store.requests.get(&args[0]) else {
        return HOST_STATUS_INVALID_ARGUMENT;
    };
    write_result(
        ctx,
        alloc_spectra_string(&request.cookie(&name).unwrap_or_default()),
    )
}

pub extern "C" fn response_new(ctx: *mut SpectraHostCallContext) -> i32 {
    let Ok(args) = read_args(ctx, 1) else {
        return HOST_STATUS_INVALID_ARGUMENT;
    };
    let Ok(status) = u16::try_from(args[0])
        .map_err(|_| ())
        .and_then(|code| Status::new(code).map_err(|_| ()))
    else {
        return HOST_STATUS_INVALID_ARGUMENT;
    };
    let mut store = store().lock().unwrap_or_else(|e| e.into_inner());
    write_result(ctx, store.response_handle(Response::new(status)))
}

pub extern "C" fn response_status(ctx: *mut SpectraHostCallContext) -> i32 {
    let Ok(args) = read_args(ctx, 1) else {
        return HOST_STATUS_INVALID_ARGUMENT;
    };
    let store = store().lock().unwrap_or_else(|e| e.into_inner());
    let Some(response) = store.responses.get(&args[0]) else {
        return HOST_STATUS_INVALID_ARGUMENT;
    };
    write_result(ctx, response.status.code() as SpectraHostValue)
}

pub extern "C" fn response_header(ctx: *mut SpectraHostCallContext) -> i32 {
    let Ok(args) = read_args(ctx, 2) else {
        return HOST_STATUS_INVALID_ARGUMENT;
    };
    let Some(name) = read_spectra_string(args[1]) else {
        return HOST_STATUS_INVALID_ARGUMENT;
    };
    let store = store().lock().unwrap_or_else(|e| e.into_inner());
    let Some(response) = store.responses.get(&args[0]) else {
        return HOST_STATUS_INVALID_ARGUMENT;
    };
    write_result(
        ctx,
        alloc_spectra_string(response.header(&name).unwrap_or("")),
    )
}

pub extern "C" fn response_body_len(ctx: *mut SpectraHostCallContext) -> i32 {
    let Ok(args) = read_args(ctx, 1) else {
        return HOST_STATUS_INVALID_ARGUMENT;
    };
    let store = store().lock().unwrap_or_else(|e| e.into_inner());
    let Some(response) = store.responses.get(&args[0]) else {
        return HOST_STATUS_INVALID_ARGUMENT;
    };
    write_result(ctx, response.body.len() as SpectraHostValue)
}

pub extern "C" fn header(ctx: *mut SpectraHostCallContext) -> i32 {
    let Ok(args) = read_args(ctx, 2) else {
        return HOST_STATUS_INVALID_ARGUMENT;
    };
    let (Some(name), Some(value)) = (read_spectra_string(args[0]), read_spectra_string(args[1]))
    else {
        return HOST_STATUS_INVALID_ARGUMENT;
    };
    let Ok(header) = Header::new(name, value) else {
        return HOST_STATUS_INVALID_ARGUMENT;
    };
    let mut store = store().lock().unwrap_or_else(|e| e.into_inner());
    write_result(ctx, store.header_handle(header))
}

pub extern "C" fn header_name(ctx: *mut SpectraHostCallContext) -> i32 {
    let Ok(args) = read_args(ctx, 1) else {
        return HOST_STATUS_INVALID_ARGUMENT;
    };
    let store = store().lock().unwrap_or_else(|e| e.into_inner());
    let Some(header) = store.headers.get(&args[0]) else {
        return HOST_STATUS_INVALID_ARGUMENT;
    };
    write_result(ctx, alloc_spectra_string(&header.name))
}

pub extern "C" fn header_value(ctx: *mut SpectraHostCallContext) -> i32 {
    let Ok(args) = read_args(ctx, 1) else {
        return HOST_STATUS_INVALID_ARGUMENT;
    };
    let store = store().lock().unwrap_or_else(|e| e.into_inner());
    let Some(header) = store.headers.get(&args[0]) else {
        return HOST_STATUS_INVALID_ARGUMENT;
    };
    write_result(ctx, alloc_spectra_string(&header.value))
}

pub extern "C" fn cookie(ctx: *mut SpectraHostCallContext) -> i32 {
    let Ok(args) = read_args(ctx, 2) else {
        return HOST_STATUS_INVALID_ARGUMENT;
    };
    let (Some(name), Some(value)) = (read_spectra_string(args[0]), read_spectra_string(args[1]))
    else {
        return HOST_STATUS_INVALID_ARGUMENT;
    };
    let Ok(cookie) = Cookie::new(name, value) else {
        return HOST_STATUS_INVALID_ARGUMENT;
    };
    let mut store = store().lock().unwrap_or_else(|e| e.into_inner());
    write_result(ctx, store.cookie_handle(cookie))
}

pub extern "C" fn cookie_name(ctx: *mut SpectraHostCallContext) -> i32 {
    let Ok(args) = read_args(ctx, 1) else {
        return HOST_STATUS_INVALID_ARGUMENT;
    };
    let store = store().lock().unwrap_or_else(|e| e.into_inner());
    let Some(cookie) = store.cookies.get(&args[0]) else {
        return HOST_STATUS_INVALID_ARGUMENT;
    };
    write_result(ctx, alloc_spectra_string(&cookie.name))
}

pub extern "C" fn cookie_value(ctx: *mut SpectraHostCallContext) -> i32 {
    let Ok(args) = read_args(ctx, 1) else {
        return HOST_STATUS_INVALID_ARGUMENT;
    };
    let store = store().lock().unwrap_or_else(|e| e.into_inner());
    let Some(cookie) = store.cookies.get(&args[0]) else {
        return HOST_STATUS_INVALID_ARGUMENT;
    };
    write_result(ctx, alloc_spectra_string(&cookie.value))
}

fn method_label(method: SpectraHostValue) -> &'static str {
    Method::from_code(method)
        .map(Method::as_str)
        .unwrap_or("UNKNOWN")
}

fn status_reason_phrase(status: SpectraHostValue) -> &'static str {
    match status {
        100 => "Continue",
        101 => "Switching Protocols",
        200 => "OK",
        201 => "Created",
        202 => "Accepted",
        204 => "No Content",
        301 => "Moved Permanently",
        302 => "Found",
        304 => "Not Modified",
        400 => "Bad Request",
        401 => "Unauthorized",
        403 => "Forbidden",
        404 => "Not Found",
        405 => "Method Not Allowed",
        409 => "Conflict",
        415 => "Unsupported Media Type",
        422 => "Unprocessable Content",
        429 => "Too Many Requests",
        500 => "Internal Server Error",
        502 => "Bad Gateway",
        503 => "Service Unavailable",
        504 => "Gateway Timeout",
        _ => "Unknown Status",
    }
}

fn is_valid_header_name(name: &str) -> bool {
    !name.is_empty()
        && name.bytes().all(|b| {
            b.is_ascii_alphanumeric()
                || matches!(
                    b,
                    b'!' | b'#'
                        | b'$'
                        | b'%'
                        | b'&'
                        | b'\''
                        | b'*'
                        | b'+'
                        | b'-'
                        | b'.'
                        | b'^'
                        | b'_'
                        | b'`'
                        | b'|'
                        | b'~'
                )
        })
}

fn is_valid_header_value(value: &str) -> bool {
    value
        .bytes()
        .all(|b| b == b'\t' || b == b' ' || (0x21..=0x7e).contains(&b) || b >= 0x80)
}

fn is_valid_cookie_name(name: &str) -> bool {
    is_valid_header_name(name)
}

fn is_valid_cookie_value(value: &str) -> bool {
    !value.is_empty()
        && value
            .bytes()
            .all(|b| b != b';' && b != b',' && b != b'\\' && b != b'"' && b > 0x20 && b < 0x7f)
}

fn is_valid_request_path(path: &str) -> bool {
    !path.is_empty() && path.starts_with('/') && !path.bytes().any(|b| b <= b' ' || b == 0x7f)
}

fn cookie_value_from_header(header: &str, name: &str) -> Option<String> {
    for pair in header.split(';') {
        let pair = pair.trim();
        let Some((candidate, value)) = pair.split_once('=') else {
            continue;
        };
        if candidate.trim().eq_ignore_ascii_case(name) {
            return Some(value.trim().to_string());
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_and_status_enumerate_documented_http_values() {
        let methods = [
            (METHOD_GET, "GET", true, false),
            (METHOD_HEAD, "HEAD", true, false),
            (METHOD_POST, "POST", false, true),
            (METHOD_PUT, "PUT", false, true),
            (METHOD_PATCH, "PATCH", false, true),
            (METHOD_DELETE, "DELETE", false, false),
            (METHOD_OPTIONS, "OPTIONS", true, false),
        ];
        for (code, name, safe, body) in methods {
            let method = Method::from_code(code).expect("documented method");
            assert_eq!(method.code(), code);
            assert_eq!(method.as_str(), name);
            assert_eq!(method.is_safe(), safe);
            assert_eq!(method.allows_body(), body);
        }
        assert!(Method::from_code(99).is_none());

        for (code, reason, class, success) in [
            (100, "Continue", 1, false),
            (200, "OK", 2, true),
            (201, "Created", 2, true),
            (204, "No Content", 2, true),
            (301, "Moved Permanently", 3, false),
            (400, "Bad Request", 4, false),
            (404, "Not Found", 4, false),
            (409, "Conflict", 4, false),
            (422, "Unprocessable Content", 4, false),
            (429, "Too Many Requests", 4, false),
            (500, "Internal Server Error", 5, false),
            (503, "Service Unavailable", 5, false),
        ] {
            let status = Status::new(code).expect("documented status");
            assert_eq!(status.reason(), reason);
            assert_eq!(status.class(), class);
            assert_eq!(status.is_success(), success);
        }
        assert!(Status::new(99).is_err());
        assert!(Status::new(600).is_err());
    }

    #[test]
    fn headers_and_cookies_are_case_insensitive_and_validate_input() {
        let mut headers = Headers::new();
        headers
            .insert("Content-Type", "application/json")
            .expect("valid header");
        assert_eq!(headers.get("content-type"), Some("application/json"));
        assert!(headers.contains("CONTENT-TYPE"));
        headers
            .insert("content-type", "text/plain")
            .expect("case-insensitive upsert");
        assert_eq!(headers.len(), 1);
        assert_eq!(headers.get("CONTENT-TYPE"), Some("text/plain"));
        assert!(headers.insert("Bad Header", "value").is_err());
        assert!(headers.insert("X-Good", "bad\rvalue").is_err());

        let cookie = Cookie::new("Session", "abc123").expect("valid cookie");
        assert_eq!(cookie.name, "Session");
        assert!(Cookie::new("bad name", "abc").is_err());
        assert!(Cookie::new("session", "bad;value").is_err());
        assert_eq!(
            cookie_value_from_header("SESSION=abc123; theme=dark", "session"),
            Some("abc123".to_string())
        );
    }

    #[test]
    fn request_response_types_cover_crud_style_flow() {
        let create = Request::new(Method::Post, "/users")
            .expect("request")
            .with_header("Content-Type", "application/json")
            .expect("header")
            .with_header("Cookie", "SESSION=abc123; theme=dark")
            .expect("cookie header")
            .with_body(br#"{"name":"Ada"}"#.to_vec());
        assert_eq!(create.method, Method::Post);
        assert_eq!(create.path, "/users");
        assert_eq!(create.header("content-type"), Some("application/json"));
        assert_eq!(create.cookie("session").as_deref(), Some("abc123"));
        assert_eq!(create.body, br#"{"name":"Ada"}"#);

        let read = Request::new(Method::Get, "/users/7").expect("read request");
        let update = Request::new(Method::Put, "/users/7").expect("update request");
        let delete = Request::new(Method::Delete, "/users/7").expect("delete request");
        assert!(read.method.is_safe());
        assert!(update.method.allows_body());
        assert!(!delete.method.allows_body());

        let response = Response::new(Status::new(201).expect("created"))
            .with_header("Location", "/users/7")
            .expect("location")
            .with_body(br#"{"id":7}"#.to_vec());
        assert_eq!(response.status.reason(), "Created");
        assert_eq!(response.header("location"), Some("/users/7"));
        assert_eq!(response.body.len(), 8);
    }

    #[test]
    fn request_parser_streams_headers_then_body() {
        let mut parser = Http1Parser::request();
        parser.push(b"POST /submit HTTP/1.1\r\nHost: example.com\r\nContent-Length: 11\r\n");
        assert!(parser.parse_next_request().unwrap().is_none());
        parser.push(b"\r\nhello ");
        assert!(parser.parse_next_request().unwrap().is_none());
        parser.push(b"world");

        let request = parser
            .parse_next_request()
            .unwrap()
            .expect("complete request");
        assert_eq!(request.method, "POST");
        assert_eq!(request.target, "/submit");
        assert_eq!(request.version, HttpVersion::HTTP_11);
        assert_eq!(request.body.bytes(), b"hello world");
        assert!(request.keep_alive);
        assert_eq!(parser.buffered_len(), 0);
    }

    #[test]
    fn parser_keeps_pipelined_request_bytes_for_next_message() {
        let mut parser = Http1Parser::request();
        parser.push(
            b"GET /one HTTP/1.1\r\nHost: example.com\r\n\r\nGET /two HTTP/1.1\r\nHost: example.com\r\nConnection: close\r\n\r\n",
        );

        let first = parser.parse_next_request().unwrap().expect("first request");
        assert_eq!(first.target, "/one");
        assert!(first.keep_alive);

        let second = parser
            .parse_next_request()
            .unwrap()
            .expect("second request");
        assert_eq!(second.target, "/two");
        assert!(!second.keep_alive);
        assert_eq!(parser.buffered_len(), 0);
    }

    #[test]
    fn response_parser_accepts_rfc_7230_style_sample() {
        let response = parse_response(
            b"HTTP/1.1 200 OK\r\nDate: Sun, 06 Nov 1994 08:49:37 GMT\r\nContent-Length: 5\r\nConnection: keep-alive\r\n\r\nhello",
        )
        .expect("valid response");

        assert_eq!(response.version, HttpVersion::HTTP_11);
        assert_eq!(response.status_code, 200);
        assert_eq!(response.reason, "OK");
        assert_eq!(response.body.bytes(), b"hello");
        assert!(response.keep_alive);
    }

    #[test]
    fn chunked_request_round_trips_with_extensions_and_trailers() {
        let raw = b"POST /upload HTTP/1.1\r\nHost: example.com\r\nTransfer-Encoding: chunked\r\n\r\n4;sig=a\r\nWiki\r\n5\r\npedia\r\n0\r\nDigest: sha-256=abc\r\n\r\n";
        let request = parse_request(raw).expect("chunked request");

        assert!(request.body.chunked);
        assert_eq!(request.body.chunks.len(), 2);
        assert_eq!(request.body.chunks[0].data, b"Wiki");
        assert_eq!(request.body.chunks[0].extension.as_deref(), Some("sig=a"));
        assert_eq!(request.body.bytes(), b"Wikipedia");
        assert_eq!(
            request.body.trailers,
            vec![Header {
                name: "Digest".to_string(),
                value: "sha-256=abc".to_string()
            }]
        );
        assert_eq!(serialize_request(&request), raw);
    }

    #[test]
    fn chunked_response_round_trips_without_trailers() {
        let raw = b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n7\r\nMozilla\r\n9\r\nDeveloper\r\n0\r\n\r\n";
        let response = parse_response(raw).expect("chunked response");

        assert!(response.body.chunked);
        assert_eq!(response.body.bytes(), b"MozillaDeveloper");
        assert!(response.body.trailers.is_empty());
        assert_eq!(serialize_response(&response), raw);
    }

    #[test]
    fn http_10_keep_alive_requires_connection_header() {
        let closed = parse_request(b"GET / HTTP/1.0\r\nHost: example.com\r\n\r\n")
            .expect("HTTP/1.0 request");
        assert!(!closed.keep_alive);

        let kept =
            parse_request(b"GET / HTTP/1.0\r\nHost: example.com\r\nConnection: keep-alive\r\n\r\n")
                .expect("HTTP/1.0 keep-alive request");
        assert!(kept.keep_alive);
    }

    #[test]
    fn malformed_header_reports_typed_position() {
        let err = parse_request(b"GET / HTTP/1.1\r\nBad Header: value\r\n\r\n")
            .expect_err("invalid header name");
        assert_eq!(err.kind, ParseErrorKind::InvalidHeader);
        assert_eq!(err.position, "GET / HTTP/1.1\r\n".len());
    }

    #[test]
    fn malformed_chunk_size_reports_typed_position() {
        let err = parse_request(
            b"POST / HTTP/1.1\r\nTransfer-Encoding: chunked\r\n\r\nZ\r\nbad\r\n0\r\n\r\n",
        )
        .expect_err("invalid chunk size");
        assert_eq!(err.kind, ParseErrorKind::InvalidChunkSize);
        assert_eq!(
            err.position,
            "POST / HTTP/1.1\r\nTransfer-Encoding: chunked\r\n\r\n".len()
        );
    }

    #[test]
    fn rejects_conflicting_content_length() {
        let err =
            parse_response(b"HTTP/1.1 200 OK\r\nContent-Length: 1\r\nContent-Length: 2\r\n\r\nx")
                .expect_err("conflicting content length");
        assert_eq!(err.kind, ParseErrorKind::BodyLengthMismatch);
    }

    #[test]
    fn rejects_unsupported_transfer_encoding() {
        let err = parse_response(b"HTTP/1.1 200 OK\r\nTransfer-Encoding: gzip\r\n\r\n")
            .expect_err("unsupported transfer encoding");
        assert_eq!(err.kind, ParseErrorKind::UnsupportedTransferEncoding);
    }
}
