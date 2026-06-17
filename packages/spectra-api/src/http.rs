use crate::{alloc_spectra_string, read_args, read_spectra_string, write_result};
use spectra_runtime::ffi::{
    SpectraHostCallContext, SpectraHostValue, HOST_STATUS_INVALID_ARGUMENT,
};
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

pub const METHOD_GET: SpectraHostValue = 1;
pub const METHOD_HEAD: SpectraHostValue = 2;
pub const METHOD_POST: SpectraHostValue = 3;
pub const METHOD_PUT: SpectraHostValue = 4;
pub const METHOD_PATCH: SpectraHostValue = 5;
pub const METHOD_DELETE: SpectraHostValue = 6;
pub const METHOD_OPTIONS: SpectraHostValue = 7;

#[derive(Clone, Copy)]
struct Request {
    method: SpectraHostValue,
}

#[derive(Clone, Copy)]
struct Response {
    status: SpectraHostValue,
}

struct HttpStore {
    next_request: SpectraHostValue,
    next_response: SpectraHostValue,
    requests: HashMap<SpectraHostValue, Request>,
    responses: HashMap<SpectraHostValue, Response>,
}

impl HttpStore {
    fn new() -> Self {
        Self {
            next_request: 1,
            next_response: 1,
            requests: HashMap::new(),
            responses: HashMap::new(),
        }
    }

    fn request_handle(&mut self, method: SpectraHostValue) -> SpectraHostValue {
        let handle = self.next_request;
        self.next_request = self.next_request.saturating_add(1).max(1);
        self.requests.insert(handle, Request { method });
        handle
    }

    fn response_handle(&mut self, status: SpectraHostValue) -> SpectraHostValue {
        let handle = self.next_response;
        self.next_response = self.next_response.saturating_add(1).max(1);
        self.responses.insert(handle, Response { status });
        handle
    }
}

fn store() -> &'static Mutex<HttpStore> {
    static STORE: OnceLock<Mutex<HttpStore>> = OnceLock::new();
    STORE.get_or_init(|| Mutex::new(HttpStore::new()))
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
    if !is_known_method(args[0]) {
        return HOST_STATUS_INVALID_ARGUMENT;
    }
    let mut store = store().lock().unwrap_or_else(|e| e.into_inner());
    write_result(ctx, store.request_handle(args[0]))
}

pub extern "C" fn request_method(ctx: *mut SpectraHostCallContext) -> i32 {
    let Ok(args) = read_args(ctx, 1) else {
        return HOST_STATUS_INVALID_ARGUMENT;
    };
    let store = store().lock().unwrap_or_else(|e| e.into_inner());
    let Some(request) = store.requests.get(&args[0]) else {
        return HOST_STATUS_INVALID_ARGUMENT;
    };
    write_result(ctx, request.method)
}

pub extern "C" fn response_new(ctx: *mut SpectraHostCallContext) -> i32 {
    let Ok(args) = read_args(ctx, 1) else {
        return HOST_STATUS_INVALID_ARGUMENT;
    };
    if !(100..=599).contains(&args[0]) {
        return HOST_STATUS_INVALID_ARGUMENT;
    }
    let mut store = store().lock().unwrap_or_else(|e| e.into_inner());
    write_result(ctx, store.response_handle(args[0]))
}

pub extern "C" fn response_status(ctx: *mut SpectraHostCallContext) -> i32 {
    let Ok(args) = read_args(ctx, 1) else {
        return HOST_STATUS_INVALID_ARGUMENT;
    };
    let store = store().lock().unwrap_or_else(|e| e.into_inner());
    let Some(response) = store.responses.get(&args[0]) else {
        return HOST_STATUS_INVALID_ARGUMENT;
    };
    write_result(ctx, response.status)
}

fn is_known_method(method: SpectraHostValue) -> bool {
    matches!(
        method,
        METHOD_GET
            | METHOD_HEAD
            | METHOD_POST
            | METHOD_PUT
            | METHOD_PATCH
            | METHOD_DELETE
            | METHOD_OPTIONS
    )
}

fn method_label(method: SpectraHostValue) -> &'static str {
    match method {
        METHOD_GET => "GET",
        METHOD_HEAD => "HEAD",
        METHOD_POST => "POST",
        METHOD_PUT => "PUT",
        METHOD_PATCH => "PATCH",
        METHOD_DELETE => "DELETE",
        METHOD_OPTIONS => "OPTIONS",
        _ => "UNKNOWN",
    }
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
                || matches!(b, b'!' | b'#' | b'$' | b'%' | b'&' | b'\'' | b'*' | b'+' | b'-' | b'.' | b'^' | b'_' | b'`' | b'|' | b'~')
        })
}

fn is_valid_header_value(value: &str) -> bool {
    value
        .bytes()
        .all(|b| b == b'\t' || b == b' ' || (0x21..=0x7e).contains(&b) || b >= 0x80)
}
