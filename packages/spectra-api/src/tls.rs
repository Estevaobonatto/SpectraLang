use crate::{read_args, write_result};
use spectra_runtime::ffi::{
    SpectraHostCallContext, SpectraHostValue, HOST_STATUS_INVALID_ARGUMENT,
};
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

pub const TLS_MODE_SERVER: SpectraHostValue = 1;
pub const TLS_MODE_CLIENT: SpectraHostValue = 2;

struct TlsStore {
    next: SpectraHostValue,
    modes: HashMap<SpectraHostValue, SpectraHostValue>,
}

impl TlsStore {
    fn new() -> Self {
        Self {
            next: 1,
            modes: HashMap::new(),
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
    let handle = store.next;
    store.next = store.next.saturating_add(1).max(1);
    store.modes.insert(handle, args[0]);
    write_result(ctx, handle)
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
