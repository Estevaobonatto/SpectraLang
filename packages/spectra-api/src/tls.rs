use crate::{read_args, write_result};
use crate::handles::ApiHandleTable;
use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer, ServerName};
use rustls::{ClientConfig, ClientConnection, RootCertStore, ServerConfig, ServerConnection};
use spectra_runtime::ffi::{
    SpectraHostCallContext, SpectraHostValue, HOST_STATUS_INVALID_ARGUMENT,
};
use spectra_runtime::handles::HandleKind;
use std::fmt;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream, ToSocketAddrs};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

pub const TLS_MODE_SERVER: SpectraHostValue = 1;
pub const TLS_MODE_CLIENT: SpectraHostValue = 2;

pub const DEFAULT_TLS_ALPN_HTTP11: &[u8] = b"http/1.1";

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TlsErrorKind {
    InvalidCertificate,
    InvalidPrivateKey,
    InvalidServerName,
    CertificateValidation,
    Handshake,
    Io,
    Protocol,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TlsError {
    pub kind: TlsErrorKind,
    pub message: String,
    pub cause: Option<String>,
}

impl TlsError {
    fn new(kind: TlsErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
            cause: None,
        }
    }

    fn with_cause(
        kind: TlsErrorKind,
        message: impl Into<String>,
        cause: impl Into<String>,
    ) -> Self {
        Self {
            kind,
            message: message.into(),
            cause: Some(cause.into()),
        }
    }
}

impl fmt::Display for TlsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.cause {
            Some(cause) => write!(f, "{}: {}", self.message, cause),
            None => write!(f, "{}", self.message),
        }
    }
}

impl std::error::Error for TlsError {}

#[derive(Clone, Debug)]
pub struct TlsServerConfig {
    pub cert_chain_der: Vec<Vec<u8>>,
    pub private_key_der: Vec<u8>,
    pub alpn_protocols: Vec<Vec<u8>>,
}

impl TlsServerConfig {
    pub fn new(cert_chain_der: Vec<Vec<u8>>, private_key_der: Vec<u8>) -> Self {
        Self {
            cert_chain_der,
            private_key_der,
            alpn_protocols: default_alpn_protocols(),
        }
    }

    pub fn with_alpn_protocols(mut self, protocols: Vec<Vec<u8>>) -> Self {
        self.alpn_protocols = protocols;
        self
    }

    pub fn build(self) -> Result<Arc<ServerConfig>, TlsError> {
        server_config_from_der(
            self.cert_chain_der,
            self.private_key_der,
            self.alpn_protocols,
        )
    }
}

#[derive(Clone, Debug)]
pub struct TlsClientConfig {
    pub root_certificates_der: Vec<Vec<u8>>,
    pub use_platform_webpki_roots: bool,
    pub alpn_protocols: Vec<Vec<u8>>,
}

impl TlsClientConfig {
    pub fn with_roots(root_certificates_der: Vec<Vec<u8>>) -> Self {
        Self {
            root_certificates_der,
            use_platform_webpki_roots: false,
            alpn_protocols: default_alpn_protocols(),
        }
    }

    pub fn with_webpki_roots() -> Self {
        Self {
            root_certificates_der: Vec::new(),
            use_platform_webpki_roots: true,
            alpn_protocols: default_alpn_protocols(),
        }
    }

    pub fn with_alpn_protocols(mut self, protocols: Vec<Vec<u8>>) -> Self {
        self.alpn_protocols = protocols;
        self
    }

    pub fn build(self) -> Result<Arc<ClientConfig>, TlsError> {
        if self.use_platform_webpki_roots {
            client_config_with_webpki_roots(self.alpn_protocols)
        } else {
            client_config_with_roots(self.root_certificates_der, self.alpn_protocols)
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HttpsResponse {
    pub raw: Vec<u8>,
    pub selected_alpn: Option<Vec<u8>>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HttpsServerExchange {
    pub peer: SocketAddr,
    pub request: Vec<u8>,
    pub selected_alpn: Option<Vec<u8>>,
}

fn default_alpn_protocols() -> Vec<Vec<u8>> {
    vec![DEFAULT_TLS_ALPN_HTTP11.to_vec()]
}

pub fn server_config_from_der(
    cert_chain_der: Vec<Vec<u8>>,
    private_key_der: Vec<u8>,
    alpn_protocols: Vec<Vec<u8>>,
) -> Result<Arc<ServerConfig>, TlsError> {
    if cert_chain_der.is_empty() {
        return Err(TlsError::new(
            TlsErrorKind::InvalidCertificate,
            "TLS server certificate chain is empty",
        ));
    }
    if private_key_der.is_empty() {
        return Err(TlsError::new(
            TlsErrorKind::InvalidPrivateKey,
            "TLS server private key is empty",
        ));
    }

    let cert_chain = cert_chain_der
        .into_iter()
        .map(CertificateDer::from)
        .collect::<Vec<_>>();
    let private_key = PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(private_key_der));
    let mut config = ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(cert_chain, private_key)
        .map_err(|error| {
            TlsError::with_cause(
                TlsErrorKind::InvalidCertificate,
                "failed to build TLS server certificate configuration",
                error.to_string(),
            )
        })?;
    config.alpn_protocols = normalize_alpn_protocols(alpn_protocols);
    Ok(Arc::new(config))
}

pub fn client_config_with_roots(
    root_certificates_der: Vec<Vec<u8>>,
    alpn_protocols: Vec<Vec<u8>>,
) -> Result<Arc<ClientConfig>, TlsError> {
    if root_certificates_der.is_empty() {
        return Err(TlsError::new(
            TlsErrorKind::InvalidCertificate,
            "TLS client root certificate store is empty",
        ));
    }
    let mut roots = RootCertStore::empty();
    for root in root_certificates_der {
        roots.add(CertificateDer::from(root)).map_err(|error| {
            TlsError::with_cause(
                TlsErrorKind::InvalidCertificate,
                "failed to add TLS root certificate",
                error.to_string(),
            )
        })?;
    }
    Ok(Arc::new(build_client_config(roots, alpn_protocols)))
}

pub fn client_config_with_webpki_roots(
    alpn_protocols: Vec<Vec<u8>>,
) -> Result<Arc<ClientConfig>, TlsError> {
    let mut roots = RootCertStore::empty();
    roots.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
    Ok(Arc::new(build_client_config(roots, alpn_protocols)))
}

fn build_client_config(roots: RootCertStore, alpn_protocols: Vec<Vec<u8>>) -> ClientConfig {
    let mut config = ClientConfig::builder()
        .with_root_certificates(roots)
        .with_no_client_auth();
    config.alpn_protocols = normalize_alpn_protocols(alpn_protocols);
    config
}

fn normalize_alpn_protocols(alpn_protocols: Vec<Vec<u8>>) -> Vec<Vec<u8>> {
    if alpn_protocols.is_empty() {
        default_alpn_protocols()
    } else {
        alpn_protocols
    }
}

pub fn https_get(
    host: &str,
    port: u16,
    path: &str,
    config: Arc<ClientConfig>,
    timeout: Duration,
) -> Result<HttpsResponse, TlsError> {
    let path = if path.starts_with('/') {
        path.to_string()
    } else {
        format!("/{path}")
    };
    let request = format!(
        "GET {path} HTTP/1.1\r\nHost: {host}\r\nUser-Agent: spectra-api/0.1\r\nAccept: */*\r\nConnection: close\r\n\r\n"
    );
    https_round_trip(host, port, request.as_bytes(), config, timeout)
}

pub fn https_round_trip(
    host: &str,
    port: u16,
    request: &[u8],
    config: Arc<ClientConfig>,
    timeout: Duration,
) -> Result<HttpsResponse, TlsError> {
    let addr = (host, port)
        .to_socket_addrs()
        .map_err(|error| io_error("failed to resolve HTTPS endpoint", error))?
        .next()
        .ok_or_else(|| TlsError::new(TlsErrorKind::Io, "HTTPS endpoint did not resolve"))?;
    let tcp = TcpStream::connect_timeout(&addr, timeout)
        .map_err(|error| io_error("failed to connect HTTPS TCP socket", error))?;
    tcp.set_read_timeout(Some(timeout))
        .map_err(|error| io_error("failed to set HTTPS read timeout", error))?;
    tcp.set_write_timeout(Some(timeout))
        .map_err(|error| io_error("failed to set HTTPS write timeout", error))?;

    let server_name = ServerName::try_from(host.to_string()).map_err(|error| {
        TlsError::with_cause(
            TlsErrorKind::InvalidServerName,
            "invalid TLS SNI server name",
            error.to_string(),
        )
    })?;
    let connection = ClientConnection::new(config, server_name).map_err(map_tls_error)?;
    let mut stream = rustls::StreamOwned::new(connection, tcp);
    stream
        .write_all(request)
        .map_err(|error| io_error("failed to write HTTPS request", error))?;
    stream
        .flush()
        .map_err(|error| io_error("failed to flush HTTPS request", error))?;

    let mut raw = Vec::new();
    stream
        .read_to_end(&mut raw)
        .map_err(|error| io_error("failed to read HTTPS response", error))?;
    let selected_alpn = stream
        .conn
        .alpn_protocol()
        .map(|protocol| protocol.to_vec());
    Ok(HttpsResponse { raw, selected_alpn })
}

pub fn serve_single_https_request(
    listener: TcpListener,
    config: Arc<ServerConfig>,
    response: Vec<u8>,
    timeout: Duration,
) -> Result<HttpsServerExchange, TlsError> {
    let (tcp, peer) = listener
        .accept()
        .map_err(|error| io_error("failed to accept HTTPS connection", error))?;
    tcp.set_read_timeout(Some(timeout))
        .map_err(|error| io_error("failed to set HTTPS server read timeout", error))?;
    tcp.set_write_timeout(Some(timeout))
        .map_err(|error| io_error("failed to set HTTPS server write timeout", error))?;
    let connection = ServerConnection::new(config).map_err(map_tls_error)?;
    let mut stream = rustls::StreamOwned::new(connection, tcp);
    let mut request = Vec::new();
    let mut buf = [0_u8; 8192];
    loop {
        let read = stream
            .read(&mut buf)
            .map_err(|error| io_error("failed to read HTTPS request", error))?;
        if read == 0 {
            return Err(TlsError::new(
                TlsErrorKind::Protocol,
                "HTTPS client closed before request headers completed",
            ));
        }
        request.extend_from_slice(&buf[..read]);
        if request.windows(4).any(|window| window == b"\r\n\r\n") {
            break;
        }
    }
    stream
        .write_all(&response)
        .map_err(|error| io_error("failed to write HTTPS response", error))?;
    stream
        .flush()
        .map_err(|error| io_error("failed to flush HTTPS response", error))?;
    stream.conn.send_close_notify();
    stream
        .flush()
        .map_err(|error| io_error("failed to flush HTTPS close notify", error))?;
    let selected_alpn = stream
        .conn
        .alpn_protocol()
        .map(|protocol| protocol.to_vec());
    Ok(HttpsServerExchange {
        peer,
        request,
        selected_alpn,
    })
}

fn map_tls_error(error: rustls::Error) -> TlsError {
    let cause = error.to_string();
    let lower = cause.to_ascii_lowercase();
    let kind = if lower.contains("cert")
        || lower.contains("certificate")
        || lower.contains("webpki")
        || lower.contains("unknownissuer")
        || lower.contains("not valid")
    {
        TlsErrorKind::CertificateValidation
    } else {
        TlsErrorKind::Handshake
    };
    TlsError::with_cause(kind, "TLS handshake failed", cause)
}

fn io_error(message: &'static str, error: std::io::Error) -> TlsError {
    TlsError::with_cause(TlsErrorKind::Io, message, error.to_string())
}

struct TlsStore {
    modes: ApiHandleTable<SpectraHostValue>,
}

impl TlsStore {
    fn new() -> Self {
        Self {
            modes: ApiHandleTable::new(HandleKind::ApiTlsMode),
        }
    }
}

fn store() -> &'static Mutex<TlsStore> {
    static STORE: OnceLock<Mutex<TlsStore>> = OnceLock::new();
    STORE.get_or_init(|| Mutex::new(TlsStore::new()))
}

pub extern "C" fn tls_config_new(ctx: *mut SpectraHostCallContext) -> i32 {
    let Ok(args) = read_args(ctx, 1) else {
        return HOST_STATUS_INVALID_ARGUMENT;
    };
    if !matches!(args[0], TLS_MODE_SERVER | TLS_MODE_CLIENT) {
        return HOST_STATUS_INVALID_ARGUMENT;
    }
    let mut store = store().lock().unwrap_or_else(|e| e.into_inner());
    write_result(ctx, store.modes.insert(args[0]))
}

pub extern "C" fn tls_config_mode(ctx: *mut SpectraHostCallContext) -> i32 {
    let Ok(args) = read_args(ctx, 1) else {
        return HOST_STATUS_INVALID_ARGUMENT;
    };
    let store = store().lock().unwrap_or_else(|e| e.into_inner());
    let Some(mode) = store.modes.get(&args[0]).copied() else {
        return HOST_STATUS_INVALID_ARGUMENT;
    };
    write_result(ctx, mode)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::http::parse_response;
    use rcgen::generate_simple_self_signed;
    use std::net::TcpListener;
    use std::thread;

    struct TestCertificate {
        cert_der: Vec<u8>,
        key_der: Vec<u8>,
    }

    fn self_signed_localhost() -> TestCertificate {
        let certified =
            generate_simple_self_signed(vec!["localhost".to_string(), "127.0.0.1".to_string()])
                .expect("self-signed certificate");
        TestCertificate {
            cert_der: certified.cert.der().to_vec(),
            key_der: certified.key_pair.serialize_der(),
        }
    }

    #[test]
    fn self_signed_https_server_and_client_round_trip() {
        let cert = self_signed_localhost();
        let client_config = TlsClientConfig::with_roots(vec![cert.cert_der.clone()])
            .build()
            .expect("client TLS");
        let server_config = TlsServerConfig::new(vec![cert.cert_der], cert.key_der)
            .build()
            .expect("server TLS");
        let listener =
            TcpListener::bind("127.0.0.1:0").expect("bind local HTTPS integration listener");
        let addr = listener.local_addr().expect("local HTTPS addr");
        let response =
            b"HTTP/1.1 200 OK\r\nContent-Length: 6\r\nConnection: close\r\n\r\nsecure".to_vec();
        let server = thread::spawn(move || {
            serve_single_https_request(listener, server_config, response, Duration::from_secs(5))
        });

        let client = https_get(
            "127.0.0.1",
            addr.port(),
            "/secure",
            client_config,
            Duration::from_secs(5),
        )
        .expect("HTTPS client receives response");
        let response = parse_response(&client.raw).expect("HTTPS response parses");
        assert_eq!(response.status_code, 200);
        assert_eq!(response.body.bytes(), b"secure");

        let server = server
            .join()
            .expect("HTTPS server thread joins")
            .expect("server exchange");
        assert!(String::from_utf8_lossy(&server.request).starts_with("GET /secure HTTP/1.1"));
        assert_eq!(
            client.selected_alpn.as_deref(),
            Some(DEFAULT_TLS_ALPN_HTTP11)
        );
        assert_eq!(
            server.selected_alpn.as_deref(),
            Some(DEFAULT_TLS_ALPN_HTTP11)
        );
    }

    #[test]
    fn alpn_defaults_to_http11_on_server_and_client_configs() {
        let cert = self_signed_localhost();
        let client_config = TlsClientConfig::with_roots(vec![cert.cert_der.clone()])
            .build()
            .expect("client TLS");
        let server_config = TlsServerConfig::new(vec![cert.cert_der], cert.key_der)
            .build()
            .expect("server TLS");
        assert_eq!(
            client_config.alpn_protocols,
            vec![DEFAULT_TLS_ALPN_HTTP11.to_vec()]
        );
        assert_eq!(
            server_config.alpn_protocols,
            vec![DEFAULT_TLS_ALPN_HTTP11.to_vec()]
        );
    }

    #[test]
    fn handshake_failures_are_typed_and_keep_cause() {
        let cert = self_signed_localhost();
        let wrong_root = self_signed_localhost();
        let client_config = TlsClientConfig::with_roots(vec![wrong_root.cert_der])
            .build()
            .expect("wrong-root client TLS");
        let server_config = TlsServerConfig::new(vec![cert.cert_der], cert.key_der)
            .build()
            .expect("server TLS");
        let listener =
            TcpListener::bind("127.0.0.1:0").expect("bind local HTTPS integration listener");
        let addr = listener.local_addr().expect("local HTTPS addr");
        let server = thread::spawn(move || {
            let response =
                b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".to_vec();
            serve_single_https_request(listener, server_config, response, Duration::from_secs(5))
        });

        let err = https_get(
            "127.0.0.1",
            addr.port(),
            "/secure",
            client_config,
            Duration::from_secs(5),
        )
        .expect_err("untrusted certificate must fail");
        assert!(
            matches!(
                err.kind,
                TlsErrorKind::CertificateValidation | TlsErrorKind::Handshake | TlsErrorKind::Io
            ),
            "unexpected TLS error kind: {:?}",
            err
        );
        assert!(err.cause.as_deref().unwrap_or_default().len() > 4);
        let _ = server.join().expect("HTTPS server thread joins");
    }

    #[test]
    #[ignore = "requires outbound network and public WebPKI validation"]
    fn known_external_endpoint_validates_chain() {
        let host = std::env::var("SPECTRA_TLS_EXTERNAL_HOST")
            .unwrap_or_else(|_| "example.com".to_string());
        let port = std::env::var("SPECTRA_TLS_EXTERNAL_PORT")
            .ok()
            .and_then(|value| value.parse().ok())
            .unwrap_or(443);
        let path = std::env::var("SPECTRA_TLS_EXTERNAL_PATH")
            .unwrap_or_else(|_| "/".to_string());
        let client_config = TlsClientConfig::with_webpki_roots()
            .build()
            .expect("webpki client TLS");
        let response = https_get(
            &host,
            port,
            &path,
            client_config,
            Duration::from_secs(10),
        )
        .expect("configured external HTTPS chain validates");
        let parsed = parse_response(&response.raw).expect("example.com response parses");
        assert!(
            (200..400).contains(&parsed.status_code),
            "{:?}",
            parsed.status_code
        );
        assert_eq!(
            response.selected_alpn.as_deref(),
            Some(DEFAULT_TLS_ALPN_HTTP11)
        );
    }

    #[test]
    fn local_client_rejects_untrusted_self_signed_chain() {
        let cert = self_signed_localhost();
        let wrong_root = self_signed_localhost();
        let client_config = TlsClientConfig::with_roots(vec![wrong_root.cert_der])
            .build()
            .expect("wrong-root client TLS");
        let server_config = TlsServerConfig::new(vec![cert.cert_der], cert.key_der)
            .build()
            .expect("server TLS");
        let listener =
            TcpListener::bind("127.0.0.1:0").expect("bind local HTTPS integration listener");
        let addr = listener.local_addr().expect("local HTTPS addr");
        let server = thread::spawn(move || {
            let response =
                b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".to_vec();
            serve_single_https_request(listener, server_config, response, Duration::from_secs(5))
        });

        let err = https_get(
            "127.0.0.1",
            addr.port(),
            "/",
            client_config,
            Duration::from_secs(5),
        )
        .expect_err("untrusted self-signed certificate");
        assert!(matches!(
            err.kind,
            TlsErrorKind::CertificateValidation | TlsErrorKind::Handshake | TlsErrorKind::Io
        ));
        let _ = server.join().expect("HTTPS server thread joins");
    }
}
