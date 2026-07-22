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
pub mod conformance;
pub mod cors;
pub mod db;
pub mod errors;
pub mod form;
pub mod handler;
pub mod http;
pub mod json;
pub mod middleware;
pub mod multipart;
pub mod query;
pub mod routing;
pub mod server;
pub mod tls;
pub mod trace;

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
        name: "spectra.api.http.method_get",
        function: http::method_get,
    },
    HostCallSpec {
        name: "spectra.api.http.method_head",
        function: http::method_head,
    },
    HostCallSpec {
        name: "spectra.api.http.method_post",
        function: http::method_post,
    },
    HostCallSpec {
        name: "spectra.api.http.method_put",
        function: http::method_put,
    },
    HostCallSpec {
        name: "spectra.api.http.method_patch",
        function: http::method_patch,
    },
    HostCallSpec {
        name: "spectra.api.http.method_delete",
        function: http::method_delete,
    },
    HostCallSpec {
        name: "spectra.api.http.method_options",
        function: http::method_options,
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
        name: "spectra.api.http.status",
        function: http::status,
    },
    HostCallSpec {
        name: "spectra.api.http.status_continue",
        function: http::status_continue,
    },
    HostCallSpec {
        name: "spectra.api.http.status_switching_protocols",
        function: http::status_switching_protocols,
    },
    HostCallSpec {
        name: "spectra.api.http.status_ok",
        function: http::status_ok,
    },
    HostCallSpec {
        name: "spectra.api.http.status_created",
        function: http::status_created,
    },
    HostCallSpec {
        name: "spectra.api.http.status_accepted",
        function: http::status_accepted,
    },
    HostCallSpec {
        name: "spectra.api.http.status_no_content",
        function: http::status_no_content,
    },
    HostCallSpec {
        name: "spectra.api.http.status_moved_permanently",
        function: http::status_moved_permanently,
    },
    HostCallSpec {
        name: "spectra.api.http.status_found",
        function: http::status_found,
    },
    HostCallSpec {
        name: "spectra.api.http.status_not_modified",
        function: http::status_not_modified,
    },
    HostCallSpec {
        name: "spectra.api.http.status_bad_request",
        function: http::status_bad_request,
    },
    HostCallSpec {
        name: "spectra.api.http.status_unauthorized",
        function: http::status_unauthorized,
    },
    HostCallSpec {
        name: "spectra.api.http.status_forbidden",
        function: http::status_forbidden,
    },
    HostCallSpec {
        name: "spectra.api.http.status_not_found",
        function: http::status_not_found,
    },
    HostCallSpec {
        name: "spectra.api.http.status_method_not_allowed",
        function: http::status_method_not_allowed,
    },
    HostCallSpec {
        name: "spectra.api.http.status_conflict",
        function: http::status_conflict,
    },
    HostCallSpec {
        name: "spectra.api.http.status_unsupported_media_type",
        function: http::status_unsupported_media_type,
    },
    HostCallSpec {
        name: "spectra.api.http.status_unprocessable_content",
        function: http::status_unprocessable_content,
    },
    HostCallSpec {
        name: "spectra.api.http.status_too_many_requests",
        function: http::status_too_many_requests,
    },
    HostCallSpec {
        name: "spectra.api.http.status_internal_server_error",
        function: http::status_internal_server_error,
    },
    HostCallSpec {
        name: "spectra.api.http.status_bad_gateway",
        function: http::status_bad_gateway,
    },
    HostCallSpec {
        name: "spectra.api.http.status_service_unavailable",
        function: http::status_service_unavailable,
    },
    HostCallSpec {
        name: "spectra.api.http.status_gateway_timeout",
        function: http::status_gateway_timeout,
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
        name: "spectra.api.http.request",
        function: http::request,
    },
    HostCallSpec {
        name: "spectra.api.http.request_method",
        function: http::request_method,
    },
    HostCallSpec {
        name: "spectra.api.http.request_path",
        function: http::request_path,
    },
    HostCallSpec {
        name: "spectra.api.http.request_header",
        function: http::request_header,
    },
    HostCallSpec {
        name: "spectra.api.http.request_with_header",
        function: http::request_with_header,
    },
    HostCallSpec {
        name: "spectra.api.http.request_cookie",
        function: http::request_cookie,
    },
    HostCallSpec {
        name: "spectra.api.http.response_new",
        function: http::response_new,
    },
    HostCallSpec {
        name: "spectra.api.http.response",
        function: http::response_new,
    },
    HostCallSpec {
        name: "spectra.api.http.response_status",
        function: http::response_status,
    },
    HostCallSpec {
        name: "spectra.api.http.response_header",
        function: http::response_header,
    },
    HostCallSpec {
        name: "spectra.api.http.response_body_len",
        function: http::response_body_len,
    },
    HostCallSpec {
        name: "spectra.api.http.header",
        function: http::header,
    },
    HostCallSpec {
        name: "spectra.api.http.header_name",
        function: http::header_name,
    },
    HostCallSpec {
        name: "spectra.api.http.header_value",
        function: http::header_value,
    },
    HostCallSpec {
        name: "spectra.api.http.cookie",
        function: http::cookie,
    },
    HostCallSpec {
        name: "spectra.api.http.cookie_name",
        function: http::cookie_name,
    },
    HostCallSpec {
        name: "spectra.api.http.cookie_value",
        function: http::cookie_value,
    },
    HostCallSpec {
        name: "spectra.api.server.new",
        function: server::server_new,
    },
    HostCallSpec {
        name: "spectra.api.server.listen",
        function: server::server_listen,
    },
    HostCallSpec {
        name: "spectra.api.server.serve",
        function: server::server_serve,
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
        name: "spectra.api.server.local_port",
        function: server::server_local_port,
    },
    HostCallSpec {
        name: "spectra.api.server.signal",
        function: server::server_signal,
    },
    HostCallSpec {
        name: "spectra.api.server.stats",
        function: server::server_stats,
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
        name: "spectra.api.routing.route_id",
        function: routing::route_id,
    },
    HostCallSpec {
        name: "spectra.api.routing.route_add",
        function: routing::route_add,
    },
    HostCallSpec {
        name: "spectra.api.routing.get",
        function: routing::get,
    },
    HostCallSpec {
        name: "spectra.api.routing.post",
        function: routing::post,
    },
    HostCallSpec {
        name: "spectra.api.routing.put",
        function: routing::put,
    },
    HostCallSpec {
        name: "spectra.api.routing.patch",
        function: routing::patch,
    },
    HostCallSpec {
        name: "spectra.api.routing.delete",
        function: routing::delete,
    },
    HostCallSpec {
        name: "spectra.api.routing.route_match",
        function: routing::route_match,
    },
    HostCallSpec {
        name: "spectra.api.routing.match_route_id",
        function: routing::match_route_id,
    },
    HostCallSpec {
        name: "spectra.api.routing.match_param",
        function: routing::match_param,
    },
    HostCallSpec {
        name: "spectra.api.routing.match_param_int",
        function: routing::match_param_int,
    },
    HostCallSpec {
        name: "spectra.api.routing.last_conflict",
        function: routing::last_conflict,
    },
    HostCallSpec {
        name: "spectra.api.query.type_string",
        function: query::type_string,
    },
    HostCallSpec {
        name: "spectra.api.query.type_int",
        function: query::type_int,
    },
    HostCallSpec {
        name: "spectra.api.query.type_bool",
        function: query::type_bool,
    },
    HostCallSpec {
        name: "spectra.api.query.parse",
        function: query::parse,
    },
    HostCallSpec {
        name: "spectra.api.query.len",
        function: query::len,
    },
    HostCallSpec {
        name: "spectra.api.query.has",
        function: query::has,
    },
    HostCallSpec {
        name: "spectra.api.query.count",
        function: query::count,
    },
    HostCallSpec {
        name: "spectra.api.query.first",
        function: query::first,
    },
    HostCallSpec {
        name: "spectra.api.query.value",
        function: query::value,
    },
    HostCallSpec {
        name: "spectra.api.query.int",
        function: query::int,
    },
    HostCallSpec {
        name: "spectra.api.query.bool",
        function: query::bool,
    },
    HostCallSpec {
        name: "spectra.api.query.schema",
        function: query::schema,
    },
    HostCallSpec {
        name: "spectra.api.query.schema_field",
        function: query::schema_field,
    },
    HostCallSpec {
        name: "spectra.api.query.bind",
        function: query::bind,
    },
    HostCallSpec {
        name: "spectra.api.query.binding_ok",
        function: query::binding_ok,
    },
    HostCallSpec {
        name: "spectra.api.query.binding_error",
        function: query::binding_error,
    },
    HostCallSpec {
        name: "spectra.api.query.binding_count",
        function: query::binding_count,
    },
    HostCallSpec {
        name: "spectra.api.query.binding_value",
        function: query::binding_value,
    },
    HostCallSpec {
        name: "spectra.api.query.binding_int",
        function: query::binding_int,
    },
    HostCallSpec {
        name: "spectra.api.query.binding_bool",
        function: query::binding_bool,
    },
    HostCallSpec {
        name: "spectra.api.query.error_code",
        function: query::error_code,
    },
    HostCallSpec {
        name: "spectra.api.query.error_message",
        function: query::error_message,
    },
    HostCallSpec {
        name: "spectra.api.form.type_string",
        function: form::type_string,
    },
    HostCallSpec {
        name: "spectra.api.form.type_int",
        function: form::type_int,
    },
    HostCallSpec {
        name: "spectra.api.form.type_bool",
        function: form::type_bool,
    },
    HostCallSpec {
        name: "spectra.api.form.parse",
        function: form::parse,
    },
    HostCallSpec {
        name: "spectra.api.form.len",
        function: form::len,
    },
    HostCallSpec {
        name: "spectra.api.form.has",
        function: form::has,
    },
    HostCallSpec {
        name: "spectra.api.form.count",
        function: form::count,
    },
    HostCallSpec {
        name: "spectra.api.form.first",
        function: form::first,
    },
    HostCallSpec {
        name: "spectra.api.form.value",
        function: form::value,
    },
    HostCallSpec {
        name: "spectra.api.form.int",
        function: form::int,
    },
    HostCallSpec {
        name: "spectra.api.form.bool",
        function: form::bool,
    },
    HostCallSpec {
        name: "spectra.api.form.schema",
        function: form::schema,
    },
    HostCallSpec {
        name: "spectra.api.form.schema_field",
        function: form::schema_field,
    },
    HostCallSpec {
        name: "spectra.api.form.bind",
        function: form::bind,
    },
    HostCallSpec {
        name: "spectra.api.form.binding_ok",
        function: form::binding_ok,
    },
    HostCallSpec {
        name: "spectra.api.form.binding_error",
        function: form::binding_error,
    },
    HostCallSpec {
        name: "spectra.api.form.binding_count",
        function: form::binding_count,
    },
    HostCallSpec {
        name: "spectra.api.form.binding_value",
        function: form::binding_value,
    },
    HostCallSpec {
        name: "spectra.api.form.binding_int",
        function: form::binding_int,
    },
    HostCallSpec {
        name: "spectra.api.form.binding_bool",
        function: form::binding_bool,
    },
    HostCallSpec {
        name: "spectra.api.form.error_code",
        function: form::error_code,
    },
    HostCallSpec {
        name: "spectra.api.form.error_message",
        function: form::error_message,
    },
    HostCallSpec {
        name: "spectra.api.multipart.parse",
        function: multipart::parse,
    },
    HostCallSpec {
        name: "spectra.api.multipart.part_count",
        function: multipart::part_count,
    },
    HostCallSpec {
        name: "spectra.api.multipart.field_count",
        function: multipart::field_count,
    },
    HostCallSpec {
        name: "spectra.api.multipart.file_count",
        function: multipart::file_count,
    },
    HostCallSpec {
        name: "spectra.api.multipart.text",
        function: multipart::text,
    },
    HostCallSpec {
        name: "spectra.api.multipart.part",
        function: multipart::part,
    },
    HostCallSpec {
        name: "spectra.api.multipart.part_name",
        function: multipart::part_name,
    },
    HostCallSpec {
        name: "spectra.api.multipart.part_filename",
        function: multipart::part_filename,
    },
    HostCallSpec {
        name: "spectra.api.multipart.part_content_type",
        function: multipart::part_content_type,
    },
    HostCallSpec {
        name: "spectra.api.multipart.part_size",
        function: multipart::part_size,
    },
    HostCallSpec {
        name: "spectra.api.multipart.part_is_file",
        function: multipart::part_is_file,
    },
    HostCallSpec {
        name: "spectra.api.multipart.file_path",
        function: multipart::file_path,
    },
    HostCallSpec {
        name: "spectra.api.multipart.file_read",
        function: multipart::file_read,
    },
    HostCallSpec {
        name: "spectra.api.multipart.file_spool_to",
        function: multipart::file_spool_to,
    },
    HostCallSpec {
        name: "spectra.api.multipart.error_code",
        function: multipart::error_code,
    },
    HostCallSpec {
        name: "spectra.api.multipart.error_message",
        function: multipart::error_message,
    },
    HostCallSpec {
        name: "spectra.api.handler.text",
        function: handler::text,
    },
    HostCallSpec {
        name: "spectra.api.handler.json",
        function: handler::json,
    },
    HostCallSpec {
        name: "spectra.api.handler.bytes",
        function: handler::bytes,
    },
    HostCallSpec {
        name: "spectra.api.handler.status",
        function: handler::status,
    },
    HostCallSpec {
        name: "spectra.api.handler.with_header",
        function: handler::with_header,
    },
    HostCallSpec {
        name: "spectra.api.handler.into_response",
        function: handler::into_response,
    },
    HostCallSpec {
        name: "spectra.api.handler.into_text_response",
        function: handler::into_text_response,
    },
    HostCallSpec {
        name: "spectra.api.handler.into_status_response",
        function: handler::into_status_response,
    },
    HostCallSpec {
        name: "spectra.api.handler.error",
        function: handler::error,
    },
    HostCallSpec {
        name: "spectra.api.handler.error_response",
        function: handler::error_response,
    },
    HostCallSpec {
        name: "spectra.api.handler.error_code",
        function: handler::error_code,
    },
    HostCallSpec {
        name: "spectra.api.handler.error_message",
        function: handler::error_message,
    },
    HostCallSpec {
        name: "spectra.api.handler.last_error_message",
        function: handler::last_error_message,
    },
    HostCallSpec {
        name: "spectra.api.handler.register_sync",
        function: handler::register_sync,
    },
    HostCallSpec {
        name: "spectra.api.handler.register_async",
        function: handler::register_async,
    },
    HostCallSpec {
        name: "spectra.api.handler.dispatch_sync",
        function: handler::dispatch_sync,
    },
    HostCallSpec {
        name: "spectra.api.handler.dispatch_async",
        function: handler::dispatch_async,
    },
    HostCallSpec {
        name: "spectra.api.cors.policy",
        function: cors::policy,
    },
    HostCallSpec {
        name: "spectra.api.cors.permissive",
        function: cors::permissive,
    },
    HostCallSpec {
        name: "spectra.api.cors.allow_origin",
        function: cors::allow_origin,
    },
    HostCallSpec {
        name: "spectra.api.cors.allow_method",
        function: cors::allow_method,
    },
    HostCallSpec {
        name: "spectra.api.cors.allow_header",
        function: cors::allow_header,
    },
    HostCallSpec {
        name: "spectra.api.cors.expose_header",
        function: cors::expose_header,
    },
    HostCallSpec {
        name: "spectra.api.cors.allow_credentials",
        function: cors::allow_credentials,
    },
    HostCallSpec {
        name: "spectra.api.cors.max_age",
        function: cors::max_age,
    },
    HostCallSpec {
        name: "spectra.api.cors.middleware",
        function: cors::middleware,
    },
    HostCallSpec {
        name: "spectra.api.cors.is_preflight",
        function: cors::is_preflight,
    },
    HostCallSpec {
        name: "spectra.api.cors.preflight",
        function: cors::preflight,
    },
    HostCallSpec {
        name: "spectra.api.cors.apply",
        function: cors::apply,
    },
    HostCallSpec {
        name: "spectra.api.cors.allowed_origin",
        function: cors::allowed_origin,
    },
    HostCallSpec {
        name: "spectra.api.middleware.chain",
        function: middleware::chain,
    },
    HostCallSpec {
        name: "spectra.api.middleware.chain_new",
        function: middleware::chain_new,
    },
    HostCallSpec {
        name: "spectra.api.middleware.chain_len",
        function: middleware::chain_len,
    },
    HostCallSpec {
        name: "spectra.api.middleware.register_sync",
        function: middleware::register_sync,
    },
    HostCallSpec {
        name: "spectra.api.middleware.register_sync_short_circuit",
        function: middleware::register_sync_short_circuit,
    },
    HostCallSpec {
        name: "spectra.api.middleware.register_async",
        function: middleware::register_async,
    },
    HostCallSpec {
        name: "spectra.api.middleware.register_async_short_circuit",
        function: middleware::register_async_short_circuit,
    },
    HostCallSpec {
        name: "spectra.api.middleware.use_sync",
        function: middleware::use_sync,
    },
    HostCallSpec {
        name: "spectra.api.middleware.use_async",
        function: middleware::use_async,
    },
    HostCallSpec {
        name: "spectra.api.middleware.execute_sync",
        function: middleware::execute_sync,
    },
    HostCallSpec {
        name: "spectra.api.middleware.execute_async",
        function: middleware::execute_async,
    },
    HostCallSpec {
        name: "spectra.api.middleware.last_trace",
        function: middleware::last_trace,
    },
    HostCallSpec {
        name: "spectra.api.middleware.trace_len",
        function: middleware::trace_len,
    },
    HostCallSpec {
        name: "spectra.api.middleware.trace_event",
        function: middleware::trace_event,
    },
    HostCallSpec {
        name: "spectra.api.middleware.trace_short_circuited",
        function: middleware::trace_short_circuited,
    },
    HostCallSpec {
        name: "spectra.api.errors.last_code",
        function: errors::last_code,
    },
    HostCallSpec {
        name: "spectra.api.errors.last_message",
        function: errors::last_message,
    },
    HostCallSpec {
        name: "spectra.api.trace.config_new",
        function: trace::config_new,
    },
    HostCallSpec {
        name: "spectra.api.trace.config_set_sample_rate",
        function: trace::config_set_sample_rate,
    },
    HostCallSpec {
        name: "spectra.api.trace.config_set_batch_size",
        function: trace::config_set_batch_size,
    },
    HostCallSpec {
        name: "spectra.api.trace.config_start",
        function: trace::config_start,
    },
    HostCallSpec {
        name: "spectra.api.trace.config_shutdown",
        function: trace::config_shutdown,
    },
    HostCallSpec {
        name: "spectra.api.trace.span_start",
        function: trace::span_start,
    },
    HostCallSpec {
        name: "spectra.api.trace.span_set_attribute",
        function: trace::span_set_attribute,
    },
    HostCallSpec {
        name: "spectra.api.trace.span_set_attribute_int",
        function: trace::span_set_attribute_int,
    },
    HostCallSpec {
        name: "spectra.api.trace.span_set_attribute_bool",
        function: trace::span_set_attribute_bool,
    },
    HostCallSpec {
        name: "spectra.api.trace.span_set_status",
        function: trace::span_set_status,
    },
    HostCallSpec {
        name: "spectra.api.trace.span_end",
        function: trace::span_end,
    },
    HostCallSpec {
        name: "spectra.api.trace.current",
        function: trace::current,
    },
    HostCallSpec {
        name: "spectra.api.trace.parent",
        function: trace::parent,
    },
    HostCallSpec {
        name: "spectra.api.trace.inject",
        function: trace::inject,
    },
    HostCallSpec {
        name: "spectra.api.trace.extract",
        function: trace::extract,
    },
    HostCallSpec {
        name: "spectra.api.trace.flush",
        function: trace::flush,
    },
    HostCallSpec {
        name: "spectra.api.trace.last_error",
        function: trace::last_error,
    },
    HostCallSpec {
        name: "spectra.api.db.sqlite.open",
        function: db::sqlite_open,
    },
    HostCallSpec {
        name: "spectra.api.db.sqlite.close",
        function: db::sqlite_close,
    },
    HostCallSpec {
        name: "spectra.api.db.sqlite.prepare",
        function: db::sqlite_prepare,
    },
    HostCallSpec {
        name: "spectra.api.db.sqlite.execute_async",
        function: db::sqlite_execute_async,
    },
    HostCallSpec {
        name: "spectra.api.db.sqlite.bind_null",
        function: db::sqlite_bind_null,
    },
    HostCallSpec {
        name: "spectra.api.db.sqlite.bind_int",
        function: db::sqlite_bind_int,
    },
    HostCallSpec {
        name: "spectra.api.db.sqlite.bind_float",
        function: db::sqlite_bind_float,
    },
    HostCallSpec {
        name: "spectra.api.db.sqlite.bind_text",
        function: db::sqlite_bind_text,
    },
    HostCallSpec {
        name: "spectra.api.db.sqlite.bind_blob",
        function: db::sqlite_bind_blob,
    },
    HostCallSpec {
        name: "spectra.api.db.sqlite.step",
        function: db::sqlite_step,
    },
    HostCallSpec {
        name: "spectra.api.db.sqlite.column_count",
        function: db::sqlite_column_count,
    },
    HostCallSpec {
        name: "spectra.api.db.sqlite.column_type",
        function: db::sqlite_column_type,
    },
    HostCallSpec {
        name: "spectra.api.db.sqlite.column_int",
        function: db::sqlite_column_int,
    },
    HostCallSpec {
        name: "spectra.api.db.sqlite.column_float",
        function: db::sqlite_column_float,
    },
    HostCallSpec {
        name: "spectra.api.db.sqlite.column_text",
        function: db::sqlite_column_text,
    },
    HostCallSpec {
        name: "spectra.api.db.sqlite.reset",
        function: db::sqlite_reset,
    },
    HostCallSpec {
        name: "spectra.api.db.sqlite.finalize",
        function: db::sqlite_finalize,
    },
    HostCallSpec {
        name: "spectra.api.db.sqlite.begin",
        function: db::sqlite_begin,
    },
    HostCallSpec {
        name: "spectra.api.db.sqlite.commit",
        function: db::sqlite_commit,
    },
    HostCallSpec {
        name: "spectra.api.db.sqlite.rollback",
        function: db::sqlite_rollback,
    },
    HostCallSpec {
        name: "spectra.api.db.sqlite.last_error_code",
        function: db::sqlite_last_error_code,
    },
    HostCallSpec {
        name: "spectra.api.db.sqlite.last_error_message",
        function: db::sqlite_last_error_message,
    },
    HostCallSpec { name: "spectra.api.db.postgres.open", function: db::postgres_open },
    HostCallSpec { name: "spectra.api.db.postgres.close", function: db::postgres_close },
    HostCallSpec { name: "spectra.api.db.postgres.prepare", function: db::postgres_prepare },
    HostCallSpec { name: "spectra.api.db.postgres.bind_null", function: db::postgres_bind_null },
    HostCallSpec { name: "spectra.api.db.postgres.bind_int", function: db::postgres_bind_int },
    HostCallSpec { name: "spectra.api.db.postgres.bind_float", function: db::postgres_bind_float },
    HostCallSpec { name: "spectra.api.db.postgres.bind_text", function: db::postgres_bind_text },
    HostCallSpec { name: "spectra.api.db.postgres.step", function: db::postgres_step },
    HostCallSpec { name: "spectra.api.db.postgres.column_count", function: db::postgres_column_count },
    HostCallSpec { name: "spectra.api.db.postgres.column_type", function: db::postgres_column_type },
    HostCallSpec { name: "spectra.api.db.postgres.column_int", function: db::postgres_column_int },
    HostCallSpec { name: "spectra.api.db.postgres.column_text", function: db::postgres_column_text },
    HostCallSpec { name: "spectra.api.db.postgres.reset", function: db::postgres_reset },
    HostCallSpec { name: "spectra.api.db.postgres.finalize", function: db::postgres_finalize },
    HostCallSpec { name: "spectra.api.db.postgres.begin", function: db::postgres_begin },
    HostCallSpec { name: "spectra.api.db.postgres.commit", function: db::postgres_commit },
    HostCallSpec { name: "spectra.api.db.postgres.rollback", function: db::postgres_rollback },
];

pub fn register() -> usize {
    spectra_runtime::initialize();
    spectra_runtime::register_standard_library();
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
    use std::io::{Read, Write};
    use std::net::TcpStream;
    use std::sync::{Mutex, MutexGuard, OnceLock};
    use std::time::Duration;

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
        assert_eq!(HOST_CALLS.len(), 250);
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
        assert_eq!(
            call("spectra.api.version.major", &[]),
            (HOST_STATUS_SUCCESS, 0)
        );
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
            call("spectra.api.server.listen", &[server, 0]),
            (HOST_STATUS_SUCCESS, 1)
        );
        assert_eq!(
            call("spectra.api.server.local_port", &[server]),
            (HOST_STATUS_SUCCESS, 0)
        );
        assert_eq!(
            call("spectra.api.server.shutdown", &[server]),
            (HOST_STATUS_SUCCESS, 1)
        );
        assert_eq!(
            call("spectra.api.server.state", &[server]),
            (HOST_STATUS_SUCCESS, server::SERVER_STATE_STOPPED)
        );
        assert_eq!(
            call(
                "spectra.api.server.signal",
                &[server, server::SERVER_SIGNAL_SIGINT]
            ),
            (HOST_STATUS_SUCCESS, 1)
        );
        assert_eq!(
            call("spectra.api.server.stats", &[server, 12]),
            (HOST_STATUS_SUCCESS, 0)
        );
        clear_host_functions();
    }

    #[test]
    fn r2216_registered_server_lifecycle_serves_router_and_signal_shutdown() {
        let _guard = test_guard();
        clear_host_functions();
        spectra_rt_manual_clear();
        register();

        let (_, router) = call("spectra.api.routing.router_new", &[]);
        let route = call(
            "spectra.api.routing.get",
            &[router, alloc_spectra_string("/hello")],
        );
        assert_eq!(route.0, HOST_STATUS_SUCCESS);
        assert!(route.1 > 0);
        let response = call("spectra.api.handler.text", &[alloc_spectra_string("hello")]);
        assert_eq!(response.0, HOST_STATUS_SUCCESS);
        assert!(response.1 > 0);
        let handler = call("spectra.api.handler.register_sync", &[route.1, response.1]);
        assert_eq!(handler.0, HOST_STATUS_SUCCESS);
        assert!(handler.1 > 0);

        let (_, server) = call("spectra.api.server.new", &[]);
        assert_eq!(
            call("spectra.api.server.listen", &[server, 0]),
            (HOST_STATUS_SUCCESS, 1)
        );
        assert_eq!(
            call("spectra.api.server.serve", &[server, router]),
            (HOST_STATUS_SUCCESS, 1)
        );
        let (_, port) = call("spectra.api.server.local_port", &[server]);
        assert!(port > 0);

        let mut stream =
            TcpStream::connect(("127.0.0.1", port as u16)).expect("connect served host");
        stream
            .set_read_timeout(Some(Duration::from_secs(2)))
            .expect("set read timeout");
        stream
            .write_all(b"GET /hello HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n")
            .expect("write request");
        let mut raw = Vec::new();
        stream.read_to_end(&mut raw).expect("read response");
        let parsed = http::parse_response(&raw).expect("parse response");
        assert_eq!(parsed.status_code, 200);
        assert_eq!(parsed.body.bytes(), b"hello");

        assert_eq!(
            call(
                "spectra.api.server.signal",
                &[server, server::SERVER_SIGNAL_SIGTERM]
            ),
            (HOST_STATUS_SUCCESS, 1)
        );
        assert_eq!(
            call("spectra.api.server.state", &[server]),
            (HOST_STATUS_SUCCESS, server::SERVER_STATE_STOPPED)
        );
        assert_eq!(
            call("spectra.api.server.stats", &[server, 10]),
            (HOST_STATUS_SUCCESS, 1)
        );
        assert_eq!(
            call("spectra.api.server.stats", &[server, 12]),
            (HOST_STATUS_SUCCESS, 0)
        );

        clear_host_functions();
        spectra_rt_manual_clear();
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
