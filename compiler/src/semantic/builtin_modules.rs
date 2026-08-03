// Builtin (virtual) module registrations
// Maps well-known `std.*` module paths to their exported function signatures
// without requiring physical `.spectra` files.  The actual implementation of
// each function lives in the runtime FFI layer (runtime/src/stdlib/mod.rs).

use super::module_registry::{
    ExportVisibility, ExportedFunction, ExportedSelfParamKind, ExportedTrait, ExportedTraitMethod,
    ExportedType, ModuleExports, ModuleRegistry,
};
use crate::ast::{FloatWidth, IntWidth, Type};

pub const STD_API_MODULE_PATHS: &[&str] = &[
    "std.api",
    "std.api.http",
    "std.api.server",
    "std.api.client",
    "std.api.json",
    "std.api.tls",
    "std.api.routing",
    "std.api.query",
    "std.api.form",
    "std.api.multipart",
    "std.api.handler",
    "std.api.cors",
    "std.api.middleware",
    "std.api.errors",
];

pub const STD_API_PUBLIC_TYPES: &[(&str, &str)] = &[
    ("std.api.http.Request", "record Request"),
    ("std.api.http.Response", "record Response"),
    ("std.api.http.Method", "record Method"),
    ("std.api.http.Status", "record Status"),
    ("std.api.http.Header", "record Header"),
    ("std.api.http.Headers", "record Headers"),
    ("std.api.http.Cookie", "record Cookie"),
    ("std.api.http.Body", "record Body"),
    ("std.api.server.Server", "record Server"),
    ("std.api.client.Client", "record Client"),
    ("std.api.json.JsonValue", "record JsonValue"),
    ("std.api.tls.TlsConfig", "record TlsConfig"),
    ("std.api.routing.Route", "record Route"),
    ("std.api.routing.Router", "record Router"),
    ("std.api.routing.RouteMatch", "record RouteMatch"),
    ("std.api.query.Query", "record Query"),
    ("std.api.query.QuerySchema", "record QuerySchema"),
    ("std.api.query.QueryBinding", "record QueryBinding"),
    ("std.api.form.Form", "record Form"),
    ("std.api.form.FormSchema", "record FormSchema"),
    ("std.api.form.FormBinding", "record FormBinding"),
    ("std.api.multipart.Multipart", "record Multipart"),
    ("std.api.multipart.MultipartPart", "record MultipartPart"),
    ("std.api.handler.HandlerHandle", "record HandlerHandle"),
    (
        "std.api.handler.AsyncHandlerHandle",
        "record AsyncHandlerHandle",
    ),
    ("std.api.handler.HandlerError", "record HandlerError"),
    (
        "std.api.middleware.MiddlewareChain",
        "record MiddlewareChain",
    ),
    (
        "std.api.middleware.MiddlewareHandle",
        "record MiddlewareHandle",
    ),
    (
        "std.api.middleware.AsyncMiddlewareHandle",
        "record AsyncMiddlewareHandle",
    ),
    (
        "std.api.middleware.MiddlewareTrace",
        "record MiddlewareTrace",
    ),
    ("std.api.cors.CorsPolicy", "record CorsPolicy"),
    ("std.api.errors.ApiError", "record ApiError"),
];

pub const STD_API_PUBLIC_FUNCTIONS: &[(&str, &str)] = &[
    ("std.api.http.method_name", "func(int) returns string"),
    ("std.api.http.method_allows_body", "func(int) returns bool"),
    ("std.api.http.method_is_safe", "func(int) returns bool"),
    ("std.api.http.method_get", "func() returns int"),
    ("std.api.http.method_head", "func() returns int"),
    ("std.api.http.method_post", "func() returns int"),
    ("std.api.http.method_put", "func() returns int"),
    ("std.api.http.method_patch", "func() returns int"),
    ("std.api.http.method_delete", "func() returns int"),
    ("std.api.http.method_options", "func() returns int"),
    ("std.api.http.status_reason", "func(int) returns string"),
    ("std.api.http.status_class", "func(int) returns int"),
    ("std.api.http.status_is_success", "func(int) returns bool"),
    ("std.api.http.status_continue", "func() returns int"),
    ("std.api.http.status_switching_protocols", "func() returns int"),
    ("std.api.http.status_ok", "func() returns int"),
    ("std.api.http.status_created", "func() returns int"),
    ("std.api.http.status_accepted", "func() returns int"),
    ("std.api.http.status_no_content", "func() returns int"),
    ("std.api.http.status_moved_permanently", "func() returns int"),
    ("std.api.http.status_found", "func() returns int"),
    ("std.api.http.status_not_modified", "func() returns int"),
    ("std.api.http.status_bad_request", "func() returns int"),
    ("std.api.http.status_unauthorized", "func() returns int"),
    ("std.api.http.status_forbidden", "func() returns int"),
    ("std.api.http.status_not_found", "func() returns int"),
    ("std.api.http.status_method_not_allowed", "func() returns int"),
    ("std.api.http.status_conflict", "func() returns int"),
    ("std.api.http.status_unsupported_media_type", "func() returns int"),
    ("std.api.http.status_unprocessable_content", "func() returns int"),
    ("std.api.http.status_too_many_requests", "func() returns int"),
    ("std.api.http.status_internal_server_error", "func() returns int"),
    ("std.api.http.status_bad_gateway", "func() returns int"),
    ("std.api.http.status_service_unavailable", "func() returns int"),
    ("std.api.http.status_gateway_timeout", "func() returns int"),
    ("std.api.http.header_name_is_valid", "func(string) returns bool"),
    ("std.api.http.header_value_is_valid", "func(string) returns bool"),
    ("std.api.http.request", "func(int, string) returns Request"),
    ("std.api.http.request_new", "func(int) returns Request"),
    ("std.api.http.request_method", "func(Request) returns int"),
    ("std.api.http.request_path", "func(Request) returns string"),
    (
        "std.api.http.request_header",
        "func(Request, string) returns string",
    ),
    (
        "std.api.http.request_with_header",
        "func(Request, string, string) returns Request",
    ),
    (
        "std.api.http.request_cookie",
        "func(Request, string) returns string",
    ),
    ("std.api.http.response", "func(int) returns Response"),
    ("std.api.http.response_new", "func(int) returns Response"),
    ("std.api.http.response_status", "func(Response) returns int"),
    (
        "std.api.http.response_header",
        "func(Response, string) returns string",
    ),
    ("std.api.http.response_body_len", "func(Response) returns int"),
    ("std.api.http.header", "func(string, string) returns Header"),
    ("std.api.http.header_name", "func(Header) returns string"),
    ("std.api.http.header_value", "func(Header) returns string"),
    ("std.api.http.cookie", "func(string, string) returns Cookie"),
    ("std.api.http.cookie_name", "func(Cookie) returns string"),
    ("std.api.http.cookie_value", "func(Cookie) returns string"),
    ("std.api.http.status", "func(int) returns Status"),
    ("std.api.server.new", "func() returns Server"),
    ("std.api.server.listen", "func(Server, int) returns bool"),
    ("std.api.server.serve", "func(Server, Router) returns task<int>"),
    ("std.api.server.state", "func(Server) returns int"),
    ("std.api.server.shutdown", "func(Server) returns bool"),
    ("std.api.server.local_port", "func(Server) returns int"),
    ("std.api.server.signal", "func(Server, int) returns bool"),
    ("std.api.server.stats", "func(Server, int) returns int"),
    ("std.api.client.new", "func() returns Client"),
    (
        "std.api.client.request",
        "func(Client, Request) returns task<Response>",
    ),
    ("std.api.client.timeout_ms", "func(Client) returns int"),
    ("std.api.json.validate", "func(string) returns bool"),
    ("std.api.json.kind", "func(string) returns int"),
    ("std.api.json.encode", "func(unknown) returns string"),
    ("std.api.json.decode", "func(string) returns JsonValue"),
    ("std.api.tls.config_new", "func(int) returns TlsConfig"),
    ("std.api.tls.config_mode", "func(TlsConfig) returns int"),
    (
        "std.api.tls.server_config",
        "func(string, string) returns TlsConfig",
    ),
    ("std.api.tls.client_config", "func() returns TlsConfig"),
    ("std.api.routing.router", "func() returns Router"),
    ("std.api.routing.router_new", "func() returns Router"),
    ("std.api.routing.route_count", "func(Router) returns int"),
    ("std.api.routing.route_id", "func(Route) returns int"),
    (
        "std.api.routing.route_add",
        "func(Router, int, string) returns Route",
    ),
    (
        "std.api.routing.route_match",
        "func(Router, int, string) returns RouteMatch",
    ),
    ("std.api.routing.match_route_id", "func(RouteMatch) returns int"),
    (
        "std.api.routing.match_param",
        "func(RouteMatch, string) returns string",
    ),
    (
        "std.api.routing.match_param_int",
        "func(RouteMatch, string) returns int",
    ),
    ("std.api.routing.last_conflict", "func() returns string"),
    ("std.api.routing.get", "func(Router, string) returns Route"),
    ("std.api.routing.post", "func(Router, string) returns Route"),
    ("std.api.routing.put", "func(Router, string) returns Route"),
    ("std.api.routing.patch", "func(Router, string) returns Route"),
    ("std.api.routing.delete", "func(Router, string) returns Route"),
    ("std.api.query.type_string", "func() returns int"),
    ("std.api.query.type_int", "func() returns int"),
    ("std.api.query.type_bool", "func() returns int"),
    ("std.api.query.parse", "func(string) returns Query"),
    ("std.api.query.len", "func(Query) returns int"),
    ("std.api.query.has", "func(Query, string) returns bool"),
    ("std.api.query.count", "func(Query, string) returns int"),
    ("std.api.query.first", "func(Query, string) returns string"),
    ("std.api.query.value", "func(Query, string, int) returns string"),
    ("std.api.query.int", "func(Query, string, int) returns int"),
    ("std.api.query.bool", "func(Query, string, int) returns bool"),
    ("std.api.query.schema", "func() returns QuerySchema"),
    (
        "std.api.query.schema_field",
        "func(QuerySchema, string, int, bool, bool) returns QuerySchema",
    ),
    (
        "std.api.query.bind",
        "func(Query, QuerySchema) returns QueryBinding",
    ),
    ("std.api.query.binding_ok", "func(QueryBinding) returns bool"),
    ("std.api.query.binding_error", "func(QueryBinding) returns string"),
    (
        "std.api.query.binding_count",
        "func(QueryBinding, string) returns int",
    ),
    (
        "std.api.query.binding_value",
        "func(QueryBinding, string, int) returns string",
    ),
    (
        "std.api.query.binding_int",
        "func(QueryBinding, string, int) returns int",
    ),
    (
        "std.api.query.binding_bool",
        "func(QueryBinding, string, int) returns bool",
    ),
    ("std.api.query.error_code", "func() returns int"),
    ("std.api.query.error_message", "func() returns string"),
    ("std.api.form.type_string", "func() returns int"),
    ("std.api.form.type_int", "func() returns int"),
    ("std.api.form.type_bool", "func() returns int"),
    ("std.api.form.parse", "func(string) returns Form"),
    ("std.api.form.len", "func(Form) returns int"),
    ("std.api.form.has", "func(Form, string) returns bool"),
    ("std.api.form.count", "func(Form, string) returns int"),
    ("std.api.form.first", "func(Form, string) returns string"),
    ("std.api.form.value", "func(Form, string, int) returns string"),
    ("std.api.form.int", "func(Form, string, int) returns int"),
    ("std.api.form.bool", "func(Form, string, int) returns bool"),
    ("std.api.form.schema", "func() returns FormSchema"),
    (
        "std.api.form.schema_field",
        "func(FormSchema, string, int, bool, bool) returns FormSchema",
    ),
    ("std.api.form.bind", "func(Form, FormSchema) returns FormBinding"),
    ("std.api.form.binding_ok", "func(FormBinding) returns bool"),
    ("std.api.form.binding_error", "func(FormBinding) returns string"),
    (
        "std.api.form.binding_count",
        "func(FormBinding, string) returns int",
    ),
    (
        "std.api.form.binding_value",
        "func(FormBinding, string, int) returns string",
    ),
    (
        "std.api.form.binding_int",
        "func(FormBinding, string, int) returns int",
    ),
    (
        "std.api.form.binding_bool",
        "func(FormBinding, string, int) returns bool",
    ),
    ("std.api.form.error_code", "func() returns int"),
    ("std.api.form.error_message", "func() returns string"),
    (
        "std.api.multipart.parse",
        "func(string, string, int, int, int) returns Multipart",
    ),
    ("std.api.multipart.part_count", "func(Multipart) returns int"),
    ("std.api.multipart.field_count", "func(Multipart) returns int"),
    ("std.api.multipart.file_count", "func(Multipart) returns int"),
    (
        "std.api.multipart.text",
        "func(Multipart, string, int) returns string",
    ),
    (
        "std.api.multipart.part",
        "func(Multipart, int) returns MultipartPart",
    ),
    ("std.api.multipart.part_name", "func(MultipartPart) returns string"),
    (
        "std.api.multipart.part_filename",
        "func(MultipartPart) returns string",
    ),
    (
        "std.api.multipart.part_content_type",
        "func(MultipartPart) returns string",
    ),
    ("std.api.multipart.part_size", "func(MultipartPart) returns int"),
    (
        "std.api.multipart.part_is_file",
        "func(MultipartPart) returns bool",
    ),
    ("std.api.multipart.file_path", "func(MultipartPart) returns string"),
    (
        "std.api.multipart.file_read",
        "func(MultipartPart, int, int) returns string",
    ),
    (
        "std.api.multipart.file_spool_to",
        "func(MultipartPart, string) returns bool",
    ),
    ("std.api.multipart.error_code", "func() returns int"),
    ("std.api.multipart.error_message", "func() returns string"),
    ("std.api.handler.text", "func(string) returns Response"),
    ("std.api.handler.json", "func(string) returns Response"),
    ("std.api.handler.bytes", "func(string) returns Response"),
    ("std.api.handler.status", "func(int) returns Response"),
    (
        "std.api.handler.with_header",
        "func(Response, string, string) returns Response",
    ),
    ("std.api.handler.into_response", "func(Response) returns Response"),
    (
        "std.api.handler.into_text_response",
        "func(string) returns Response",
    ),
    (
        "std.api.handler.into_status_response",
        "func(int) returns Response",
    ),
    ("std.api.handler.error", "func(int, string) returns HandlerError"),
    (
        "std.api.handler.error_response",
        "func(HandlerError) returns Response",
    ),
    ("std.api.handler.error_code", "func(HandlerError) returns int"),
    (
        "std.api.handler.error_message",
        "func(HandlerError) returns string",
    ),
    ("std.api.handler.last_error_message", "func() returns string"),
    (
        "std.api.handler.register_sync",
        "func(int, Response) returns HandlerHandle",
    ),
    (
        "std.api.handler.register_async",
        "func(int, Response) returns AsyncHandlerHandle",
    ),
    (
        "std.api.handler.dispatch_sync",
        "func(HandlerHandle, Request) returns Response",
    ),
    (
        "std.api.handler.dispatch_async",
        "func(AsyncHandlerHandle, Request) returns Response",
    ),
    ("std.api.cors.policy", "func() returns CorsPolicy"),
    ("std.api.cors.permissive", "func() returns CorsPolicy"),
    (
        "std.api.cors.allow_origin",
        "func(CorsPolicy, string) returns CorsPolicy",
    ),
    (
        "std.api.cors.allow_method",
        "func(CorsPolicy, int) returns CorsPolicy",
    ),
    (
        "std.api.cors.allow_header",
        "func(CorsPolicy, string) returns CorsPolicy",
    ),
    (
        "std.api.cors.expose_header",
        "func(CorsPolicy, string) returns CorsPolicy",
    ),
    (
        "std.api.cors.allow_credentials",
        "func(CorsPolicy, bool) returns CorsPolicy",
    ),
    ("std.api.cors.max_age", "func(CorsPolicy, int) returns CorsPolicy"),
    (
        "std.api.cors.middleware",
        "func(CorsPolicy) returns MiddlewareHandle",
    ),
    ("std.api.cors.is_preflight", "func(Request) returns bool"),
    (
        "std.api.cors.preflight",
        "func(CorsPolicy, Request) returns Response",
    ),
    (
        "std.api.cors.apply",
        "func(CorsPolicy, Request, Response) returns Response",
    ),
    (
        "std.api.cors.allowed_origin",
        "func(CorsPolicy, string) returns string",
    ),
    ("std.api.middleware.chain", "func() returns MiddlewareChain"),
    ("std.api.middleware.chain_new", "func() returns MiddlewareChain"),
    ("std.api.middleware.chain_len", "func(MiddlewareChain) returns int"),
    (
        "std.api.middleware.register_sync",
        "func(string, string) returns MiddlewareHandle",
    ),
    (
        "std.api.middleware.register_sync_short_circuit",
        "func(string, string, Response) returns MiddlewareHandle",
    ),
    (
        "std.api.middleware.register_async",
        "func(string, string) returns AsyncMiddlewareHandle",
    ),
    (
        "std.api.middleware.register_async_short_circuit",
        "func(string, string, Response) returns AsyncMiddlewareHandle",
    ),
    (
        "std.api.middleware.use_sync",
        "func(MiddlewareChain, MiddlewareHandle) returns MiddlewareChain",
    ),
    (
        "std.api.middleware.use_async",
        "func(MiddlewareChain, AsyncMiddlewareHandle) returns MiddlewareChain",
    ),
    (
        "std.api.middleware.execute_sync",
        "func(MiddlewareChain, Request, Response) returns Response",
    ),
    (
        "std.api.middleware.execute_async",
        "func(MiddlewareChain, Request, Response) returns Response",
    ),
    ("std.api.middleware.last_trace", "func() returns MiddlewareTrace"),
    ("std.api.middleware.trace_len", "func(MiddlewareTrace) returns int"),
    (
        "std.api.middleware.trace_event",
        "func(MiddlewareTrace, int) returns string",
    ),
    (
        "std.api.middleware.trace_short_circuited",
        "func(MiddlewareTrace) returns bool",
    ),
    ("std.api.errors.last_code", "func() returns int"),
    ("std.api.errors.last_message", "func() returns string"),
];

pub const STD_TIME_PUBLIC_TYPES: &[(&str, &str)] = &[
    ("std.time.Duration", "record Duration"),
    ("std.time.Instant", "record Instant"),
    ("std.time.UtcDateTime", "record UtcDateTime"),
];

pub const STD_TIME_PUBLIC_FUNCTIONS: &[(&str, &str)] = &[
    ("std.time.time_now_millis", "func() returns int"),
    ("std.time.time_now_secs", "func() returns int"),
    ("std.time.sleep_ms", "func(int) returns unit"),
    ("std.time.monotonic_millis", "func() returns int"),
    ("std.time.monotonic_nanos", "func() returns int"),
    ("std.time.duration_ms", "func(int) returns Duration"),
    ("std.time.duration_secs", "func(int) returns Duration"),
    ("std.time.duration_millis", "func(Duration) returns int"),
    ("std.time.duration_secs_value", "func(Duration) returns int"),
    (
        "std.time.duration_add",
        "func(Duration, Duration) returns Duration",
    ),
    (
        "std.time.duration_sub",
        "func(Duration, Duration) returns Duration",
    ),
    ("std.time.instant_now", "func() returns Instant"),
    ("std.time.instant_elapsed_ms", "func(Instant) returns int"),
    ("std.time.instant_add", "func(Instant, Duration) returns Instant"),
    ("std.time.instant_has_elapsed", "func(Instant) returns bool"),
    ("std.time.sleep", "func(Duration) returns unit"),
    ("std.time.unix_to_utc", "func(int) returns UtcDateTime"),
    ("std.time.utc_year", "func(UtcDateTime) returns int"),
    ("std.time.utc_month", "func(UtcDateTime) returns int"),
    ("std.time.utc_day", "func(UtcDateTime) returns int"),
    ("std.time.utc_hour", "func(UtcDateTime) returns int"),
    ("std.time.utc_minute", "func(UtcDateTime) returns int"),
    ("std.time.utc_second", "func(UtcDateTime) returns int"),
];

pub const STD_RANGE_PUBLIC_TYPES: &[(&str, &str)] = &[("std.range.Range", "record Range")];

pub const STD_RANGE_PUBLIC_FUNCTIONS: &[(&str, &str)] = &[
    ("std.range.create", "func(int, int, bool) returns Range"),
    ("std.range.len", "func(Range) returns int"),
    ("std.range.at", "func(Range, int) returns int"),
    ("std.range.eq", "func(Range, Range) returns bool"),
    ("std.range.start", "func(Range) returns int"),
    ("std.range.end", "func(Range) returns int"),
    ("std.range.is_inclusive", "func(Range) returns bool"),
];

/// Register all built-in standard library modules in the given registry.
pub fn register_builtin_modules(registry: &mut ModuleRegistry) {
    registry.register_module("std.io".to_string(), make_std_io());
    registry.register_module("std.math".to_string(), make_std_math());
    registry.register_module("std.numeric".to_string(), make_std_numeric());
    registry.register_module("std.collections".to_string(), make_std_collections());
    registry.register_module("std.string".to_string(), make_std_string());
    registry.register_module("std.convert".to_string(), make_std_convert());
    registry.register_module("std.random".to_string(), make_std_random());
    registry.register_module("std.fs".to_string(), make_std_fs());
    registry.register_module("std.env".to_string(), make_std_env());
    registry.register_module("std.option".to_string(), make_std_option());
    registry.register_module("std.result".to_string(), make_std_result());
    registry.register_module("std.char".to_string(), make_std_char());
    registry.register_module("std.time".to_string(), make_std_time());
    registry.register_module("std.range".to_string(), make_std_range());
    registry.register_module("std.tensor".to_string(), make_std_tensor());
    registry.register_module("std.ml".to_string(), make_std_ml());
    registry.register_module("std.concurrent".to_string(), make_std_concurrent());
    registry.register_module("std.serve".to_string(), make_std_serve());
    register_std_api_modules(registry, "std.api");
    // Convenience aliases used in existing examples
    registry.register_module("spectra.std.io".to_string(), make_std_io());
    registry.register_module("spectra.std.math".to_string(), make_std_math());
    registry.register_module("spectra.std.numeric".to_string(), make_std_numeric());
    registry.register_module(
        "spectra.std.collections".to_string(),
        make_std_collections(),
    );
    registry.register_module("spectra.std.string".to_string(), make_std_string());
    registry.register_module("spectra.std.convert".to_string(), make_std_convert());
    registry.register_module("spectra.std.random".to_string(), make_std_random());
    registry.register_module("spectra.std.fs".to_string(), make_std_fs());
    registry.register_module("spectra.std.env".to_string(), make_std_env());
    registry.register_module("spectra.std.option".to_string(), make_std_option());
    registry.register_module("spectra.std.result".to_string(), make_std_result());
    registry.register_module("spectra.std.char".to_string(), make_std_char());
    registry.register_module("spectra.std.time".to_string(), make_std_time());
    registry.register_module("spectra.std.range".to_string(), make_std_range());
    registry.register_module("spectra.std.tensor".to_string(), make_std_tensor());
    registry.register_module("spectra.std.ml".to_string(), make_std_ml());
    registry.register_module("spectra.std.concurrent".to_string(), make_std_concurrent());
    registry.register_module("spectra.std.serve".to_string(), make_std_serve());
    register_std_api_modules(registry, "spectra.std.api");
}

fn pub_fn(params: Vec<Type>, return_type: Type) -> ExportedFunction {
    ExportedFunction {
        params,
        return_type,
        visibility: ExportVisibility::Public,
        is_async: false,
    }
}

fn exported_trait_method(
    params: Vec<Type>,
    return_type: Type,
    self_kind: Option<ExportedSelfParamKind>,
    is_async: bool,
) -> ExportedTraitMethod {
    ExportedTraitMethod {
        params,
        return_type,
        self_kind,
        is_async,
        has_default: false,
    }
}

fn public_type(members: &[&str]) -> ExportedType {
    ExportedType {
        members: members.iter().map(|member| (*member).to_string()).collect(),
        visibility: ExportVisibility::Public,
        is_enum: false,
        struct_fields: None,
        enum_variants: None,
        enum_struct_variants: None,
    }
}

fn api_type(name: &str) -> Type {
    Type::Struct {
        name: name.to_string(),
    }
}

fn api_task(output: Type) -> Type {
    Type::Task {
        output: Box::new(output),
    }
}

fn register_std_api_modules(registry: &mut ModuleRegistry, prefix: &str) {
    registry.register_module(prefix.to_string(), make_std_api_root(prefix));
    registry.register_module(format!("{prefix}.http"), make_std_api_http(prefix));
    registry.register_module(format!("{prefix}.server"), make_std_api_server(prefix));
    registry.register_module(format!("{prefix}.client"), make_std_api_client(prefix));
    registry.register_module(format!("{prefix}.json"), make_std_api_json(prefix));
    registry.register_module(format!("{prefix}.tls"), make_std_api_tls(prefix));
    registry.register_module(format!("{prefix}.routing"), make_std_api_routing(prefix));
    registry.register_module(format!("{prefix}.query"), make_std_api_query(prefix));
    registry.register_module(format!("{prefix}.form"), make_std_api_form(prefix));
    registry.register_module(
        format!("{prefix}.multipart"),
        make_std_api_multipart(prefix),
    );
    registry.register_module(format!("{prefix}.handler"), make_std_api_handler(prefix));
    registry.register_module(format!("{prefix}.cors"), make_std_api_cors(prefix));
    registry.register_module(
        format!("{prefix}.middleware"),
        make_std_api_middleware(prefix),
    );
    registry.register_module(format!("{prefix}.errors"), make_std_api_errors(prefix));
    registry.register_module(format!("{prefix}.trace"), make_std_api_trace(prefix));
    registry.register_module(format!("{prefix}.health"), make_std_api_health(prefix));
    registry.register_module(format!("{prefix}.db.sqlite"), make_std_api_db_sqlite(prefix));
    registry.register_module(format!("{prefix}.db.postgres"), make_std_api_db_postgres(prefix));
    registry.register_module(format!("{prefix}.db.redis"), make_std_api_db_redis(prefix));
}

fn stdlib_segments(prefix: &str) -> Vec<String> {
    prefix.split('.').map(|part| part.to_string()).collect()
}

fn api_module(prefix: &str, leaf: Option<&str>) -> ModuleExports {
    let mut stdlib_path = stdlib_segments(prefix);
    if let Some(leaf) = leaf {
        stdlib_path.push(leaf.to_string());
    }
    ModuleExports {
        stdlib_path: Some(stdlib_path),
        package_name: Some("spectra.api".to_string()),
        ..Default::default()
    }
}

fn make_std_api_root(prefix: &str) -> ModuleExports {
    let mut exports = api_module(prefix, None);
    for name in [
        "Request",
        "Response",
        "Router",
        "Server",
        "Client",
        "TlsConfig",
        "SqliteConnection",
        "SqliteStatement",
    ] {
        exports.types.insert(name.to_string(), public_type(&[]));
    }
    exports
}

fn make_std_range() -> ModuleExports {
    let mut exports = ModuleExports {
        stdlib_path: Some(vec!["std".to_string(), "range".to_string()]),
        package_name: Some("std".to_string()),
        ..Default::default()
    };

    exports.types.insert("Range".to_string(), public_type(&[]));

    let range = Type::Range;
    exports.functions.insert(
        "create".to_string(),
        pub_fn(vec![Type::Int, Type::Int, Type::Bool], range.clone()),
    );
    exports
        .functions
        .insert("len".to_string(), pub_fn(vec![range.clone()], Type::Int));
    exports.functions.insert(
        "at".to_string(),
        pub_fn(vec![range.clone(), Type::Int], Type::Int),
    );
    exports.functions.insert(
        "eq".to_string(),
        pub_fn(vec![range.clone(), range.clone()], Type::Bool),
    );
    exports
        .functions
        .insert("start".to_string(), pub_fn(vec![range.clone()], Type::Int));
    exports
        .functions
        .insert("end".to_string(), pub_fn(vec![range.clone()], Type::Int));
    exports
        .functions
        .insert("is_inclusive".to_string(), pub_fn(vec![range], Type::Bool));

    exports
}

fn make_std_api_http(prefix: &str) -> ModuleExports {
    let mut exports = api_module(prefix, Some("http"));
    for (name, members) in [
        ("Request", &["method", "path", "headers", "body"][..]),
        ("Response", &["status", "headers", "body"][..]),
        ("Method", &["code", "name"][..]),
        ("Status", &["code", "reason"][..]),
        ("Header", &["name", "value"][..]),
        ("Headers", &["len"][..]),
        ("Cookie", &["name", "value"][..]),
        ("Body", &["len"][..]),
    ] {
        exports.types.insert(name.to_string(), public_type(members));
    }

    let request = api_type("Request");
    let response = api_type("Response");
    let header = api_type("Header");
    let cookie = api_type("Cookie");
    let status = api_type("Status");
    let functions = [
        ("method_name", vec![Type::Int], Type::String),
        ("method_allows_body", vec![Type::Int], Type::Bool),
        ("method_is_safe", vec![Type::Int], Type::Bool),
        ("method_get", vec![], Type::Int),
        ("method_head", vec![], Type::Int),
        ("method_post", vec![], Type::Int),
        ("method_put", vec![], Type::Int),
        ("method_patch", vec![], Type::Int),
        ("method_delete", vec![], Type::Int),
        ("method_options", vec![], Type::Int),
        ("status_reason", vec![Type::Int], Type::String),
        ("status_class", vec![Type::Int], Type::Int),
        ("status_is_success", vec![Type::Int], Type::Bool),
        ("status_continue", vec![], Type::Int),
        ("status_switching_protocols", vec![], Type::Int),
        ("status_ok", vec![], Type::Int),
        ("status_created", vec![], Type::Int),
        ("status_accepted", vec![], Type::Int),
        ("status_no_content", vec![], Type::Int),
        ("status_moved_permanently", vec![], Type::Int),
        ("status_found", vec![], Type::Int),
        ("status_not_modified", vec![], Type::Int),
        ("status_bad_request", vec![], Type::Int),
        ("status_unauthorized", vec![], Type::Int),
        ("status_forbidden", vec![], Type::Int),
        ("status_not_found", vec![], Type::Int),
        ("status_method_not_allowed", vec![], Type::Int),
        ("status_conflict", vec![], Type::Int),
        ("status_unsupported_media_type", vec![], Type::Int),
        ("status_unprocessable_content", vec![], Type::Int),
        ("status_too_many_requests", vec![], Type::Int),
        ("status_internal_server_error", vec![], Type::Int),
        ("status_bad_gateway", vec![], Type::Int),
        ("status_service_unavailable", vec![], Type::Int),
        ("status_gateway_timeout", vec![], Type::Int),
        ("header_name_is_valid", vec![Type::String], Type::Bool),
        ("header_value_is_valid", vec![Type::String], Type::Bool),
        ("request", vec![Type::Int, Type::String], request.clone()),
        ("request_new", vec![Type::Int], request.clone()),
        ("request_method", vec![request.clone()], Type::Int),
        ("request_path", vec![request.clone()], Type::String),
        (
            "request_header",
            vec![request.clone(), Type::String],
            Type::String,
        ),
        (
            "request_with_header",
            vec![request.clone(), Type::String, Type::String],
            request.clone(),
        ),
        (
            "request_cookie",
            vec![request.clone(), Type::String],
            Type::String,
        ),
        ("response", vec![Type::Int], response.clone()),
        ("response_new", vec![Type::Int], response.clone()),
        ("response_status", vec![response.clone()], Type::Int),
        (
            "response_header",
            vec![response.clone(), Type::String],
            Type::String,
        ),
        ("response_body_len", vec![response], Type::Int),
        ("header", vec![Type::String, Type::String], header.clone()),
        ("header_name", vec![header.clone()], Type::String),
        ("header_value", vec![header], Type::String),
        ("cookie", vec![Type::String, Type::String], cookie.clone()),
        ("cookie_name", vec![cookie.clone()], Type::String),
        ("cookie_value", vec![cookie], Type::String),
        ("status", vec![Type::Int], status),
    ];
    for (name, params, return_type) in functions {
        exports
            .functions
            .insert(name.to_string(), pub_fn(params, return_type));
    }
    exports
}

fn make_std_api_server(prefix: &str) -> ModuleExports {
    let mut exports = api_module(prefix, Some("server"));
    exports
        .types
        .insert("Server".to_string(), public_type(&["state"]));
    let server = api_type("Server");
    let router = api_type("Router");
    let functions = [
        ("new", vec![], server.clone()),
        ("listen", vec![server.clone(), Type::Int], Type::Bool),
        ("serve", vec![server.clone(), router], api_task(Type::Int)),
        ("state", vec![server.clone()], Type::Int),
        ("shutdown", vec![server.clone()], Type::Bool),
        ("local_port", vec![server.clone()], Type::Int),
        ("signal", vec![server.clone(), Type::Int], Type::Bool),
        ("stats", vec![server, Type::Int], Type::Int),
    ];
    for (name, params, return_type) in functions {
        exports
            .functions
            .insert(name.to_string(), pub_fn(params, return_type));
    }
    exports
}

fn make_std_api_client(prefix: &str) -> ModuleExports {
    let mut exports = api_module(prefix, Some("client"));
    exports
        .types
        .insert("Client".to_string(), public_type(&["timeout_ms"]));
    let client = api_type("Client");
    let request = api_type("Request");
    let response = api_type("Response");
    let functions = [
        ("new", vec![], client.clone()),
        ("request", vec![client.clone(), request], api_task(response)),
        ("timeout_ms", vec![client], Type::Int),
    ];
    for (name, params, return_type) in functions {
        exports
            .functions
            .insert(name.to_string(), pub_fn(params, return_type));
    }
    exports
}

fn make_std_api_json(prefix: &str) -> ModuleExports {
    let mut exports = api_module(prefix, Some("json"));
    exports
        .types
        .insert("JsonValue".to_string(), public_type(&["kind"]));
    let functions = [
        ("validate", vec![Type::String], Type::Bool),
        ("kind", vec![Type::String], Type::Int),
        ("encode", vec![Type::Unknown], Type::String),
        ("decode", vec![Type::String], api_type("JsonValue")),
    ];
    for (name, params, return_type) in functions {
        exports
            .functions
            .insert(name.to_string(), pub_fn(params, return_type));
    }
    exports
}

fn make_std_api_tls(prefix: &str) -> ModuleExports {
    let mut exports = api_module(prefix, Some("tls"));
    exports
        .types
        .insert("TlsConfig".to_string(), public_type(&["mode"]));
    let tls_config = api_type("TlsConfig");
    let functions = [
        ("config_new", vec![Type::Int], tls_config.clone()),
        ("config_mode", vec![tls_config.clone()], Type::Int),
        (
            "server_config",
            vec![Type::String, Type::String],
            tls_config.clone(),
        ),
        ("client_config", vec![], tls_config),
    ];
    for (name, params, return_type) in functions {
        exports
            .functions
            .insert(name.to_string(), pub_fn(params, return_type));
    }
    exports
}

fn make_std_api_routing(prefix: &str) -> ModuleExports {
    let mut exports = api_module(prefix, Some("routing"));
    exports
        .types
        .insert("Route".to_string(), public_type(&["method", "path"]));
    exports
        .types
        .insert("Router".to_string(), public_type(&["route_count"]));
    exports
        .types
        .insert("RouteMatch".to_string(), public_type(&["route_id"]));
    let router = api_type("Router");
    let route = api_type("Route");
    let route_match = api_type("RouteMatch");
    let mut functions = vec![
        ("router", vec![], router.clone()),
        ("router_new", vec![], router.clone()),
        ("route_count", vec![router.clone()], Type::Int),
        ("route_id", vec![route.clone()], Type::Int),
        (
            "route_add",
            vec![router.clone(), Type::Int, Type::String],
            route.clone(),
        ),
        (
            "route_match",
            vec![router.clone(), Type::Int, Type::String],
            route_match.clone(),
        ),
        ("match_route_id", vec![route_match.clone()], Type::Int),
        (
            "match_param",
            vec![route_match.clone(), Type::String],
            Type::String,
        ),
        (
            "match_param_int",
            vec![route_match, Type::String],
            Type::Int,
        ),
        ("last_conflict", vec![], Type::String),
    ];
    for name in ["get", "post", "put", "patch", "delete"] {
        functions.push((name, vec![router.clone(), Type::String], route.clone()));
    }
    for (name, params, return_type) in functions {
        exports
            .functions
            .insert(name.to_string(), pub_fn(params, return_type));
    }
    exports
}

fn make_std_api_query(prefix: &str) -> ModuleExports {
    let mut exports = api_module(prefix, Some("query"));
    exports
        .types
        .insert("Query".to_string(), public_type(&["len"]));
    exports
        .types
        .insert("QuerySchema".to_string(), public_type(&["field_count"]));
    exports
        .types
        .insert("QueryBinding".to_string(), public_type(&["ok"]));
    let query = api_type("Query");
    let schema = api_type("QuerySchema");
    let binding = api_type("QueryBinding");
    let functions = [
        ("type_string", vec![], Type::Int),
        ("type_int", vec![], Type::Int),
        ("type_bool", vec![], Type::Int),
        ("parse", vec![Type::String], query.clone()),
        ("len", vec![query.clone()], Type::Int),
        ("has", vec![query.clone(), Type::String], Type::Bool),
        ("count", vec![query.clone(), Type::String], Type::Int),
        ("first", vec![query.clone(), Type::String], Type::String),
        (
            "value",
            vec![query.clone(), Type::String, Type::Int],
            Type::String,
        ),
        (
            "int",
            vec![query.clone(), Type::String, Type::Int],
            Type::Int,
        ),
        (
            "bool",
            vec![query.clone(), Type::String, Type::Int],
            Type::Bool,
        ),
        ("schema", vec![], schema.clone()),
        (
            "schema_field",
            vec![
                schema.clone(),
                Type::String,
                Type::Int,
                Type::Bool,
                Type::Bool,
            ],
            schema.clone(),
        ),
        ("bind", vec![query, schema], binding.clone()),
        ("binding_ok", vec![binding.clone()], Type::Bool),
        ("binding_error", vec![binding.clone()], Type::String),
        (
            "binding_count",
            vec![binding.clone(), Type::String],
            Type::Int,
        ),
        (
            "binding_value",
            vec![binding.clone(), Type::String, Type::Int],
            Type::String,
        ),
        (
            "binding_int",
            vec![binding.clone(), Type::String, Type::Int],
            Type::Int,
        ),
        (
            "binding_bool",
            vec![binding, Type::String, Type::Int],
            Type::Bool,
        ),
        ("error_code", vec![], Type::Int),
        ("error_message", vec![], Type::String),
    ];
    for (name, params, return_type) in functions {
        exports
            .functions
            .insert(name.to_string(), pub_fn(params, return_type));
    }
    exports
}

fn make_std_api_form(prefix: &str) -> ModuleExports {
    let mut exports = api_module(prefix, Some("form"));
    exports
        .types
        .insert("Form".to_string(), public_type(&["len"]));
    exports
        .types
        .insert("FormSchema".to_string(), public_type(&["field_count"]));
    exports
        .types
        .insert("FormBinding".to_string(), public_type(&["ok"]));
    let form = api_type("Form");
    let schema = api_type("FormSchema");
    let binding = api_type("FormBinding");
    let functions = [
        ("type_string", vec![], Type::Int),
        ("type_int", vec![], Type::Int),
        ("type_bool", vec![], Type::Int),
        ("parse", vec![Type::String], form.clone()),
        ("len", vec![form.clone()], Type::Int),
        ("has", vec![form.clone(), Type::String], Type::Bool),
        ("count", vec![form.clone(), Type::String], Type::Int),
        ("first", vec![form.clone(), Type::String], Type::String),
        (
            "value",
            vec![form.clone(), Type::String, Type::Int],
            Type::String,
        ),
        (
            "int",
            vec![form.clone(), Type::String, Type::Int],
            Type::Int,
        ),
        (
            "bool",
            vec![form.clone(), Type::String, Type::Int],
            Type::Bool,
        ),
        ("schema", vec![], schema.clone()),
        (
            "schema_field",
            vec![
                schema.clone(),
                Type::String,
                Type::Int,
                Type::Bool,
                Type::Bool,
            ],
            schema.clone(),
        ),
        ("bind", vec![form, schema], binding.clone()),
        ("binding_ok", vec![binding.clone()], Type::Bool),
        ("binding_error", vec![binding.clone()], Type::String),
        (
            "binding_count",
            vec![binding.clone(), Type::String],
            Type::Int,
        ),
        (
            "binding_value",
            vec![binding.clone(), Type::String, Type::Int],
            Type::String,
        ),
        (
            "binding_int",
            vec![binding.clone(), Type::String, Type::Int],
            Type::Int,
        ),
        (
            "binding_bool",
            vec![binding, Type::String, Type::Int],
            Type::Bool,
        ),
        ("error_code", vec![], Type::Int),
        ("error_message", vec![], Type::String),
    ];
    for (name, params, return_type) in functions {
        exports
            .functions
            .insert(name.to_string(), pub_fn(params, return_type));
    }
    exports
}

fn make_std_api_multipart(prefix: &str) -> ModuleExports {
    let mut exports = api_module(prefix, Some("multipart"));
    exports
        .types
        .insert("Multipart".to_string(), public_type(&["part_count"]));
    exports.types.insert(
        "MultipartPart".to_string(),
        public_type(&["name", "filename", "content_type", "size"]),
    );
    let multipart = api_type("Multipart");
    let part = api_type("MultipartPart");
    let functions = [
        (
            "parse",
            vec![Type::String, Type::String, Type::Int, Type::Int, Type::Int],
            multipart.clone(),
        ),
        ("part_count", vec![multipart.clone()], Type::Int),
        ("field_count", vec![multipart.clone()], Type::Int),
        ("file_count", vec![multipart.clone()], Type::Int),
        (
            "text",
            vec![multipart.clone(), Type::String, Type::Int],
            Type::String,
        ),
        ("part", vec![multipart, Type::Int], part.clone()),
        ("part_name", vec![part.clone()], Type::String),
        ("part_filename", vec![part.clone()], Type::String),
        ("part_content_type", vec![part.clone()], Type::String),
        ("part_size", vec![part.clone()], Type::Int),
        ("part_is_file", vec![part.clone()], Type::Bool),
        ("file_path", vec![part.clone()], Type::String),
        (
            "file_read",
            vec![part.clone(), Type::Int, Type::Int],
            Type::String,
        ),
        ("file_spool_to", vec![part, Type::String], Type::Bool),
        ("error_code", vec![], Type::Int),
        ("error_message", vec![], Type::String),
    ];
    for (name, params, return_type) in functions {
        exports
            .functions
            .insert(name.to_string(), pub_fn(params, return_type));
    }
    exports
}

fn make_std_api_handler(prefix: &str) -> ModuleExports {
    let mut exports = api_module(prefix, Some("handler"));
    exports
        .types
        .insert("HandlerHandle".to_string(), public_type(&["route_id"]));
    exports
        .types
        .insert("AsyncHandlerHandle".to_string(), public_type(&["route_id"]));
    exports.types.insert(
        "HandlerError".to_string(),
        public_type(&["status", "message"]),
    );

    let request = api_type("Request");
    let response = api_type("Response");
    let handler_handle = api_type("HandlerHandle");
    let async_handler_handle = api_type("AsyncHandlerHandle");
    let handler_error = api_type("HandlerError");

    let functions = [
        ("text", vec![Type::String], response.clone()),
        ("json", vec![Type::String], response.clone()),
        ("bytes", vec![Type::String], response.clone()),
        ("status", vec![Type::Int], response.clone()),
        (
            "with_header",
            vec![response.clone(), Type::String, Type::String],
            response.clone(),
        ),
        ("into_response", vec![response.clone()], response.clone()),
        ("into_text_response", vec![Type::String], response.clone()),
        ("into_status_response", vec![Type::Int], response.clone()),
        (
            "error",
            vec![Type::Int, Type::String],
            handler_error.clone(),
        ),
        (
            "error_response",
            vec![handler_error.clone()],
            response.clone(),
        ),
        ("error_code", vec![handler_error.clone()], Type::Int),
        ("error_message", vec![handler_error], Type::String),
        ("last_error_message", vec![], Type::String),
        (
            "register_sync",
            vec![Type::Int, response.clone()],
            handler_handle.clone(),
        ),
        (
            "register_async",
            vec![Type::Int, response.clone()],
            async_handler_handle.clone(),
        ),
        (
            "dispatch_sync",
            vec![handler_handle, request.clone()],
            response.clone(),
        ),
        (
            "dispatch_async",
            vec![async_handler_handle, request],
            response,
        ),
    ];
    for (name, params, return_type) in functions {
        exports
            .functions
            .insert(name.to_string(), pub_fn(params, return_type));
    }

    exports.traits.insert(
        "IntoResponse".to_string(),
        ExportedTrait {
            visibility: ExportVisibility::Public,
            methods: [(
                "into_response".to_string(),
                exported_trait_method(
                    vec![Type::Unknown],
                    api_type("Response"),
                    Some(ExportedSelfParamKind::Reference { mutable: false }),
                    false,
                ),
            )]
            .into_iter()
            .collect(),
        },
    );
    exports.traits.insert(
        "Handler".to_string(),
        ExportedTrait {
            visibility: ExportVisibility::Public,
            methods: [(
                "call".to_string(),
                exported_trait_method(
                    vec![Type::Unknown, api_type("Request")],
                    api_type("Response"),
                    Some(ExportedSelfParamKind::Reference { mutable: false }),
                    false,
                ),
            )]
            .into_iter()
            .collect(),
        },
    );
    exports.traits.insert(
        "AsyncHandler".to_string(),
        ExportedTrait {
            visibility: ExportVisibility::Public,
            methods: [(
                "call".to_string(),
                exported_trait_method(
                    vec![Type::Unknown, api_type("Request")],
                    api_task(api_type("Response")),
                    Some(ExportedSelfParamKind::Reference { mutable: false }),
                    true,
                ),
            )]
            .into_iter()
            .collect(),
        },
    );

    exports
}

fn make_std_api_middleware(prefix: &str) -> ModuleExports {
    let mut exports = api_module(prefix, Some("middleware"));
    exports.types.insert(
        "MiddlewareChain".to_string(),
        public_type(&["middleware_count"]),
    );
    exports
        .types
        .insert("MiddlewareHandle".to_string(), public_type(&["order"]));
    exports
        .types
        .insert("AsyncMiddlewareHandle".to_string(), public_type(&["order"]));
    exports
        .types
        .insert("MiddlewareTrace".to_string(), public_type(&["event_count"]));

    let chain = api_type("MiddlewareChain");
    let middleware = api_type("MiddlewareHandle");
    let async_middleware = api_type("AsyncMiddlewareHandle");
    let trace = api_type("MiddlewareTrace");
    let request = api_type("Request");
    let response = api_type("Response");

    let functions = [
        ("chain", vec![], chain.clone()),
        ("chain_new", vec![], chain.clone()),
        ("chain_len", vec![chain.clone()], Type::Int),
        (
            "register_sync",
            vec![Type::String, Type::String],
            middleware.clone(),
        ),
        (
            "register_sync_short_circuit",
            vec![Type::String, Type::String, response.clone()],
            middleware.clone(),
        ),
        (
            "register_async",
            vec![Type::String, Type::String],
            async_middleware.clone(),
        ),
        (
            "register_async_short_circuit",
            vec![Type::String, Type::String, response.clone()],
            async_middleware.clone(),
        ),
        ("use_sync", vec![chain.clone(), middleware], chain.clone()),
        (
            "use_async",
            vec![chain.clone(), async_middleware],
            chain.clone(),
        ),
        (
            "execute_sync",
            vec![chain.clone(), request.clone(), response.clone()],
            response.clone(),
        ),
        (
            "execute_async",
            vec![chain.clone(), request, response.clone()],
            response.clone(),
        ),
        ("last_trace", vec![], trace.clone()),
        ("trace_len", vec![trace.clone()], Type::Int),
        ("trace_event", vec![trace.clone(), Type::Int], Type::String),
        ("trace_short_circuited", vec![trace], Type::Bool),
    ];
    for (name, params, return_type) in functions {
        exports
            .functions
            .insert(name.to_string(), pub_fn(params, return_type));
    }

    exports.traits.insert(
        "Middleware".to_string(),
        ExportedTrait {
            visibility: ExportVisibility::Public,
            methods: [
                (
                    "on_request".to_string(),
                    exported_trait_method(
                        vec![Type::Unknown, api_type("Request")],
                        api_type("Request"),
                        Some(ExportedSelfParamKind::Reference { mutable: false }),
                        false,
                    ),
                ),
                (
                    "on_response".to_string(),
                    exported_trait_method(
                        vec![Type::Unknown, api_type("Response")],
                        api_type("Response"),
                        Some(ExportedSelfParamKind::Reference { mutable: false }),
                        false,
                    ),
                ),
            ]
            .into_iter()
            .collect(),
        },
    );
    exports.traits.insert(
        "AsyncMiddleware".to_string(),
        ExportedTrait {
            visibility: ExportVisibility::Public,
            methods: [
                (
                    "on_request".to_string(),
                    exported_trait_method(
                        vec![Type::Unknown, api_type("Request")],
                        api_task(api_type("Request")),
                        Some(ExportedSelfParamKind::Reference { mutable: false }),
                        true,
                    ),
                ),
                (
                    "on_response".to_string(),
                    exported_trait_method(
                        vec![Type::Unknown, api_type("Response")],
                        api_task(api_type("Response")),
                        Some(ExportedSelfParamKind::Reference { mutable: false }),
                        true,
                    ),
                ),
            ]
            .into_iter()
            .collect(),
        },
    );

    exports
}

fn make_std_api_cors(prefix: &str) -> ModuleExports {
    let mut exports = api_module(prefix, Some("cors"));
    exports
        .types
        .insert("CorsPolicy".to_string(), public_type(&["origin_count"]));

    let policy = api_type("CorsPolicy");
    let request = api_type("Request");
    let response = api_type("Response");
    let middleware = api_type("MiddlewareHandle");
    let functions = [
        ("policy", vec![], policy.clone()),
        ("permissive", vec![], policy.clone()),
        (
            "allow_origin",
            vec![policy.clone(), Type::String],
            policy.clone(),
        ),
        (
            "allow_method",
            vec![policy.clone(), Type::Int],
            policy.clone(),
        ),
        (
            "allow_header",
            vec![policy.clone(), Type::String],
            policy.clone(),
        ),
        (
            "expose_header",
            vec![policy.clone(), Type::String],
            policy.clone(),
        ),
        (
            "allow_credentials",
            vec![policy.clone(), Type::Bool],
            policy.clone(),
        ),
        ("max_age", vec![policy.clone(), Type::Int], policy.clone()),
        ("middleware", vec![policy.clone()], middleware),
        ("is_preflight", vec![request.clone()], Type::Bool),
        (
            "preflight",
            vec![policy.clone(), request.clone()],
            response.clone(),
        ),
        (
            "apply",
            vec![policy.clone(), request, response.clone()],
            response,
        ),
        ("allowed_origin", vec![policy, Type::String], Type::String),
    ];
    for (name, params, return_type) in functions {
        exports
            .functions
            .insert(name.to_string(), pub_fn(params, return_type));
    }

    exports
}

fn make_std_api_errors(prefix: &str) -> ModuleExports {
    let mut exports = api_module(prefix, Some("errors"));
    exports
        .types
        .insert("ApiError".to_string(), public_type(&["code", "message"]));
    let functions = [
        ("last_code", vec![], Type::Int),
        ("last_message", vec![], Type::String),
    ];
    for (name, params, return_type) in functions {
        exports
            .functions
            .insert(name.to_string(), pub_fn(params, return_type));
    }
    exports
}

fn make_std_api_trace(prefix: &str) -> ModuleExports {
    let mut exports = api_module(prefix, Some("trace"));
    let config = api_type("TraceConfig");
    let span = api_type("TraceSpan");
    exports.types.insert("TraceConfig".to_string(), public_type(&[]));
    exports.types.insert("TraceSpan".to_string(), public_type(&[]));
    for (name, params, return_type) in [
        ("config_new", vec![Type::String, Type::String], config.clone()),
        ("config_set_sample_rate", vec![config.clone(), Type::Float], Type::Bool),
        ("config_set_batch_size", vec![config.clone(), Type::Int], Type::Bool),
        ("config_start", vec![config.clone()], Type::Bool),
        ("config_shutdown", vec![config.clone()], Type::Bool),
        ("span_start", vec![Type::String, Type::Int], span.clone()),
        ("span_set_attribute", vec![span.clone(), Type::String, Type::String], Type::Bool),
        ("span_set_attribute_int", vec![span.clone(), Type::String, Type::Int], Type::Bool),
        ("span_set_attribute_bool", vec![span.clone(), Type::String, Type::Bool], Type::Bool),
        ("span_set_status", vec![span.clone(), Type::Int], Type::Bool),
        ("span_end", vec![span.clone()], Type::Bool),
        ("current", vec![], span.clone()),
        ("parent", vec![span.clone()], span.clone()),
        ("inject", vec![span.clone()], Type::Bool),
        ("extract", vec![Type::String], Type::Bool),
        ("flush", vec![], Type::Int),
        ("last_error", vec![], Type::String),
    ] { exports.functions.insert(name.to_string(), pub_fn(params, return_type)); }
    exports
}

fn make_std_api_health(prefix: &str) -> ModuleExports {
    let mut exports = api_module(prefix, Some("health"));
    exports.functions.insert("startup_complete".into(), pub_fn(vec![], Type::Bool));
    exports.functions.insert("startup_failed".into(), pub_fn(vec![Type::String], Type::Bool));
    exports
}

fn make_std_api_db_sqlite(prefix: &str) -> ModuleExports {
    let mut exports = api_module(&format!("{prefix}.db.sqlite"), None);
    let connection = api_type("SqliteConnection");
    let statement = api_type("SqliteStatement");
    exports.types.insert("SqliteConnection".to_string(), public_type(&[]));
    exports.types.insert("SqliteStatement".to_string(), public_type(&[]));
    for (name, params, return_type) in [
        ("open", vec![Type::String], connection.clone()),
        ("close", vec![connection.clone()], Type::Bool),
        ("prepare", vec![connection.clone(), Type::String], statement.clone()),
        ("execute_async", vec![connection.clone(), Type::String], Type::Int),
        ("bind_null", vec![statement.clone(), Type::Int], Type::Bool),
        ("bind_int", vec![statement.clone(), Type::Int, Type::Int], Type::Bool),
        ("bind_float", vec![statement.clone(), Type::Int, Type::Float], Type::Bool),
        ("bind_text", vec![statement.clone(), Type::Int, Type::String], Type::Bool),
        ("bind_blob", vec![statement.clone(), Type::Int, Type::String], Type::Bool),
        ("step", vec![statement.clone()], Type::Int),
        ("column_count", vec![statement.clone()], Type::Int),
        ("column_type", vec![statement.clone(), Type::Int], Type::Int),
        ("column_int", vec![statement.clone(), Type::Int], Type::Int),
        ("column_float", vec![statement.clone(), Type::Int], Type::Float),
        ("column_text", vec![statement.clone(), Type::Int], Type::String),
        ("reset", vec![statement.clone()], Type::Bool),
        ("finalize", vec![statement.clone()], Type::Bool),
        ("begin", vec![connection.clone()], Type::Bool),
        ("commit", vec![connection.clone()], Type::Bool),
        ("rollback", vec![connection.clone()], Type::Bool),
        ("last_error_code", vec![], Type::String),
        ("last_error_message", vec![], Type::String),
    ] { exports.functions.insert(name.to_string(), pub_fn(params, return_type)); }
    exports
}

fn make_std_api_db_postgres(prefix: &str) -> ModuleExports {
    let mut exports = api_module(&format!("{prefix}.db.postgres"), None);
    let connection = api_type("PostgresConnection");
    let statement = api_type("PostgresStatement");
    let notification_channel = api_type("PostgresNotificationChannel");
    let notification = api_type("PostgresNotification");
    exports.types.insert("PostgresConnection".to_string(), public_type(&[]));
    exports.types.insert("PostgresStatement".to_string(), public_type(&[]));
    exports.types.insert("PostgresNotificationChannel".to_string(), public_type(&[]));
    exports.types.insert("PostgresNotification".to_string(), public_type(&[]));
    for (name, params, return_type) in [
        ("open", vec![Type::String], connection.clone()),
        ("close", vec![connection.clone()], Type::Bool),
        ("prepare", vec![connection.clone(), Type::String], statement.clone()),
        ("bind_null", vec![statement.clone(), Type::Int], Type::Bool),
        ("bind_int", vec![statement.clone(), Type::Int, Type::Int], Type::Bool),
        ("bind_float", vec![statement.clone(), Type::Int, Type::Float], Type::Bool),
        ("bind_text", vec![statement.clone(), Type::Int, Type::String], Type::Bool),
        ("step", vec![statement.clone()], Type::Int),
        ("column_count", vec![statement.clone()], Type::Int),
        ("column_type", vec![statement.clone(), Type::Int], Type::Int),
        ("column_int", vec![statement.clone(), Type::Int], Type::Int),
        ("column_text", vec![statement.clone(), Type::Int], Type::String),
        ("reset", vec![statement.clone()], Type::Bool),
        ("finalize", vec![statement.clone()], Type::Bool),
        ("begin", vec![connection.clone()], Type::Bool),
        ("commit", vec![connection.clone()], Type::Bool),
        ("rollback", vec![connection.clone()], Type::Bool),
        ("execute_async", vec![connection.clone(), Type::String], api_task(Type::Int)),
        ("step_async", vec![statement.clone()], api_task(Type::Int)),
        ("savepoint", vec![connection.clone(), Type::String], Type::Bool),
        ("rollback_to", vec![connection.clone(), Type::String], Type::Bool),
        ("release_savepoint", vec![connection.clone(), Type::String], Type::Bool),
        ("copy_in_text_async", vec![connection.clone(), Type::String, Type::String], api_task(Type::Int)),
        ("copy_out_text_async", vec![connection.clone(), Type::String], api_task(Type::String)),
        ("listen", vec![connection.clone(), Type::String], notification_channel.clone()),
        ("notify_async", vec![connection.clone(), Type::String, Type::String], api_task(Type::Bool)),
        ("notification_next_async", vec![notification_channel.clone(), Type::Int], api_task(notification.clone())),
        ("notification_channel", vec![notification.clone()], Type::String),
        ("notification_payload", vec![notification.clone()], Type::String),
        ("notification_process_id", vec![notification.clone()], Type::Int),
        ("notification_free", vec![notification], Type::Bool),
        ("notification_close", vec![notification_channel], Type::Bool),
        ("last_error_code", vec![], Type::String),
        ("last_error_message", vec![], Type::String),
    ] { exports.functions.insert(name.to_string(), pub_fn(params, return_type)); }
    exports
}

fn make_std_api_db_redis(prefix: &str) -> ModuleExports {
    let mut exports = api_module(&format!("{prefix}.db.redis"), None);
    let connection = api_type("RedisConnection");
    exports.types.insert("RedisConnection".to_string(), public_type(&[]));
    for (name, params, return_type) in [
        ("open", vec![Type::String], connection.clone()),
        ("close", vec![connection.clone()], Type::Bool),
        ("get", vec![connection.clone(), Type::String], Type::String),
        ("set", vec![connection.clone(), Type::String, Type::String], Type::Bool),
        ("delete", vec![connection.clone(), Type::String], Type::Bool),
        ("expire", vec![connection.clone(), Type::String, Type::Int], Type::Bool),
        ("incr", vec![connection.clone(), Type::String, Type::Int], Type::Int),
        ("exists", vec![connection.clone(), Type::String], Type::Bool),
    ] { exports.functions.insert(name.to_string(), pub_fn(params, return_type)); }
    exports
}

fn make_std_io() -> ModuleExports {
    let mut exports = ModuleExports {
        stdlib_path: Some(vec!["std".to_string(), "io".to_string()]),
        package_name: Some("std".to_string()),
        ..Default::default()
    };

    // print(value: any) -> unit
    // The runtime FFI accepts a single value and prints it.
    exports
        .functions
        .insert("print".to_string(), pub_fn(vec![Type::Unknown], Type::Unit));
    // println(value: any) -> unit  (print + newline)
    exports.functions.insert(
        "println".to_string(),
        pub_fn(vec![Type::Unknown], Type::Unit),
    );
    // eprint(value: any) -> unit  (stderr, no newline)
    exports.functions.insert(
        "eprint".to_string(),
        pub_fn(vec![Type::Unknown], Type::Unit),
    );
    // eprintln(value: any) -> unit
    exports.functions.insert(
        "eprintln".to_string(),
        pub_fn(vec![Type::Unknown], Type::Unit),
    );
    // flush() -> unit
    exports
        .functions
        .insert("flush".to_string(), pub_fn(vec![], Type::Unit));
    // read_line() -> string
    exports
        .functions
        .insert("read_line".to_string(), pub_fn(vec![], Type::String));
    // input(prompt: string) -> string  (prints prompt, flushes, reads line)
    exports.functions.insert(
        "input".to_string(),
        pub_fn(vec![Type::String], Type::String),
    );

    exports
}

fn make_std_math() -> ModuleExports {
    let mut exports = ModuleExports {
        stdlib_path: Some(vec!["std".to_string(), "math".to_string()]),
        package_name: Some("std".to_string()),
        ..Default::default()
    };

    exports
        .functions
        .insert("abs".to_string(), pub_fn(vec![Type::Int], Type::Int));
    exports.functions.insert(
        "min".to_string(),
        pub_fn(vec![Type::Int, Type::Int], Type::Int),
    );
    exports.functions.insert(
        "max".to_string(),
        pub_fn(vec![Type::Int, Type::Int], Type::Int),
    );
    exports.functions.insert(
        "clamp".to_string(),
        pub_fn(vec![Type::Int, Type::Int, Type::Int], Type::Int),
    );
    exports
        .functions
        .insert("sqrt_f".to_string(), pub_fn(vec![Type::Float], Type::Float));
    exports.functions.insert(
        "pow_f".to_string(),
        pub_fn(vec![Type::Float, Type::Float], Type::Float),
    );
    exports.functions.insert(
        "floor_f".to_string(),
        pub_fn(vec![Type::Float], Type::Float),
    );
    exports
        .functions
        .insert("ceil_f".to_string(), pub_fn(vec![Type::Float], Type::Float));
    exports.functions.insert(
        "round_f".to_string(),
        pub_fn(vec![Type::Float], Type::Float),
    );
    exports
        .functions
        .insert("sin_f".to_string(), pub_fn(vec![Type::Float], Type::Float));
    exports
        .functions
        .insert("cos_f".to_string(), pub_fn(vec![Type::Float], Type::Float));
    exports
        .functions
        .insert("tan_f".to_string(), pub_fn(vec![Type::Float], Type::Float));
    exports
        .functions
        .insert("log_f".to_string(), pub_fn(vec![Type::Float], Type::Float));
    exports
        .functions
        .insert("log2_f".to_string(), pub_fn(vec![Type::Float], Type::Float));
    exports.functions.insert(
        "log10_f".to_string(),
        pub_fn(vec![Type::Float], Type::Float),
    );
    exports.functions.insert(
        "atan2_f".to_string(),
        pub_fn(vec![Type::Float, Type::Float], Type::Float),
    );
    exports
        .functions
        .insert("pi".to_string(), pub_fn(vec![], Type::Float));
    exports
        .functions
        .insert("e_const".to_string(), pub_fn(vec![], Type::Float));
    // sign(n: int) -> int — returns -1, 0, or 1
    exports
        .functions
        .insert("sign".to_string(), pub_fn(vec![Type::Int], Type::Int));
    // gcd(a: int, b: int) -> int
    exports.functions.insert(
        "gcd".to_string(),
        pub_fn(vec![Type::Int, Type::Int], Type::Int),
    );
    // lcm(a: int, b: int) -> int
    exports.functions.insert(
        "lcm".to_string(),
        pub_fn(vec![Type::Int, Type::Int], Type::Int),
    );
    // is_nan_f(x: float) -> bool
    exports.functions.insert(
        "is_nan_f".to_string(),
        pub_fn(vec![Type::Float], Type::Bool),
    );
    // is_infinite_f(x: float) -> bool
    exports.functions.insert(
        "is_infinite_f".to_string(),
        pub_fn(vec![Type::Float], Type::Bool),
    );
    // abs_f(x: float) -> float
    exports
        .functions
        .insert("abs_f".to_string(), pub_fn(vec![Type::Float], Type::Float));

    exports
}

fn make_std_numeric() -> ModuleExports {
    let mut exports = ModuleExports {
        stdlib_path: Some(vec!["std".to_string(), "numeric".to_string()]),
        package_name: Some("std".to_string()),
        ..Default::default()
    };
    for (name, ty) in [
        ("i8", Type::ExactInt { signed: true, width: IntWidth::I8 }),
        ("i16", Type::ExactInt { signed: true, width: IntWidth::I16 }),
        ("i32", Type::ExactInt { signed: true, width: IntWidth::I32 }),
        ("i64", Type::ExactInt { signed: true, width: IntWidth::I64 }),
        ("u8", Type::ExactInt { signed: false, width: IntWidth::I8 }),
        ("u16", Type::ExactInt { signed: false, width: IntWidth::I16 }),
        ("u32", Type::ExactInt { signed: false, width: IntWidth::I32 }),
        ("u64", Type::ExactInt { signed: false, width: IntWidth::I64 }),
    ] {
        for op in ["add", "sub", "mul"] {
            exports.functions.insert(
                format!("wrapping_{op}_{name}"),
                pub_fn(vec![ty.clone(), ty.clone()], ty.clone()),
            );
        }
    }
    exports.functions.insert(
        "checked_f32".to_string(),
        pub_fn(
            vec![Type::ExactFloat { width: FloatWidth::F64 }],
            Type::ExactFloat { width: FloatWidth::F32 },
        ),
    );
    exports
}

fn make_std_collections() -> ModuleExports {
    let mut exports = ModuleExports {
        stdlib_path: Some(vec!["std".to_string(), "collections".to_string()]),
        package_name: Some("std".to_string()),
        ..Default::default()
    };

    // list_new() -> int (handle)
    exports
        .functions
        .insert("list_new".to_string(), pub_fn(vec![], Type::Int));
    // list_push(handle: int, value: int) -> unit
    exports.functions.insert(
        "list_push".to_string(),
        pub_fn(vec![Type::Int, Type::Int], Type::Unit),
    );
    // list_len(handle: int) -> int
    exports
        .functions
        .insert("list_len".to_string(), pub_fn(vec![Type::Int], Type::Int));
    // list_get(handle: int, index: int) -> int  (-1 if out of bounds)
    exports.functions.insert(
        "list_get".to_string(),
        pub_fn(vec![Type::Int, Type::Int], Type::Int),
    );
    // list_set(handle: int, index: int, value: int) -> unit
    exports.functions.insert(
        "list_set".to_string(),
        pub_fn(vec![Type::Int, Type::Int, Type::Int], Type::Unit),
    );
    // list_contains(handle: int, value: int) -> bool
    exports.functions.insert(
        "list_contains".to_string(),
        pub_fn(vec![Type::Int, Type::Int], Type::Bool),
    );
    // list_clear(handle: int) -> unit
    exports.functions.insert(
        "list_clear".to_string(),
        pub_fn(vec![Type::Int], Type::Unit),
    );
    // list_free(handle: int) -> unit
    exports
        .functions
        .insert("list_free".to_string(), pub_fn(vec![Type::Int], Type::Unit));
    // list_free_all() -> int
    exports
        .functions
        .insert("list_free_all".to_string(), pub_fn(vec![], Type::Int));
    // list_pop(handle: int) -> int  (returns popped value; -1 if empty)
    exports
        .functions
        .insert("list_pop".to_string(), pub_fn(vec![Type::Int], Type::Int));
    // list_pop_front(handle: int) -> int  (returns removed front value; -1 if empty)
    exports.functions.insert(
        "list_pop_front".to_string(),
        pub_fn(vec![Type::Int], Type::Int),
    );
    // list_insert_at(handle: int, index: int, value: int) -> unit
    exports.functions.insert(
        "list_insert_at".to_string(),
        pub_fn(vec![Type::Int, Type::Int, Type::Int], Type::Unit),
    );
    // list_remove_at(handle: int, index: int) -> int  (returns removed value; -1 if out of bounds)
    exports.functions.insert(
        "list_remove_at".to_string(),
        pub_fn(vec![Type::Int, Type::Int], Type::Int),
    );
    // list_index_of(handle: int, value: int) -> int  (-1 if not found)
    exports.functions.insert(
        "list_index_of".to_string(),
        pub_fn(vec![Type::Int, Type::Int], Type::Int),
    );
    // list_sort(handle: int) -> unit  (sorts ascending in place)
    exports
        .functions
        .insert("list_sort".to_string(), pub_fn(vec![Type::Int], Type::Unit));
    let fn_int_to_int = Type::Fn {
        params: vec![Type::Int],
        return_type: Box::new(Type::Int),
    };
    let fn_int_int_to_int = Type::Fn {
        params: vec![Type::Int, Type::Int],
        return_type: Box::new(Type::Int),
    };
    // list_map(handle: int, f: fn(int)->int) -> int  (returns new list handle)
    exports.functions.insert(
        "list_map".to_string(),
        pub_fn(vec![Type::Int, fn_int_to_int.clone()], Type::Int),
    );
    // list_filter(handle: int, pred: fn(int)->int) -> int  (returns new list handle)
    exports.functions.insert(
        "list_filter".to_string(),
        pub_fn(vec![Type::Int, fn_int_to_int], Type::Int),
    );
    // list_reduce(handle: int, initial: int, f: fn(int,int)->int) -> int
    exports.functions.insert(
        "list_reduce".to_string(),
        pub_fn(
            vec![Type::Int, Type::Int, fn_int_int_to_int.clone()],
            Type::Int,
        ),
    );
    // list_sort_by(handle: int, cmp: fn(int,int)->int) -> unit
    exports.functions.insert(
        "list_sort_by".to_string(),
        pub_fn(vec![Type::Int, fn_int_int_to_int], Type::Unit),
    );

    // ── map API (R-3123: expose existing runtime HashMap<i64, i64>) ──────────
    // map_new() -> int  (returns handle; 0 on internal error)
    exports
        .functions
        .insert("map_new".to_string(), pub_fn(vec![], Type::Int));
    // map_set(handle: int, key: int, value: int) -> unit
    exports.functions.insert(
        "map_set".to_string(),
        pub_fn(vec![Type::Int, Type::Int, Type::Int], Type::Unit),
    );
    // map_get(handle: int, key: int) -> int  (0 if key absent or handle invalid)
    exports.functions.insert(
        "map_get".to_string(),
        pub_fn(vec![Type::Int, Type::Int], Type::Int),
    );
    // map_contains(handle: int, key: int) -> int  (1 if present, 0 otherwise)
    exports.functions.insert(
        "map_contains".to_string(),
        pub_fn(vec![Type::Int, Type::Int], Type::Int),
    );
    // map_remove(handle: int, key: int) -> int  (removed value, 0 if absent)
    exports.functions.insert(
        "map_remove".to_string(),
        pub_fn(vec![Type::Int, Type::Int], Type::Int),
    );
    // map_len(handle: int) -> int
    exports
        .functions
        .insert("map_len".to_string(), pub_fn(vec![Type::Int], Type::Int));
    // map_clear(handle: int) -> unit
    exports
        .functions
        .insert("map_clear".to_string(), pub_fn(vec![Type::Int], Type::Unit));
    // map_free(handle: int) -> unit
    exports
        .functions
        .insert("map_free".to_string(), pub_fn(vec![Type::Int], Type::Unit));

    // type aliases
    exports.types.insert(
        "List".to_string(),
        ExportedType {
            members: vec!["new".to_string(), "push".to_string(), "len".to_string()],
            visibility: ExportVisibility::Public,
            is_enum: false,
            struct_fields: None,
            enum_variants: None,
            enum_struct_variants: None,
        },
    );

    exports
}

fn make_std_tensor() -> ModuleExports {
    let mut exports = ModuleExports {
        stdlib_path: Some(vec!["std".to_string(), "tensor".to_string()]),
        package_name: Some("std".to_string()),
        ..Default::default()
    };

    let int = Type::Int;
    let float = Type::Float;
    let tensor_float_rank1 = Type::Tensor {
        dtype: Box::new(Type::Float),
        rank: Some(1),
        dims: None,
        layout: None,
        device: None,
    };
    let tensor_float_rank2 = Type::Tensor {
        dtype: Box::new(Type::Float),
        rank: Some(2),
        dims: None,
        layout: None,
        device: None,
    };
    let tensor_float_rank0 = Type::Tensor {
        dtype: Box::new(Type::Float),
        rank: Some(0),
        dims: None,
        layout: None,
        device: None,
    };
    let tensor_float_dynamic = Type::Tensor {
        dtype: Box::new(Type::Float),
        rank: None,
        dims: None,
        layout: None,
        device: None,
    };
    let unit = Type::Unit;
    let bool_ty = Type::Bool;

    let functions = [
        (
            "vector_f",
            vec![int.clone(), float.clone()],
            tensor_float_rank1.clone(),
        ),
        (
            "matrix_f",
            vec![int.clone(), int.clone(), float.clone()],
            tensor_float_rank2.clone(),
        ),
        ("zeros", vec![int.clone()], int.clone()),
        ("ones", vec![int.clone()], int.clone()),
        ("full", vec![int.clone(), int.clone()], int.clone()),
        (
            "full_f",
            vec![int.clone(), float.clone()],
            tensor_float_rank1.clone(),
        ),
        (
            "arange",
            vec![int.clone(), int.clone(), int.clone()],
            int.clone(),
        ),
        ("zeros2", vec![int.clone(), int.clone()], int.clone()),
        ("ones2", vec![int.clone(), int.clone()], int.clone()),
        (
            "full2",
            vec![int.clone(), int.clone(), int.clone()],
            int.clone(),
        ),
        (
            "full2_f",
            vec![int.clone(), int.clone(), float.clone()],
            tensor_float_rank2.clone(),
        ),
        ("len", vec![int.clone()], int.clone()),
        ("rank", vec![int.clone()], int.clone()),
        ("dim", vec![int.clone(), int.clone()], int.clone()),
        ("rows", vec![int.clone()], int.clone()),
        ("cols", vec![int.clone()], int.clone()),
        ("is_valid", vec![int.clone()], bool_ty.clone()),
        ("get", vec![int.clone(), int.clone()], int.clone()),
        ("get_f", vec![int.clone(), int.clone()], float.clone()),
        (
            "set",
            vec![int.clone(), int.clone(), int.clone()],
            unit.clone(),
        ),
        (
            "set_f",
            vec![int.clone(), int.clone(), float.clone()],
            unit.clone(),
        ),
        (
            "get2",
            vec![int.clone(), int.clone(), int.clone()],
            int.clone(),
        ),
        (
            "get2_f",
            vec![int.clone(), int.clone(), int.clone()],
            float.clone(),
        ),
        (
            "set2",
            vec![int.clone(), int.clone(), int.clone(), int.clone()],
            unit.clone(),
        ),
        (
            "set2_f",
            vec![int.clone(), int.clone(), int.clone(), float.clone()],
            unit.clone(),
        ),
        (
            "reshape",
            vec![int.clone(), int.clone(), int.clone()],
            int.clone(),
        ),
        ("flatten", vec![int.clone()], int.clone()),
        (
            "permute",
            vec![int.clone(), int.clone(), int.clone()],
            int.clone(),
        ),
        (
            "slice",
            vec![int.clone(), int.clone(), int.clone()],
            int.clone(),
        ),
        ("concat", vec![int.clone(), int.clone()], int.clone()),
        ("stack", vec![int.clone(), int.clone()], int.clone()),
        (
            "add",
            vec![int.clone(), int.clone()],
            tensor_float_dynamic.clone(),
        ),
        (
            "sub",
            vec![int.clone(), int.clone()],
            tensor_float_dynamic.clone(),
        ),
        (
            "mul",
            vec![int.clone(), int.clone()],
            tensor_float_dynamic.clone(),
        ),
        (
            "div",
            vec![int.clone(), int.clone()],
            tensor_float_dynamic.clone(),
        ),
        ("sum", vec![int.clone()], int.clone()),
        ("sum_f", vec![int.clone()], float.clone()),
        ("sum_t", vec![int.clone()], tensor_float_rank0.clone()),
        ("mean_f", vec![int.clone()], float.clone()),
        ("mean_t", vec![int.clone()], tensor_float_rank0.clone()),
        ("max", vec![int.clone()], int.clone()),
        ("min", vec![int.clone()], int.clone()),
        ("argmax", vec![int.clone()], int.clone()),
        (
            "matmul",
            vec![int.clone(), int.clone()],
            tensor_float_rank2.clone(),
        ),
        (
            "matmul_batched",
            vec![int.clone(), int.clone()],
            int.clone(),
        ),
        ("transpose", vec![int.clone()], tensor_float_rank2.clone()),
        ("dot", vec![int.clone(), int.clone()], int.clone()),
        (
            "dot_t",
            vec![int.clone(), int.clone()],
            tensor_float_rank0.clone(),
        ),
        ("neg", vec![int.clone()], tensor_float_dynamic.clone()),
        ("exp_f", vec![int.clone()], tensor_float_dynamic.clone()),
        ("log_f", vec![int.clone()], tensor_float_dynamic.clone()),
        ("sqrt_f", vec![int.clone()], tensor_float_dynamic.clone()),
        ("relu", vec![int.clone()], tensor_float_dynamic.clone()),
        ("sigmoid_f", vec![int.clone()], tensor_float_dynamic.clone()),
        ("tanh_f", vec![int.clone()], tensor_float_dynamic.clone()),
        ("seed", vec![int.clone()], unit.clone()),
        (
            "uniform",
            vec![int.clone(), int.clone(), int.clone()],
            int.clone(),
        ),
        (
            "uniform_f",
            vec![int.clone(), float.clone(), float.clone()],
            tensor_float_rank1.clone(),
        ),
        (
            "normal_f",
            vec![int.clone(), float.clone(), float.clone()],
            tensor_float_rank1.clone(),
        ),
        (
            "bernoulli",
            vec![int.clone(), float.clone()],
            tensor_float_rank1.clone(),
        ),
        ("categorical", vec![int.clone(), int.clone()], int.clone()),
        ("set_deterministic_mode", vec![int.clone()], int.clone()),
        ("deterministic_mode", vec![], int.clone()),
        ("tolerance_abs", vec![], float.clone()),
        ("tolerance_rel", vec![], float.clone()),
        ("device", vec![int.clone()], int.clone()),
        ("device_available", vec![int.clone()], bool_ty.clone()),
        ("device_status", vec![int.clone()], int.clone()),
        ("to_device", vec![int.clone(), int.clone()], int.clone()),
        ("cpu", vec![int.clone()], int.clone()),
        ("sync", vec![int.clone()], unit.clone()),
        ("precision", vec![int.clone()], int.clone()),
        ("to_precision", vec![int.clone(), int.clone()], int.clone()),
        ("stats_allocations", vec![], int.clone()),
        ("stats_active", vec![], int.clone()),
        ("stats_peak_bytes", vec![], int.clone()),
        ("stats_reused_buffers", vec![], int.clone()),
        ("stats_pool_hits", vec![], int.clone()),
        ("stats_pool_misses", vec![], int.clone()),
        ("stats_active_bytes", vec![], int.clone()),
        ("stats_scratch_reuses", vec![], int.clone()),
        ("kernel_strategy", vec![], int.clone()),
        ("stats_kernel_ops", vec![], int.clone()),
        ("stats_kernel_elements", vec![], int.clone()),
        ("stats_device_transfers", vec![], int.clone()),
        ("stats_gpu_kernel_ops", vec![], int.clone()),
        ("stats_cpu_fallbacks", vec![], int.clone()),
        ("stats_gpu_errors", vec![int.clone()], int.clone()),
        ("stats_device_pool_hits", vec![], int.clone()),
        ("stats_device_pool_misses", vec![], int.clone()),
        ("stats_device_pool_bytes_resident", vec![], int.clone()),
        ("storage_device", vec![int.clone()], int.clone()),
        ("stats_device_resident_tensors", vec![], int.clone()),
        ("stats_gpu_backward_ops", vec![], int.clone()),
        ("stats_graph_nodes", vec![], int.clone()),
        ("stats_lifetime_records", vec![], int.clone()),
        ("stats_released_lifetimes", vec![], int.clone()),
        ("stats_allocation_sites", vec![], int.clone()),
        ("stats_reuse_rate_per_mille", vec![], int.clone()),
        ("memory_report", vec![], Type::String),
        ("reset_stats", vec![], unit.clone()),
        (
            "requires_grad",
            vec![int.clone(), bool_ty.clone()],
            tensor_float_dynamic.clone(),
        ),
        ("diff", vec![tensor_float_rank0.clone()], unit.clone()),
        ("backward", vec![tensor_float_rank0.clone()], unit.clone()),
        ("grad", vec![int.clone()], tensor_float_dynamic.clone()),
        ("zero_grad", vec![int.clone()], unit.clone()),
        ("set_grad_enabled", vec![bool_ty.clone()], unit.clone()),
        ("grad_enabled", vec![], bool_ty.clone()),
        ("free", vec![int.clone()], unit.clone()),
        ("free_all", vec![], int.clone()),
        ("refill", vec![int.clone(), float.clone()], unit.clone()),
    ];

    for (name, params, return_type) in functions {
        exports
            .functions
            .insert(name.to_string(), pub_fn(params, return_type));
    }

    exports.types.insert(
        "Tensor".to_string(),
        ExportedType {
            members: vec![
                "shape".to_string(),
                "dtype".to_string(),
                "device".to_string(),
                "precision".to_string(),
                "layout".to_string(),
            ],
            visibility: ExportVisibility::Public,
            is_enum: false,
            struct_fields: None,
            enum_variants: None,
            enum_struct_variants: None,
        },
    );

    exports
}

fn make_std_ml() -> ModuleExports {
    let mut exports = ModuleExports {
        stdlib_path: Some(vec!["std".to_string(), "ml".to_string()]),
        package_name: Some("std".to_string()),
        ..Default::default()
    };

    let int = Type::Int;
    let float = Type::Float;
    let unit = Type::Unit;
    let bool_ty = Type::Bool;
    let tensor_float_rank0 = Type::Tensor {
        dtype: Box::new(Type::Float),
        rank: Some(0),
        dims: None,
        layout: None,
        device: None,
    };
    let tensor_float_rank2 = Type::Tensor {
        dtype: Box::new(Type::Float),
        rank: Some(2),
        dims: None,
        layout: None,
        device: None,
    };

    let functions = [
        ("module_new", vec![], int.clone()),
        (
            "module_add_parameter",
            vec![int.clone(), int.clone()],
            unit.clone(),
        ),
        ("module_parameter_count", vec![int.clone()], int.clone()),
        (
            "module_parameter",
            vec![int.clone(), int.clone()],
            int.clone(),
        ),
        (
            "module_set_training",
            vec![int.clone(), bool_ty.clone()],
            unit.clone(),
        ),
        ("module_is_training", vec![int.clone()], bool_ty.clone()),
        (
            "linear",
            vec![int.clone(), int.clone(), int.clone()],
            tensor_float_rank2.clone(),
        ),
        (
            "conv2d",
            vec![
                int.clone(),
                int.clone(),
                int.clone(),
                int.clone(),
                int.clone(),
                int.clone(),
                int.clone(),
                int.clone(),
                int.clone(),
                int.clone(),
            ],
            int.clone(),
        ),
        (
            "dropout",
            vec![int.clone(), float.clone(), bool_ty.clone()],
            int.clone(),
        ),
        (
            "max_pool2d",
            vec![
                int.clone(),
                int.clone(),
                int.clone(),
                int.clone(),
                int.clone(),
                int.clone(),
                int.clone(),
            ],
            int.clone(),
        ),
        (
            "mse_loss",
            vec![int.clone(), int.clone()],
            tensor_float_rank0.clone(),
        ),
        (
            "bce_loss",
            vec![int.clone(), int.clone()],
            tensor_float_rank0.clone(),
        ),
        (
            "cross_entropy_loss",
            vec![int.clone(), int.clone()],
            tensor_float_rank0.clone(),
        ),
        (
            "nll_loss",
            vec![int.clone(), int.clone()],
            tensor_float_rank0.clone(),
        ),
        ("sgd_step", vec![int.clone(), float.clone()], unit.clone()),
        (
            "sgd_momentum_step",
            vec![int.clone(), int.clone(), float.clone(), float.clone()],
            unit.clone(),
        ),
        (
            "adam_step",
            vec![
                int.clone(),
                int.clone(),
                int.clone(),
                float.clone(),
                float.clone(),
                float.clone(),
                float.clone(),
                int.clone(),
            ],
            unit.clone(),
        ),
        (
            "adamw_step",
            vec![
                int.clone(),
                int.clone(),
                int.clone(),
                float.clone(),
                float.clone(),
                float.clone(),
                float.clone(),
                int.clone(),
                float.clone(),
            ],
            unit.clone(),
        ),
        (
            "exp_lr",
            vec![float.clone(), float.clone(), int.clone()],
            float.clone(),
        ),
        (
            "unscale_grad",
            vec![int.clone(), float.clone()],
            unit.clone(),
        ),
        (
            "dataset_from_tensors",
            vec![int.clone(), int.clone(), int.clone()],
            int.clone(),
        ),
        (
            "dataset_from_csv",
            vec![Type::String, int.clone(), int.clone()],
            int.clone(),
        ),
        ("dataset_from_jsonl", vec![Type::String], int.clone()),
        (
            "dataset_from_npy",
            vec![Type::String, Type::String, int.clone()],
            int.clone(),
        ),
        ("dataset_from_directory", vec![Type::String], int.clone()),
        ("dataset_len", vec![int.clone()], int.clone()),
        (
            "dataset_map_features",
            vec![int.clone(), float.clone(), float.clone()],
            int.clone(),
        ),
        (
            "dataset_filter_label_min",
            vec![int.clone(), float.clone()],
            int.clone(),
        ),
        (
            "dataset_train_split",
            vec![int.clone(), int.clone()],
            int.clone(),
        ),
        (
            "dataset_test_split",
            vec![int.clone(), int.clone()],
            int.clone(),
        ),
        (
            "dataloader_new",
            vec![int.clone(), int.clone(), int.clone()],
            int.clone(),
        ),
        ("dataloader_batch_count", vec![int.clone()], int.clone()),
        (
            "dataloader_batch_features",
            vec![int.clone(), int.clone()],
            int.clone(),
        ),
        (
            "dataloader_batch_labels",
            vec![int.clone(), int.clone()],
            int.clone(),
        ),
        (
            "dataframe_from_csv",
            vec![Type::String, int.clone()],
            int.clone(),
        ),
        ("dataframe_rows", vec![int.clone()], int.clone()),
        ("dataframe_cols", vec![int.clone()], int.clone()),
        (
            "dataframe_column",
            vec![int.clone(), int.clone()],
            int.clone(),
        ),
        (
            "experiment_start",
            vec![Type::String, Type::String, int.clone()],
            int.clone(),
        ),
        (
            "experiment_set_config",
            vec![int.clone(), Type::String, Type::String],
            unit.clone(),
        ),
        (
            "experiment_log_metric",
            vec![int.clone(), Type::String, float.clone(), int.clone()],
            unit.clone(),
        ),
        (
            "experiment_log_artifact",
            vec![int.clone(), Type::String],
            unit.clone(),
        ),
        (
            "experiment_set_lockfile",
            vec![int.clone(), Type::String],
            unit.clone(),
        ),
        (
            "experiment_set_model_output",
            vec![int.clone(), Type::String],
            unit.clone(),
        ),
        ("experiment_finish", vec![int.clone()], unit.clone()),
        ("experiment_manifest_path", vec![int.clone()], Type::String),
        ("experiment_repro_command", vec![int.clone()], Type::String),
        (
            "experiment_compare_manifests",
            vec![Type::String, Type::String],
            int.clone(),
        ),
        (
            "distributed_session_start",
            vec![Type::String, Type::String, int.clone(), int.clone()],
            int.clone(),
        ),
        (
            "distributed_worker_step",
            vec![int.clone(), int.clone(), int.clone(), float.clone()],
            int.clone(),
        ),
        ("distributed_global_step", vec![int.clone()], int.clone()),
        (
            "distributed_worker_step_count",
            vec![int.clone(), int.clone()],
            int.clone(),
        ),
        (
            "distributed_checkpoint_save",
            vec![int.clone(), Type::String, int.clone()],
            Type::String,
        ),
        ("distributed_resume", vec![Type::String], int.clone()),
        ("distributed_summary", vec![int.clone()], Type::String),
        (
            "onnx_export",
            vec![Type::String, Type::String],
            Type::String,
        ),
        ("onnx_import_summary", vec![Type::String], Type::String),
        ("onnx_validate", vec![Type::String], int.clone()),
        (
            "onnx_roundtrip",
            vec![Type::String, Type::String],
            Type::String,
        ),
        (
            "embedding_lookup",
            vec![int.clone(), int.clone()],
            int.clone(),
        ),
        (
            "positional_encoding",
            vec![int.clone(), int.clone()],
            int.clone(),
        ),
        (
            "layer_norm",
            vec![int.clone(), int.clone(), int.clone(), float.clone()],
            int.clone(),
        ),
        ("gelu", vec![int.clone()], int.clone()),
        ("swiglu", vec![int.clone(), int.clone()], int.clone()),
        (
            "attention",
            vec![int.clone(), int.clone(), int.clone()],
            int.clone(),
        ),
        ("kv_cache_new", vec![int.clone(), int.clone()], int.clone()),
        (
            "kv_cache_append",
            vec![int.clone(), int.clone(), int.clone()],
            int.clone(),
        ),
        ("kv_cache_keys", vec![int.clone()], int.clone()),
        ("kv_cache_values", vec![int.clone()], int.clone()),
        ("kv_cache_len", vec![int.clone()], int.clone()),
        (
            "logits_sample",
            vec![int.clone(), float.clone()],
            int.clone(),
        ),
        ("tokenizer_wordpiece", vec![Type::String], int.clone()),
        ("tokenizer_load", vec![Type::String], int.clone()),
        (
            "tokenizer_encode",
            vec![int.clone(), Type::String],
            int.clone(),
        ),
        (
            "tokenizer_decode",
            vec![int.clone(), int.clone()],
            Type::String,
        ),
        ("text_embed", vec![Type::String, int.clone()], int.clone()),
        ("embedding_load", vec![Type::String, Type::String], int.clone()),
        ("vector_index_new", vec![int.clone()], int.clone()),
        (
            "vector_index_insert",
            vec![int.clone(), Type::String, int.clone()],
            int.clone(),
        ),
        (
            "vector_index_query",
            vec![int.clone(), int.clone(), int.clone()],
            Type::String,
        ),
        (
            "vector_index_persist",
            vec![int.clone(), Type::String],
            Type::String,
        ),
        ("vector_index_load", vec![Type::String], int.clone()),
        (
            "vector_index_set_metadata",
            vec![int.clone(), Type::String, Type::String],
            Type::Bool,
        ),
        ("vector_index_metrics", vec![int.clone()], Type::String),
        (
            "rag_chunk_text",
            vec![Type::String, int.clone(), int.clone()],
            Type::String,
        ),
        (
            "rag_build_prompt",
            vec![Type::String, Type::String],
            Type::String,
        ),
        (
            "rag_evaluate_answer",
            vec![Type::String, Type::String],
            int.clone(),
        ),
        (
            "metrics_classification",
            vec![int.clone(), int.clone()],
            Type::String,
        ),
        (
            "metrics_regression",
            vec![int.clone(), int.clone()],
            Type::String,
        ),
        (
            "metrics_ranking",
            vec![int.clone(), int.clone(), int.clone()],
            Type::String,
        ),
        (
            "metrics_generation",
            vec![Type::String, Type::String],
            Type::String,
        ),
        (
            "serving_metrics",
            vec![int.clone(), int.clone(), int.clone()],
            Type::String,
        ),
        (
            "evaluation_report",
            vec![
                Type::String,
                Type::String,
                Type::String,
                Type::String,
                Type::String,
                Type::String,
                Type::String,
            ],
            Type::String,
        ),
        (
            "artifact_new",
            vec![Type::String, Type::String, Type::String],
            int.clone(),
        ),
        (
            "artifact_set_metadata",
            vec![int.clone(), Type::String, Type::String],
            bool_ty.clone(),
        ),
        (
            "artifact_add_tensor",
            vec![int.clone(), Type::String, int.clone()],
            bool_ty.clone(),
        ),
        ("artifact_save", vec![int.clone(), Type::String], bool_ty.clone()),
        ("artifact_load", vec![Type::String], int.clone()),
        ("artifact_tensor", vec![int.clone(), Type::String], int.clone()),
        ("artifact_metadata", vec![int.clone(), Type::String], Type::String),
        ("artifact_validate", vec![Type::String], bool_ty.clone()),
        ("artifact_free", vec![int.clone()], unit.clone()),
    ];

    for (name, params, return_type) in functions {
        exports
            .functions
            .insert(name.to_string(), pub_fn(params, return_type));
    }

    exports.types.insert(
        "Module".to_string(),
        ExportedType {
            members: vec!["parameters".to_string(), "training".to_string()],
            visibility: ExportVisibility::Public,
            is_enum: false,
            struct_fields: None,
            enum_variants: None,
            enum_struct_variants: None,
        },
    );

    exports
}

fn make_std_concurrent() -> ModuleExports {
    let mut exports = ModuleExports {
        stdlib_path: Some(vec!["std".to_string(), "concurrent".to_string()]),
        package_name: Some("std".to_string()),
        ..Default::default()
    };

    let int = Type::Int;
    let bool_ty = Type::Bool;
    let unit = Type::Unit;

    let functions = [
        ("task_spawn", vec![int.clone()], int.clone()),
        ("task_join", vec![int.clone()], int.clone()),
        (
            "task_spawn_batch",
            vec![int.clone(), int.clone()],
            int.clone(),
        ),
        ("task_join_batch_sum", vec![int.clone()], int.clone()),
        ("task_is_done", vec![int.clone()], bool_ty.clone()),
        ("channel_new", vec![], int.clone()),
        (
            "channel_send",
            vec![int.clone(), int.clone()],
            bool_ty.clone(),
        ),
        ("channel_recv", vec![int.clone()], int.clone()),
        ("channel_len", vec![int.clone()], int.clone()),
        ("channel_close", vec![int.clone()], unit.clone()),
        ("counter_new", vec![int.clone()], int.clone()),
        ("counter_add", vec![int.clone(), int.clone()], int.clone()),
        ("counter_get", vec![int.clone()], int.clone()),
        (
            "pipeline_sum",
            vec![int.clone(), int.clone(), int.clone()],
            int.clone(),
        ),
        ("stats_tasks_spawned", vec![], int.clone()),
        ("stats_channels", vec![], int.clone()),
        ("reset", vec![], unit.clone()),
    ];

    for (name, params, return_type) in functions {
        exports
            .functions
            .insert(name.to_string(), pub_fn(params, return_type));
    }

    exports
}

fn make_std_serve() -> ModuleExports {
    let mut exports = ModuleExports {
        stdlib_path: Some(vec!["std".to_string(), "serve".to_string()]),
        package_name: Some("std".to_string()),
        ..Default::default()
    };

    let int = Type::Int;
    let bool_ty = Type::Bool;

    let functions = [
        ("server_new", vec![int.clone()], int.clone()),
        ("server_warmup", vec![int.clone()], bool_ty.clone()),
        ("server_is_warm", vec![int.clone()], bool_ty.clone()),
        (
            "server_enqueue",
            vec![int.clone(), int.clone()],
            int.clone(),
        ),
        (
            "server_cancel",
            vec![int.clone(), int.clone()],
            bool_ty.clone(),
        ),
        (
            "server_process_batch",
            vec![int.clone(), int.clone()],
            int.clone(),
        ),
        ("server_result", vec![int.clone(), int.clone()], int.clone()),
        ("server_pending", vec![int.clone()], int.clone()),
        (
            "server_set_timeout",
            vec![int.clone(), int.clone()],
            bool_ty.clone(),
        ),
        ("server_resident_model", vec![int.clone()], int.clone()),
        (
            "server_benchmark",
            vec![int.clone(), int.clone(), int.clone()],
            int.clone(),
        ),
        (
            "server_set_input_policy",
            vec![int.clone(), int.clone(), int.clone()],
            bool_ty.clone(),
        ),
        (
            "server_set_output_policy",
            vec![int.clone(), int.clone(), int.clone()],
            bool_ty.clone(),
        ),
        (
            "server_set_rate_limit",
            vec![int.clone(), int.clone()],
            bool_ty.clone(),
        ),
        (
            "server_set_fallback",
            vec![int.clone(), int.clone()],
            bool_ty.clone(),
        ),
        ("server_last_diagnostic", vec![int.clone()], Type::String),
        ("server_audit_log", vec![int.clone()], Type::String),
        (
            "server_set_model_version",
            vec![int.clone(), Type::String],
            bool_ty.clone(),
        ),
        (
            "server_monitoring_snapshot",
            vec![int.clone()],
            Type::String,
        ),
        (
            "server_distribution_summary",
            vec![int.clone()],
            Type::String,
        ),
        (
            "drift_check",
            vec![Type::String, Type::String, int.clone()],
            Type::String,
        ),
        (
            "export_monitoring",
            vec![
                int.clone(),
                Type::String,
                Type::String,
                Type::String,
                Type::String,
            ],
            Type::String,
        ),
        ("reset", vec![], Type::Unit),
    ];

    for (name, params, return_type) in functions {
        exports
            .functions
            .insert(name.to_string(), pub_fn(params, return_type));
    }

    exports
}

fn make_std_string() -> ModuleExports {
    let mut exports = ModuleExports {
        stdlib_path: Some(vec!["std".to_string(), "string".to_string()]),
        package_name: Some("std".to_string()),
        ..Default::default()
    };

    // len(s: string) -> int — number of characters (bytes for ASCII content)
    exports
        .functions
        .insert("len".to_string(), pub_fn(vec![Type::String], Type::Int));
    // contains(s: string, sub: string) -> bool
    exports.functions.insert(
        "contains".to_string(),
        pub_fn(vec![Type::String, Type::String], Type::Bool),
    );
    // to_upper(s: string) -> string
    exports.functions.insert(
        "to_upper".to_string(),
        pub_fn(vec![Type::String], Type::String),
    );
    // to_lower(s: string) -> string
    exports.functions.insert(
        "to_lower".to_string(),
        pub_fn(vec![Type::String], Type::String),
    );
    // trim(s: string) -> string
    exports
        .functions
        .insert("trim".to_string(), pub_fn(vec![Type::String], Type::String));
    // starts_with(s: string, prefix: string) -> bool
    exports.functions.insert(
        "starts_with".to_string(),
        pub_fn(vec![Type::String, Type::String], Type::Bool),
    );
    // ends_with(s: string, suffix: string) -> bool
    exports.functions.insert(
        "ends_with".to_string(),
        pub_fn(vec![Type::String, Type::String], Type::Bool),
    );
    // eq(a: string, b: string) -> bool
    exports.functions.insert(
        "eq".to_string(),
        pub_fn(vec![Type::String, Type::String], Type::Bool),
    );
    // concat(a: string, b: string) -> string
    exports.functions.insert(
        "concat".to_string(),
        pub_fn(vec![Type::String, Type::String], Type::String),
    );
    // repeat_str(s: string, n: int) -> string
    exports.functions.insert(
        "repeat_str".to_string(),
        pub_fn(vec![Type::String, Type::Int], Type::String),
    );
    // String builder API (R-3108). Avoids the per-call allocation cost of
    // `concat` by accumulating parts in a handle and joining them in a
    // single allocation on `builder_finish`. The constructor takes a
    // capacity hint (in bytes) so it is not parsed as a no-arg method
    // call by the parser.
    exports.functions.insert(
        "builder_new".to_string(),
        pub_fn(vec![Type::Int], Type::Int),
    );
    exports.functions.insert(
        "builder_push".to_string(),
        pub_fn(vec![Type::Int, Type::String], Type::Unit),
    );
    exports.functions.insert(
        "builder_len".to_string(),
        pub_fn(vec![Type::Int], Type::Int),
    );
    exports.functions.insert(
        "builder_finish".to_string(),
        pub_fn(vec![Type::Int], Type::String),
    );
    exports.functions.insert(
        "builder_free".to_string(),
        pub_fn(vec![Type::Int], Type::Unit),
    );
    // concat_n(list: int, count: int) -> string
    // Concatenates the first `count` string elements of a std.collections
    // list (each stored as a string handle) into a single fresh allocation.
    // Low-level building block for R-3108; the user-facing string builder
    // API is added by a follow-up.
    // Currently not exposed at the language level because list_push takes
    // int and the encoding of a string handle is not user-facing. This entry
    // is left commented until the builder API lands.
    // exports.functions.insert(
    //     "concat_n".to_string(),
    //     pub_fn(vec![Type::Int, Type::Int], Type::String),
    // );
    // char_at(s: string, index: int) -> int  (returns char code; -1 if out of bounds)
    exports.functions.insert(
        "char_at".to_string(),
        pub_fn(vec![Type::String, Type::Int], Type::Int),
    );
    // substring(s: string, start: int, end: int) -> string
    exports.functions.insert(
        "substring".to_string(),
        pub_fn(vec![Type::String, Type::Int, Type::Int], Type::String),
    );
    // replace(s: string, from: string, to: string) -> string
    exports.functions.insert(
        "replace".to_string(),
        pub_fn(vec![Type::String, Type::String, Type::String], Type::String),
    );
    // index_of(s: string, sub: string) -> int  (-1 if not found)
    exports.functions.insert(
        "index_of".to_string(),
        pub_fn(vec![Type::String, Type::String], Type::Int),
    );
    // split_first(s: string, sep: string) -> string
    exports.functions.insert(
        "split_first".to_string(),
        pub_fn(vec![Type::String, Type::String], Type::String),
    );
    // split_last(s: string, sep: string) -> string
    exports.functions.insert(
        "split_last".to_string(),
        pub_fn(vec![Type::String, Type::String], Type::String),
    );
    // is_empty(s: string) -> bool
    exports.functions.insert(
        "is_empty".to_string(),
        pub_fn(vec![Type::String], Type::Bool),
    );
    // count_occurrences(s: string, sub: string) -> int
    exports.functions.insert(
        "count_occurrences".to_string(),
        pub_fn(vec![Type::String, Type::String], Type::Int),
    );
    // split_by(s: string, sep: string) -> int  (returns list handle; each element is a string pointer)
    exports.functions.insert(
        "split_by".to_string(),
        pub_fn(vec![Type::String, Type::String], Type::Int),
    );
    // pad_left(s: string, width: int, pad_char: int) -> string
    exports.functions.insert(
        "pad_left".to_string(),
        pub_fn(vec![Type::String, Type::Int, Type::Int], Type::String),
    );
    // pad_right(s: string, width: int, pad_char: int) -> string
    exports.functions.insert(
        "pad_right".to_string(),
        pub_fn(vec![Type::String, Type::Int, Type::Int], Type::String),
    );
    // reverse_str(s: string) -> string
    exports.functions.insert(
        "reverse_str".to_string(),
        pub_fn(vec![Type::String], Type::String),
    );

    exports
}

fn make_std_convert() -> ModuleExports {
    let mut exports = ModuleExports {
        stdlib_path: Some(vec!["std".to_string(), "convert".to_string()]),
        package_name: Some("std".to_string()),
        ..Default::default()
    };

    // to_string(val: int) -> string  (also accepts float via Unknown type)
    exports.functions.insert(
        "int_to_string".to_string(),
        pub_fn(vec![Type::Int], Type::String),
    );
    // float_to_string(val: float) -> string
    exports.functions.insert(
        "float_to_string".to_string(),
        pub_fn(vec![Type::Float], Type::String),
    );
    // bool_to_string(val: bool) -> string
    exports.functions.insert(
        "bool_to_string".to_string(),
        pub_fn(vec![Type::Bool], Type::String),
    );
    // string_to_int(s: string) -> int  (returns 0 on parse error)
    exports.functions.insert(
        "string_to_int".to_string(),
        pub_fn(vec![Type::String], Type::Int),
    );
    // string_to_float(s: string) -> float  (returns 0.0 on parse error)
    exports.functions.insert(
        "string_to_float".to_string(),
        pub_fn(vec![Type::String], Type::Float),
    );
    // int_to_float(val: int) -> float
    exports.functions.insert(
        "int_to_float".to_string(),
        pub_fn(vec![Type::Int], Type::Float),
    );
    // float_to_int(val: float) -> int  (truncates)
    exports.functions.insert(
        "float_to_int".to_string(),
        pub_fn(vec![Type::Float], Type::Int),
    );
    // string_to_int_or(s: string, default: int) -> int
    exports.functions.insert(
        "string_to_int_or".to_string(),
        pub_fn(vec![Type::String, Type::Int], Type::Int),
    );
    // string_to_float_or(s: string, default: float) -> float
    exports.functions.insert(
        "string_to_float_or".to_string(),
        pub_fn(vec![Type::String, Type::Float], Type::Float),
    );
    // string_to_bool(s: string) -> bool
    exports.functions.insert(
        "string_to_bool".to_string(),
        pub_fn(vec![Type::String], Type::Bool),
    );
    // bool_to_int(b: bool) -> int
    exports.functions.insert(
        "bool_to_int".to_string(),
        pub_fn(vec![Type::Bool], Type::Int),
    );

    exports
}

fn make_std_random() -> ModuleExports {
    let mut exports = ModuleExports {
        stdlib_path: Some(vec!["std".to_string(), "random".to_string()]),
        package_name: Some("std".to_string()),
        ..Default::default()
    };

    // random_seed(seed: int) -> unit
    exports.functions.insert(
        "random_seed".to_string(),
        pub_fn(vec![Type::Int], Type::Unit),
    );
    // random_int(min: int, max: int) -> int
    exports.functions.insert(
        "random_int".to_string(),
        pub_fn(vec![Type::Int, Type::Int], Type::Int),
    );
    // random_float() -> float  ([0.0, 1.0))
    exports
        .functions
        .insert("random_float".to_string(), pub_fn(vec![], Type::Float));
    // random_bool() -> bool
    exports
        .functions
        .insert("random_bool".to_string(), pub_fn(vec![], Type::Bool));

    exports
}

fn make_std_fs() -> ModuleExports {
    let mut exports = ModuleExports {
        stdlib_path: Some(vec!["std".to_string(), "fs".to_string()]),
        package_name: Some("std".to_string()),
        ..Default::default()
    };

    // fs_read(path: string) returns string  (reads entire file; returns "" on error)
    exports.functions.insert(
        "fs_read".to_string(),
        pub_fn(vec![Type::String], Type::String),
    );
    // fs_write(path: string, content: string) -> bool
    // Creates missing parent directories when possible; returns false on controlled filesystem failures.
    exports.functions.insert(
        "fs_write".to_string(),
        pub_fn(vec![Type::String, Type::String], Type::Bool),
    );
    // fs_append(path: string, content: string) -> bool
    // Creates missing parent directories when possible; returns false on controlled filesystem failures.
    exports.functions.insert(
        "fs_append".to_string(),
        pub_fn(vec![Type::String, Type::String], Type::Bool),
    );
    // fs_exists(path: string) -> bool
    exports.functions.insert(
        "fs_exists".to_string(),
        pub_fn(vec![Type::String], Type::Bool),
    );
    // fs_remove(path: string) -> bool
    exports.functions.insert(
        "fs_remove".to_string(),
        pub_fn(vec![Type::String], Type::Bool),
    );

    exports
}

fn make_std_env() -> ModuleExports {
    let mut exports = ModuleExports {
        stdlib_path: Some(vec!["std".to_string(), "env".to_string()]),
        package_name: Some("std".to_string()),
        ..Default::default()
    };

    // env_get(key: string) returns string  (returns "" if not set)
    exports.functions.insert(
        "env_get".to_string(),
        pub_fn(vec![Type::String], Type::String),
    );
    // env_set(key: string, value: string) -> bool
    exports.functions.insert(
        "env_set".to_string(),
        pub_fn(vec![Type::String, Type::String], Type::Bool),
    );
    // env_args_count() -> int
    exports
        .functions
        .insert("env_args_count".to_string(), pub_fn(vec![], Type::Int));
    // env_arg(index: int) returns string  (returns "" if out of bounds)
    exports
        .functions
        .insert("env_arg".to_string(), pub_fn(vec![Type::Int], Type::String));

    exports
}

fn make_std_option() -> ModuleExports {
    let mut exports = ModuleExports {
        stdlib_path: Some(vec!["std".to_string(), "option".to_string()]),
        package_name: Some("std".to_string()),
        ..Default::default()
    };

    // is_some(opt: unknown) -> bool
    exports.functions.insert(
        "is_some".to_string(),
        pub_fn(vec![Type::Unknown], Type::Bool),
    );
    // is_none(opt: unknown) -> bool
    exports.functions.insert(
        "is_none".to_string(),
        pub_fn(vec![Type::Unknown], Type::Bool),
    );
    // option_unwrap(opt: unknown) -> unknown  (runtime error on None)
    exports.functions.insert(
        "option_unwrap".to_string(),
        pub_fn(vec![Type::Unknown], Type::Unknown),
    );
    // option_unwrap_or(opt: unknown, default: unknown) -> unknown
    exports.functions.insert(
        "option_unwrap_or".to_string(),
        pub_fn(vec![Type::Unknown, Type::Unknown], Type::Unknown),
    );

    exports
}

fn make_std_result() -> ModuleExports {
    let mut exports = ModuleExports {
        stdlib_path: Some(vec!["std".to_string(), "result".to_string()]),
        package_name: Some("std".to_string()),
        ..Default::default()
    };

    // is_ok(res: unknown) -> bool
    exports
        .functions
        .insert("is_ok".to_string(), pub_fn(vec![Type::Unknown], Type::Bool));
    // is_err(res: unknown) -> bool
    exports.functions.insert(
        "is_err".to_string(),
        pub_fn(vec![Type::Unknown], Type::Bool),
    );
    // result_unwrap(res: unknown) -> unknown  (runtime error on Err)
    exports.functions.insert(
        "result_unwrap".to_string(),
        pub_fn(vec![Type::Unknown], Type::Unknown),
    );
    // result_unwrap_or(res: unknown, default: unknown) -> unknown
    exports.functions.insert(
        "result_unwrap_or".to_string(),
        pub_fn(vec![Type::Unknown, Type::Unknown], Type::Unknown),
    );
    // result_unwrap_err(res: unknown) -> unknown  (runtime error on Ok)
    exports.functions.insert(
        "result_unwrap_err".to_string(),
        pub_fn(vec![Type::Unknown], Type::Unknown),
    );

    exports
}

fn make_std_char() -> ModuleExports {
    let mut exports = ModuleExports {
        stdlib_path: Some(vec!["std".to_string(), "char".to_string()]),
        package_name: Some("std".to_string()),
        ..Default::default()
    };

    // All functions take an int (Unicode code point) and return bool or int.
    // is_alpha(c: int) -> bool
    exports
        .functions
        .insert("is_alpha".to_string(), pub_fn(vec![Type::Int], Type::Bool));
    // is_digit_char(c: int) -> bool
    exports.functions.insert(
        "is_digit_char".to_string(),
        pub_fn(vec![Type::Int], Type::Bool),
    );
    // is_whitespace_char(c: int) -> bool
    exports.functions.insert(
        "is_whitespace_char".to_string(),
        pub_fn(vec![Type::Int], Type::Bool),
    );
    // is_upper_char(c: int) -> bool
    exports.functions.insert(
        "is_upper_char".to_string(),
        pub_fn(vec![Type::Int], Type::Bool),
    );
    // is_lower_char(c: int) -> bool
    exports.functions.insert(
        "is_lower_char".to_string(),
        pub_fn(vec![Type::Int], Type::Bool),
    );
    // to_upper_char(c: int) -> int  (returns uppercased code point)
    exports.functions.insert(
        "to_upper_char".to_string(),
        pub_fn(vec![Type::Int], Type::Int),
    );
    // to_lower_char(c: int) -> int  (returns lowercased code point)
    exports.functions.insert(
        "to_lower_char".to_string(),
        pub_fn(vec![Type::Int], Type::Int),
    );
    // is_alphanumeric(c: int) -> bool
    exports.functions.insert(
        "is_alphanumeric".to_string(),
        pub_fn(vec![Type::Int], Type::Bool),
    );

    exports
}

fn make_std_time() -> ModuleExports {
    let mut exports = ModuleExports {
        stdlib_path: Some(vec!["std".to_string(), "time".to_string()]),
        package_name: Some("std".to_string()),
        ..Default::default()
    };

    for name in ["Duration", "Instant", "UtcDateTime"] {
        exports.types.insert(name.to_string(), public_type(&[]));
    }

    let duration = Type::Struct {
        name: "Duration".to_string(),
    };
    let instant = Type::Struct {
        name: "Instant".to_string(),
    };
    let utc = Type::Struct {
        name: "UtcDateTime".to_string(),
    };

    // time_now_millis() -> int  (milliseconds since Unix epoch; -1 on error)
    exports
        .functions
        .insert("time_now_millis".to_string(), pub_fn(vec![], Type::Int));
    // time_now_secs() -> int  (seconds since Unix epoch; -1 on error)
    exports
        .functions
        .insert("time_now_secs".to_string(), pub_fn(vec![], Type::Int));
    // sleep_ms(ms: int) -> unit  (sleeps for ms milliseconds)
    exports
        .functions
        .insert("sleep_ms".to_string(), pub_fn(vec![Type::Int], Type::Unit));
    exports
        .functions
        .insert("monotonic_millis".to_string(), pub_fn(vec![], Type::Int));
    exports
        .functions
        .insert("monotonic_nanos".to_string(), pub_fn(vec![], Type::Int));
    exports.functions.insert(
        "duration_ms".to_string(),
        pub_fn(vec![Type::Int], duration.clone()),
    );
    exports.functions.insert(
        "duration_secs".to_string(),
        pub_fn(vec![Type::Int], duration.clone()),
    );
    exports.functions.insert(
        "duration_millis".to_string(),
        pub_fn(vec![duration.clone()], Type::Int),
    );
    exports.functions.insert(
        "duration_secs_value".to_string(),
        pub_fn(vec![duration.clone()], Type::Int),
    );
    exports.functions.insert(
        "duration_add".to_string(),
        pub_fn(vec![duration.clone(), duration.clone()], duration.clone()),
    );
    exports.functions.insert(
        "duration_sub".to_string(),
        pub_fn(vec![duration.clone(), duration.clone()], duration.clone()),
    );
    exports
        .functions
        .insert("instant_now".to_string(), pub_fn(vec![], instant.clone()));
    exports.functions.insert(
        "instant_elapsed_ms".to_string(),
        pub_fn(vec![instant.clone()], Type::Int),
    );
    exports.functions.insert(
        "instant_add".to_string(),
        pub_fn(vec![instant.clone(), duration.clone()], instant.clone()),
    );
    exports.functions.insert(
        "instant_has_elapsed".to_string(),
        pub_fn(vec![instant.clone()], Type::Bool),
    );
    exports
        .functions
        .insert("sleep".to_string(), pub_fn(vec![duration], Type::Unit));
    exports.functions.insert(
        "unix_to_utc".to_string(),
        pub_fn(vec![Type::Int], utc.clone()),
    );
    for field in [
        "utc_year",
        "utc_month",
        "utc_day",
        "utc_hour",
        "utc_minute",
        "utc_second",
    ] {
        exports
            .functions
            .insert(field.to_string(), pub_fn(vec![utc.clone()], Type::Int));
    }

    exports
}
