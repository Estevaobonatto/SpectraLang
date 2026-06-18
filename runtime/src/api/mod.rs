//! Runtime-facing API host-call namespace contract.
//!
//! The concrete `spectra.api` implementation lives in the workspace crate
//! `spectra-api`, which depends on `spectra-runtime` and registers these names
//! through the runtime host-call registry.

pub const HOST_PREFIX: &str = "spectra.api.";

pub const REQUIRED_HOST_CALLS: &[&str] = &[
    "spectra.api.version.major",
    "spectra.api.version.minor",
    "spectra.api.version.patch",
    "spectra.api.http.method_name",
    "spectra.api.http.method_allows_body",
    "spectra.api.http.method_is_safe",
    "spectra.api.http.method_get",
    "spectra.api.http.method_head",
    "spectra.api.http.method_post",
    "spectra.api.http.method_put",
    "spectra.api.http.method_patch",
    "spectra.api.http.method_delete",
    "spectra.api.http.method_options",
    "spectra.api.http.status_reason",
    "spectra.api.http.status_class",
    "spectra.api.http.status_is_success",
    "spectra.api.http.status",
    "spectra.api.http.status_continue",
    "spectra.api.http.status_switching_protocols",
    "spectra.api.http.status_ok",
    "spectra.api.http.status_created",
    "spectra.api.http.status_accepted",
    "spectra.api.http.status_no_content",
    "spectra.api.http.status_moved_permanently",
    "spectra.api.http.status_found",
    "spectra.api.http.status_not_modified",
    "spectra.api.http.status_bad_request",
    "spectra.api.http.status_unauthorized",
    "spectra.api.http.status_forbidden",
    "spectra.api.http.status_not_found",
    "spectra.api.http.status_method_not_allowed",
    "spectra.api.http.status_conflict",
    "spectra.api.http.status_unsupported_media_type",
    "spectra.api.http.status_unprocessable_content",
    "spectra.api.http.status_too_many_requests",
    "spectra.api.http.status_internal_server_error",
    "spectra.api.http.status_bad_gateway",
    "spectra.api.http.status_service_unavailable",
    "spectra.api.http.status_gateway_timeout",
    "spectra.api.http.header_name_is_valid",
    "spectra.api.http.header_value_is_valid",
    "spectra.api.http.request_new",
    "spectra.api.http.request",
    "spectra.api.http.request_method",
    "spectra.api.http.request_path",
    "spectra.api.http.request_header",
    "spectra.api.http.request_cookie",
    "spectra.api.http.response_new",
    "spectra.api.http.response",
    "spectra.api.http.response_status",
    "spectra.api.http.response_header",
    "spectra.api.http.response_body_len",
    "spectra.api.http.header",
    "spectra.api.http.header_name",
    "spectra.api.http.header_value",
    "spectra.api.http.cookie",
    "spectra.api.http.cookie_name",
    "spectra.api.http.cookie_value",
    "spectra.api.server.new",
    "spectra.api.server.state",
    "spectra.api.server.shutdown",
    "spectra.api.client.new",
    "spectra.api.client.timeout_ms",
    "spectra.api.json.validate",
    "spectra.api.json.kind",
    "spectra.api.tls.config_new",
    "spectra.api.tls.config_mode",
    "spectra.api.routing.router_new",
    "spectra.api.routing.route_count",
    "spectra.api.routing.route_id",
    "spectra.api.routing.route_add",
    "spectra.api.routing.get",
    "spectra.api.routing.post",
    "spectra.api.routing.put",
    "spectra.api.routing.patch",
    "spectra.api.routing.delete",
    "spectra.api.routing.route_match",
    "spectra.api.routing.match_route_id",
    "spectra.api.routing.match_param",
    "spectra.api.routing.match_param_int",
    "spectra.api.routing.last_conflict",
    "spectra.api.query.type_string",
    "spectra.api.query.type_int",
    "spectra.api.query.type_bool",
    "spectra.api.query.parse",
    "spectra.api.query.len",
    "spectra.api.query.has",
    "spectra.api.query.count",
    "spectra.api.query.first",
    "spectra.api.query.value",
    "spectra.api.query.int",
    "spectra.api.query.bool",
    "spectra.api.query.schema",
    "spectra.api.query.schema_field",
    "spectra.api.query.bind",
    "spectra.api.query.binding_ok",
    "spectra.api.query.binding_error",
    "spectra.api.query.binding_count",
    "spectra.api.query.binding_value",
    "spectra.api.query.binding_int",
    "spectra.api.query.binding_bool",
    "spectra.api.query.error_code",
    "spectra.api.query.error_message",
    "spectra.api.errors.last_code",
    "spectra.api.errors.last_message",
];

pub fn required_host_call_count() -> usize {
    REQUIRED_HOST_CALLS.len()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn api_host_call_contract_is_unique_and_prefixed() {
        let mut seen = HashSet::new();
        for name in REQUIRED_HOST_CALLS {
            assert!(name.starts_with(HOST_PREFIX), "{name}");
            assert!(seen.insert(*name), "{name}");
        }
        assert_eq!(required_host_call_count(), 105);
    }
}
