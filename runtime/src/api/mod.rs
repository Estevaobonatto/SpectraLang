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
    "spectra.api.http.status_reason",
    "spectra.api.http.status_class",
    "spectra.api.http.status_is_success",
    "spectra.api.http.header_name_is_valid",
    "spectra.api.http.header_value_is_valid",
    "spectra.api.http.request_new",
    "spectra.api.http.request_method",
    "spectra.api.http.response_new",
    "spectra.api.http.response_status",
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
        assert_eq!(required_host_call_count(), 28);
    }
}
