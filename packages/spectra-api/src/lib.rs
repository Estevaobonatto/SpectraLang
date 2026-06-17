//! Native implementation boundary for the `spectra.api` package.
//!
//! This crate owns the Phase 22 API host-call namespace. Later Phase 22 items
//! add protocol-complete HTTP parsing, servers, clients, routing, JSON, and
//! TLS on top of this registration layer.

use spectra_runtime::ffi::{
    register_host_function, HostFunction, SpectraHostCallContext, SpectraHostValue,
    HOST_STATUS_INVALID_ARGUMENT, HOST_STATUS_SUCCESS,
};

pub mod client;
pub mod errors;
pub mod http;
pub mod json;
pub mod routing;
pub mod server;
pub mod tls;

pub const HOST_PREFIX: &str = "spectra.api.";
pub const VERSION_MAJOR: SpectraHostValue = 0;
pub const VERSION_MINOR: SpectraHostValue = 1;
pub const VERSION_PATCH: SpectraHostValue = 0;

#[derive(Clone, Copy, Debug)]
pub struct HostCallSpec {
    pub name: &'static str,
    pub function: HostFunction,
}

pub const HOST_CALLS: &[HostCallSpec] = &[
    HostCallSpec {
        name: "spectra.api.version.major",
        function: api_version_major,
    },
    HostCallSpec {
        name: "spectra.api.version.minor",
        function: api_version_minor,
    },
    HostCallSpec {
        name: "spectra.api.version.patch",
        function: api_version_patch,
    },
    HostCallSpec {
        name: "spectra.api.http.method_name",
        function: http::method_name,
    },
    HostCallSpec {
        name: "spectra.api.http.method_allows_body",
        function: http::method_allows_body,
    },
    HostCallSpec {
        name: "spectra.api.http.method_is_safe",
        function: http::method_is_safe,
    },
    HostCallSpec {
        name: "spectra.api.http.status_reason",
        function: http::status_reason,
    },
    HostCallSpec {
        name: "spectra.api.http.status_class",
        function: http::status_class,
    },
    HostCallSpec {
        name: "spectra.api.http.status_is_success",
        function: http::status_is_success,
    },
    HostCallSpec {
        name: "spectra.api.http.header_name_is_valid",
        function: http::header_name_is_valid,
    },
    HostCallSpec {
        name: "spectra.api.http.header_value_is_valid",
        function: http::header_value_is_valid,
    },
    HostCallSpec {
        name: "spectra.api.http.request_new",
        function: http::request_new,
    },
    HostCallSpec {
        name: "spectra.api.http.request_method",
        function: http::request_method,
    },
    HostCallSpec {
        name: "spectra.api.http.response_new",
        function: http::response_new,
    },
    HostCallSpec {
        name: "spectra.api.http.response_status",
        function: http::response_status,
    },
    HostCallSpec {
        name: "spectra.api.server.new",
        function: server::server_new,
    },
    HostCallSpec {
        name: "spectra.api.server.state",
        function: server::server_state,
    },
    HostCallSpec {
        name: "spectra.api.server.shutdown",
        function: server::server_shutdown,
    },
    HostCallSpec {
        name: "spectra.api.client.new",
        function: client::client_new,
    },
    HostCallSpec {
        name: "spectra.api.client.timeout_ms",
        function: client::client_timeout_ms,
    },
    HostCallSpec {
        name: "spectra.api.json.validate",
        function: json::json_validate,
    },
    HostCallSpec {
        name: "spectra.api.json.kind",
        function: json::json_kind,
    },
    HostCallSpec {
        name: "spectra.api.tls.config_new",
        function: tls::tls_config_new,
    },
    HostCallSpec {
        name: "spectra.api.tls.config_mode",
        function: tls::tls_config_mode,
    },
    HostCallSpec {
        name: "spectra.api.routing.router_new",
        function: routing::router_new,
    },
    HostCallSpec {
        name: "spectra.api.routing.route_count",
        function: routing::route_count,
    },
    HostCallSpec {
        name: "spectra.api.errors.last_code",
        function: errors::last_code,
    },
    HostCallSpec {
        name: "spectra.api.errors.last_message",
        function: errors::last_message,
    },
];

pub fn register() -> usize {
    spectra_runtime::initialize();
    let mut inserted = 0;
    for spec in HOST_CALLS {
        if register_host_function(spec.name, spec.function) {
            inserted += 1;
        }
    }
    inserted
}

#[no_mangle]
pub extern "C" fn spectra_api_register_host_calls() -> usize {
    register()
}

#[no_mangle]
pub extern "C" fn spectra_api_host_call_count() -> usize {
    HOST_CALLS.len()
}

extern "C" fn api_version_major(ctx: *mut SpectraHostCallContext) -> i32 {
    write_result(ctx, VERSION_MAJOR)
}

extern "C" fn api_version_minor(ctx: *mut SpectraHostCallContext) -> i32 {
    write_result(ctx, VERSION_MINOR)
}

extern "C" fn api_version_patch(ctx: *mut SpectraHostCallContext) -> i32 {
    write_result(ctx, VERSION_PATCH)
}

pub(crate) fn read_args<'a>(
    ctx: *mut SpectraHostCallContext,
    expected: usize,
) -> Result<&'a [SpectraHostValue], i32> {
    if ctx.is_null() {
        return Err(HOST_STATUS_INVALID_ARGUMENT);
    }
    let ctx_ref = unsafe { &*ctx };
    if ctx_ref.arg_len < expected || (ctx_ref.arg_len > 0 && ctx_ref.args.is_null()) {
        return Err(HOST_STATUS_INVALID_ARGUMENT);
    }
    let args = if ctx_ref.arg_len == 0 {
        &[]
    } else {
        unsafe { std::slice::from_raw_parts(ctx_ref.args, ctx_ref.arg_len) }
    };
    Ok(args)
}

pub(crate) fn write_result(ctx: *mut SpectraHostCallContext, value: SpectraHostValue) -> i32 {
    if ctx.is_null() {
        return HOST_STATUS_INVALID_ARGUMENT;
    }
    let ctx_ref = unsafe { &mut *ctx };
    if ctx_ref.result_len == 0 || ctx_ref.results.is_null() {
        return HOST_STATUS_INVALID_ARGUMENT;
    }
    let results = unsafe { std::slice::from_raw_parts_mut(ctx_ref.results, ctx_ref.result_len) };
    results[0] = value;
    HOST_STATUS_SUCCESS
}

pub(crate) fn read_spectra_string(ptr_value: SpectraHostValue) -> Option<String> {
    if ptr_value == 0 {
        return None;
    }
    let ptr = ptr_value as *const i64;
    if ptr.is_null() {
        return None;
    }
    let mut bytes = Vec::new();
    unsafe {
        let mut offset = 0usize;
        loop {
            let value = *ptr.add(offset);
            if value == 0 {
                break;
            }
            if !(0..=255).contains(&value) {
                return None;
            }
            bytes.push(value as u8);
            offset += 1;
            if offset > 1_048_576 {
                return None;
            }
        }
    }
    String::from_utf8(bytes).ok()
}

pub(crate) fn alloc_spectra_string(value: &str) -> SpectraHostValue {
    let len = value.len() + 1;
    let bytes = len * std::mem::size_of::<i64>();
    let ptr = spectra_runtime::ffi::spectra_rt_manual_alloc(bytes) as *mut i64;
    if ptr.is_null() {
        return 0;
    }
    unsafe {
        for (idx, byte) in value.bytes().enumerate() {
            *ptr.add(idx) = byte as i64;
        }
        *ptr.add(value.len()) = 0;
    }
    ptr as SpectraHostValue
}

#[cfg(test)]
mod tests {
    use super::*;
    use spectra_runtime::ffi::{
        clear_host_functions, lookup_host_function, spectra_rt_manual_clear,
        SpectraHostCallContext, HOST_STATUS_NOT_FOUND,
    };
    use std::collections::HashSet;
    use std::sync::{Mutex, MutexGuard, OnceLock};

    fn test_guard() -> MutexGuard<'static, ()> {
        static GUARD: OnceLock<Mutex<()>> = OnceLock::new();
        GUARD
            .get_or_init(|| Mutex::new(()))
            .lock()
            .expect("spectra-api test guard poisoned")
    }

    fn call(name: &str, args: &[SpectraHostValue]) -> (i32, SpectraHostValue) {
        let func = lookup_host_function(name).expect("host function registered");
        let mut result = [0_i64];
        let mut ctx = SpectraHostCallContext {
            args: args.as_ptr(),
            arg_len: args.len(),
            results: result.as_mut_ptr(),
            result_len: result.len(),
            invoke_fn: None,
        };
        let status = func(&mut ctx as *mut _);
        (status, result[0])
    }

    #[test]
    fn host_call_table_is_unique_and_prefixed() {
        let mut names = HashSet::new();
        for spec in HOST_CALLS {
            assert!(spec.name.starts_with(HOST_PREFIX), "{}", spec.name);
            assert!(names.insert(spec.name), "duplicate {}", spec.name);
        }
        assert_eq!(HOST_CALLS.len(), 28);
    }

    #[test]
    fn register_adds_all_api_host_calls_to_runtime_registry() {
        let _guard = test_guard();
        clear_host_functions();
        let inserted = register();
        assert_eq!(inserted, HOST_CALLS.len());
        for spec in HOST_CALLS {
            assert!(lookup_host_function(spec.name).is_some(), "{}", spec.name);
        }
        let second = register();
        assert_eq!(second, 0);
        clear_host_functions();
    }

    #[test]
    fn registered_version_and_http_functions_execute() {
        let _guard = test_guard();
        clear_host_functions();
        register();
        assert_eq!(call("spectra.api.version.major", &[]), (HOST_STATUS_SUCCESS, 0));
        assert_eq!(
            call("spectra.api.http.method_allows_body", &[http::METHOD_POST]),
            (HOST_STATUS_SUCCESS, 1)
        );
        assert_eq!(
            call("spectra.api.http.status_is_success", &[201]),
            (HOST_STATUS_SUCCESS, 1)
        );
        assert_eq!(
            call("spectra.api.http.status_class", &[404]),
            (HOST_STATUS_SUCCESS, 4)
        );
        assert_eq!(
            call("spectra.api.errors.last_code", &[]),
            (HOST_STATUS_SUCCESS, errors::ERROR_NONE)
        );
        clear_host_functions();
    }

    #[test]
    fn registered_handle_functions_keep_state() {
        let _guard = test_guard();
        clear_host_functions();
        register();
        let (_, req) = call("spectra.api.http.request_new", &[http::METHOD_GET]);
        assert!(req > 0);
        assert_eq!(
            call("spectra.api.http.request_method", &[req]),
            (HOST_STATUS_SUCCESS, http::METHOD_GET)
        );
        let (_, resp) = call("spectra.api.http.response_new", &[204]);
        assert!(resp > 0);
        assert_eq!(
            call("spectra.api.http.response_status", &[resp]),
            (HOST_STATUS_SUCCESS, 204)
        );
        let (_, server) = call("spectra.api.server.new", &[]);
        assert!(server > 0);
        assert_eq!(
            call("spectra.api.server.shutdown", &[server]),
            (HOST_STATUS_SUCCESS, 1)
        );
        assert_eq!(
            call("spectra.api.server.state", &[server]),
            (HOST_STATUS_SUCCESS, server::SERVER_STATE_STOPPED)
        );
        clear_host_functions();
    }

    #[test]
    fn string_based_registered_functions_accept_runtime_strings() {
        let _guard = test_guard();
        clear_host_functions();
        register();
        let name = alloc_spectra_string("Content-Type");
        let value = alloc_spectra_string("application/json");
        assert_eq!(
            call("spectra.api.http.header_name_is_valid", &[name]),
            (HOST_STATUS_SUCCESS, 1)
        );
        assert_eq!(
            call("spectra.api.http.header_value_is_valid", &[value]),
            (HOST_STATUS_SUCCESS, 1)
        );
        let invalid = alloc_spectra_string("bad header");
        assert_eq!(
            call("spectra.api.http.header_name_is_valid", &[invalid]),
            (HOST_STATUS_SUCCESS, 0)
        );
        spectra_rt_manual_clear();
        clear_host_functions();
    }

    #[test]
    fn missing_host_call_still_reports_runtime_not_found() {
        let _guard = test_guard();
        clear_host_functions();
        let mut out = [0_i64];
        let name = "spectra.api.http.missing";
        let status = spectra_runtime::ffi::spectra_rt_host_invoke(
            name.as_ptr(),
            name.len(),
            std::ptr::null(),
            0,
            out.as_mut_ptr(),
            1,
        );
        assert_eq!(status, HOST_STATUS_NOT_FOUND);
    }
}
