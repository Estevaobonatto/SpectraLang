use crate::{read_args, write_result};
use spectra_runtime::ffi::{
    SpectraHostCallContext, SpectraHostValue, HOST_STATUS_INVALID_ARGUMENT,
};
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

const DEFAULT_TIMEOUT_MS: SpectraHostValue = 30_000;

struct ClientStore {
    next: SpectraHostValue,
    timeouts: HashMap<SpectraHostValue, SpectraHostValue>,
}

impl ClientStore {
    fn new() -> Self {
        Self {
            next: 1,
            timeouts: HashMap::new(),
        }
    }
}

fn store() -> &'static Mutex<ClientStore> {
    static STORE: OnceLock<Mutex<ClientStore>> = OnceLock::new();
    STORE.get_or_init(|| Mutex::new(ClientStore::new()))
}

pub extern "C" fn client_new(ctx: *mut SpectraHostCallContext) -> i32 {
    let mut store = store().lock().unwrap_or_else(|e| e.into_inner());
    let handle = store.next;
    store.next = store.next.saturating_add(1).max(1);
    store.timeouts.insert(handle, DEFAULT_TIMEOUT_MS);
    write_result(ctx, handle)
}

pub extern "C" fn client_timeout_ms(ctx: *mut SpectraHostCallContext) -> i32 {
    let Ok(args) = read_args(ctx, 1) else {
        return HOST_STATUS_INVALID_ARGUMENT;
    };
    let store = store().lock().unwrap_or_else(|e| e.into_inner());
    let Some(timeout) = store.timeouts.get(&args[0]).copied() else {
        return HOST_STATUS_INVALID_ARGUMENT;
    };
    write_result(ctx, timeout)
}
