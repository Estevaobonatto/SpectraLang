//! Runtime tracing with W3C Trace Context and OTLP/HTTP export.
//!
//! The implementation deliberately keeps the public surface small: handles
//! are opaque integers, while ownership, parentage and export state stay in
//! the runtime. No sidecar or test-only collector is involved in production
//! export.

use std::cell::RefCell;
use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, SystemTime, UNIX_EPOCH, Instant};

const MAX_NAME: usize = 256;
const MAX_ATTRIBUTES: usize = 64;
const MAX_VALUE: usize = 4096;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SpanKind { Internal = 1, Server = 2, Client = 3, Producer = 4, Consumer = 5 }

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SpanStatus { Unset, Ok, Error }

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TraceContext { pub trace_id: [u8; 16], pub span_id: [u8; 8], pub flags: u8 }

impl TraceContext {
    pub fn traceparent(&self) -> String {
        format!("00-{}-{}-{:02x}", hex(&self.trace_id), hex(&self.span_id), self.flags)
    }
    pub fn parse(value: &str) -> Result<Self, &'static str> {
        let parts: Vec<_> = value.split('-').collect();
        if parts.len() != 4 || parts[0] != "00" || parts[1].len() != 32 || parts[2].len() != 16 || parts[3].len() != 2 { return Err("E2702"); }
        let trace_id = decode_hex::<16>(parts[1]).ok_or("E2702")?;
        let span_id = decode_hex::<8>(parts[2]).ok_or("E2702")?;
        let flags = u8::from_str_radix(parts[3], 16).map_err(|_| "E2702")?;
        if trace_id.iter().all(|v| *v == 0) || span_id.iter().all(|v| *v == 0) { return Err("E2702"); }
        Ok(Self { trace_id, span_id, flags })
    }
}

#[derive(Clone, Debug)]
pub struct TraceConfig { pub endpoint: String, pub service_name: String, pub sample_rate: f64, pub batch_size: usize, pub queue_capacity: usize, pub shutdown_timeout: Duration }

#[derive(Clone, Debug)]
struct Span { context: TraceContext, parent: Option<TraceContext>, name: String, kind: SpanKind, attributes: Vec<(String, String)>, status: SpanStatus, started: Instant, start_unix_nanos: u64, ended: bool }

#[derive(Default)]
struct State { configs: HashMap<u64, TraceConfig>, spans: HashMap<u64, Span>, completed: Vec<Span>, last_error: Option<String>, active_config: Option<u64>, exported: u64, dropped: u64 }

fn state() -> &'static Mutex<State> { static STATE: OnceLock<Mutex<State>> = OnceLock::new(); STATE.get_or_init(|| Mutex::new(State::default())) }
fn next_id() -> u64 { static COUNTER: AtomicU64 = AtomicU64::new(1); COUNTER.fetch_add(1, Ordering::Relaxed) }
fn new_context(parent: Option<&TraceContext>) -> TraceContext {
    let n = next_id();
    let mut trace_id = [0u8; 16]; let mut span_id = [0u8; 8];
    if let Some(parent) = parent { trace_id = parent.trace_id; }
    else if getrandom::fill(&mut trace_id).is_err() { trace_id[..8].copy_from_slice(&n.to_le_bytes()); trace_id[8..].copy_from_slice(&unix_nanos().to_le_bytes()); }
    if getrandom::fill(&mut span_id).is_err() { span_id.copy_from_slice(&n.to_le_bytes()); }
    TraceContext { trace_id, span_id, flags: 1 }
}

thread_local! {
    static CONTEXT_STACK: RefCell<Vec<TraceContext>> = const { RefCell::new(Vec::new()) };
    static SPAN_STACK: RefCell<Vec<u64>> = const { RefCell::new(Vec::new()) };
}

pub fn config_new(endpoint: &str, service_name: &str) -> Result<u64, &'static str> {
    if endpoint.is_empty() || service_name.is_empty() || service_name.len() > MAX_NAME || !(endpoint.starts_with("http://") || endpoint.starts_with("https://")) { return Err("E2701"); }
    let id = next_id(); state().lock().unwrap().configs.insert(id, TraceConfig { endpoint: endpoint.to_string(), service_name: service_name.to_string(), sample_rate: 1.0, batch_size: 256, queue_capacity: 4096, shutdown_timeout: Duration::from_secs(5) }); Ok(id)
}
pub fn config_set_sample_rate(id: u64, rate: f64) -> Result<(), &'static str> { if !(0.0..=1.0).contains(&rate) { return Err("E2701"); } state().lock().unwrap().configs.get_mut(&id).map(|c| c.sample_rate = rate).ok_or("E2701") }
pub fn config_set_batch_size(id: u64, size: usize) -> Result<(), &'static str> { if size == 0 || size > 65536 { return Err("E2701"); } state().lock().unwrap().configs.get_mut(&id).map(|c| c.batch_size = size).ok_or("E2701") }
pub fn config_start(id: u64) -> Result<(), &'static str> { let mut s = state().lock().unwrap(); if !s.configs.contains_key(&id) { return Err("E2701"); } s.active_config = Some(id); Ok(()) }
pub fn config_shutdown(id: u64) -> Result<(), &'static str> {
    {
        let s = state().lock().unwrap();
        if s.active_config != Some(id) { return Err("E2701"); }
    }
    flush().map(|_| ())?;
    let mut s = state().lock().unwrap();
    if s.active_config != Some(id) { return Err("E2701"); }
    s.active_config = None;
    Ok(())
}

pub fn span_start(name: &str, kind: SpanKind) -> Result<u64, &'static str> {
    let parent = CONTEXT_STACK.with(|stack| stack.borrow().last().cloned());
    span_start_with_parent(name, kind, parent)
}
pub fn span_start_with_parent(name: &str, kind: SpanKind, explicit_parent: Option<TraceContext>) -> Result<u64, &'static str> {
    if name.is_empty() || name.len() > MAX_NAME { return Err("E2701"); }
    let mut s = state().lock().unwrap(); let config = s.active_config.and_then(|id| s.configs.get(&id));
    if config.is_none() { return Err("E2704"); }
    let parent = explicit_parent; let context = new_context(parent.as_ref()); let id = next_id();
    s.spans.insert(id, Span { context: context.clone(), parent, name: name.to_string(), kind, attributes: Vec::new(), status: SpanStatus::Unset, started: Instant::now(), start_unix_nanos: unix_nanos(), ended: false });
    SPAN_STACK.with(|stack| stack.borrow_mut().push(id));
    CONTEXT_STACK.with(|stack| stack.borrow_mut().push(context)); Ok(id)
}
pub fn span_set_attribute(id: u64, key: &str, value: &str) -> Result<(), &'static str> { if key.is_empty() || key.len() > MAX_NAME || value.len() > MAX_VALUE { return Err("E2701"); } let mut s = state().lock().unwrap(); let span = s.spans.get_mut(&id).ok_or("E2703")?; if span.ended { return Err("E2703"); } if span.attributes.len() >= MAX_ATTRIBUTES { return Err("E2701"); } span.attributes.push((key.to_string(), value.to_string())); Ok(()) }
pub fn span_set_status(id: u64, status: SpanStatus) -> Result<(), &'static str> { let mut s = state().lock().unwrap(); let span = s.spans.get_mut(&id).ok_or("E2703")?; if span.ended { return Err("E2703"); } span.status = status; Ok(()) }
pub fn span_end(id: u64) -> Result<(), &'static str> {
    let mut s = state().lock().unwrap();
    let is_current = SPAN_STACK.with(|stack| stack.borrow().last().copied() == Some(id));
    if !is_current { return Err("E2703"); }
    if s.spans.get(&id).ok_or("E2703")?.ended { return Err("E2703"); }
    let capacity = s.active_config.and_then(|config| s.configs.get(&config)).map(|config| config.queue_capacity).unwrap_or(4096);
    if s.completed.len() >= capacity {
        s.dropped += 1;
        s.last_error = Some("E2705".to_string());
        return Err("E2705");
    }
    s.spans.get_mut(&id).expect("span checked above").ended = true;
    let span = s.spans.remove(&id).unwrap();
    s.completed.push(span);
    SPAN_STACK.with(|stack| {
        let mut stack = stack.borrow_mut();
        if stack.last().copied() != Some(id) { return Err("E2703"); }
        stack.pop();
        Ok(())
    })?;
    CONTEXT_STACK.with(|stack| { stack.borrow_mut().pop(); });
    Ok(())
}
pub fn current() -> Option<u64> { SPAN_STACK.with(|stack| stack.borrow().last().copied()) }
pub fn context(id: u64) -> Result<TraceContext, &'static str> { state().lock().unwrap().spans.get(&id).map(|s| s.context.clone()).ok_or("E2703") }
pub fn inject(id: u64) -> Result<String, &'static str> { Ok(context(id)?.traceparent()) }
pub fn extract(value: &str) -> Result<TraceContext, &'static str> { TraceContext::parse(value) }
pub fn current_traceparent() -> Option<String> { current().and_then(|id| context(id).ok()).map(|ctx| ctx.traceparent()) }
pub fn parent_context(id: u64) -> Result<Option<TraceContext>, &'static str> { state().lock().unwrap().spans.get(&id).map(|span| span.parent.clone()).ok_or("E2703") }
pub fn last_error() -> Option<String> { state().lock().unwrap().last_error.clone() }
pub unsafe fn alloc_string(value: &str) -> i64 {
    use crate::ffi::spectra_rt_manual_alloc;
    let bytes = value.as_bytes();
    let total = (bytes.len() + 1) * std::mem::size_of::<i64>();
    let raw = spectra_rt_manual_alloc(total) as *mut i64;
    if raw.is_null() { return 0; }
    for (index, byte) in bytes.iter().enumerate() { *raw.add(index) = i64::from(*byte); }
    *raw.add(bytes.len()) = 0;
    raw as i64
}

pub fn flush() -> Result<usize, &'static str> {
    let (config, spans) = { let mut s = state().lock().unwrap(); let config = s.active_config.and_then(|id| s.configs.get(&id).cloned()).ok_or("E2704")?; (config, std::mem::take(&mut s.completed)) };
    if spans.is_empty() { return Ok(0); }
    let payload = encode_otlp(&config.service_name, &spans); match send_otlp(&config.endpoint, &payload) { Ok(()) => { state().lock().unwrap().exported += spans.len() as u64; Ok(spans.len()) }, Err(_) => { let mut s = state().lock().unwrap(); s.dropped += spans.len() as u64; s.last_error = Some("E2706".to_string()); Err("E2706") } }
}

fn send_otlp(endpoint: &str, body: &[u8]) -> Result<(), ()> {
    let url = endpoint.strip_prefix("http://").ok_or(())?; let (authority, path) = url.split_once('/').unwrap_or((url, "v1/traces")); let mut addrs = authority.to_socket_addrs().map_err(|_| ())?; let mut stream = TcpStream::connect_timeout(&addrs.next().ok_or(())?, Duration::from_secs(5)).map_err(|_| ())?;
    let request = format!("POST /{} HTTP/1.1\r\nHost: {}\r\nContent-Type: application/x-protobuf\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", path, authority, body.len()); stream.write_all(request.as_bytes()).map_err(|_| ())?; stream.write_all(body).map_err(|_| ())?; let mut response = Vec::new(); stream.read_to_end(&mut response).map_err(|_| ())?; if response.windows(12).any(|w| w == b" 200 ") || response.starts_with(b"HTTP/1.1 20") { Ok(()) } else { Err(()) }
}

fn encode_otlp(service: &str, spans: &[Span]) -> Vec<u8> { let mut scope_spans = Vec::new(); for span in spans { let mut out = Vec::new(); field_bytes(&mut out, 1, &span.context.trace_id); field_bytes(&mut out, 2, &span.context.span_id); if let Some(parent) = &span.parent { field_bytes(&mut out, 4, &parent.span_id); } field_string(&mut out, 5, &span.name); field_varint(&mut out, 6, span.kind as u64); field_fixed64(&mut out, 7, span.start_unix_nanos); field_fixed64(&mut out, 8, span.start_unix_nanos + span.started.elapsed().as_nanos() as u64); for (key, value) in &span.attributes { let mut kv = Vec::new(); field_string(&mut kv, 1, key); let mut any = Vec::new(); field_string(&mut any, 1, value); field_message(&mut kv, 2, &any); field_message(&mut out, 9, &kv); } let mut status = Vec::new(); field_varint(&mut status, 2, match span.status { SpanStatus::Unset => 0, SpanStatus::Ok => 1, SpanStatus::Error => 2 }); field_message(&mut out, 15, &status); field_message(&mut scope_spans, 2, &out); } let mut scope = Vec::new(); field_string(&mut scope, 1, "spectralang.runtime"); field_message(&mut scope_spans, 1, &scope); let mut resource = Vec::new(); let mut kv = Vec::new(); field_string(&mut kv, 1, "service.name"); let mut any = Vec::new(); field_string(&mut any, 1, service); field_message(&mut kv, 2, &any); field_message(&mut resource, 1, &kv); let mut resource_spans = Vec::new(); field_message(&mut resource_spans, 1, &resource); field_message(&mut resource_spans, 2, &scope_spans); let mut request = Vec::new(); field_message(&mut request, 1, &resource_spans); request }
fn field_varint(out: &mut Vec<u8>, field: u32, value: u64) { put_varint(out, ((field as u64) << 3) | 0); put_varint(out, value); }
fn field_fixed64(out: &mut Vec<u8>, field: u32, value: u64) { put_varint(out, ((field as u64) << 3) | 1); out.extend_from_slice(&value.to_le_bytes()); }
fn field_string(out: &mut Vec<u8>, field: u32, value: &str) { field_bytes(out, field, value.as_bytes()); }
fn field_bytes(out: &mut Vec<u8>, field: u32, value: &[u8]) { put_varint(out, ((field as u64) << 3) | 2); put_varint(out, value.len() as u64); out.extend_from_slice(value); }
fn field_message(out: &mut Vec<u8>, field: u32, value: &[u8]) { field_bytes(out, field, value); }
fn put_varint(out: &mut Vec<u8>, mut value: u64) { while value >= 0x80 { out.push((value as u8) | 0x80); value >>= 7; } out.push(value as u8); }
fn unix_nanos() -> u64 { SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_nanos() as u64 }
fn hex<const N: usize>(bytes: &[u8; N]) -> String { bytes.iter().map(|b| format!("{b:02x}")).collect() }
fn decode_hex<const N: usize>(value: &str) -> Option<[u8; N]> { if value.len() != N * 2 { return None; } let mut out = [0u8; N]; for i in 0..N { out[i] = u8::from_str_radix(&value[i*2..i*2+2], 16).ok()?; } Some(out) }

pub fn begin_external_span(kind: SpanKind, name: &str) -> Result<u64, &'static str> { span_start(name, kind) }
pub fn end_external_span(id: u64, success: bool) -> Result<(), &'static str> { span_set_status(id, if success { SpanStatus::Ok } else { SpanStatus::Error })?; span_end(id) }

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn traceparent_round_trip() { let ctx = TraceContext { trace_id: [1;16], span_id: [2;8], flags: 1 }; assert_eq!(TraceContext::parse(&ctx.traceparent()).unwrap(), ctx); }
    #[test] fn rejects_zero_ids() { assert!(TraceContext::parse("00-00000000000000000000000000000000-0101010101010101-01").is_err()); }
    #[test] fn protobuf_contains_export_request() { let ctx = TraceContext { trace_id: [1;16], span_id: [2;8], flags: 1 }; let span = Span { context: ctx, parent: None, name: "test".into(), kind: SpanKind::Internal, attributes: vec![("k".into(), "v".into())], status: SpanStatus::Ok, started: Instant::now(), start_unix_nanos: 1, ended: true }; let bytes = encode_otlp("test", &[span]); assert!(!bytes.is_empty()); assert!(bytes.contains(&b't')); }
}
