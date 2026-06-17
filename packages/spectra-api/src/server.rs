use crate::{read_args, write_result};
use spectra_runtime::ffi::{
    SpectraHostCallContext, SpectraHostValue, HOST_STATUS_INVALID_ARGUMENT,
};
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

pub const SERVER_STATE_CREATED: SpectraHostValue = 1;
pub const SERVER_STATE_STOPPED: SpectraHostValue = 2;

struct ServerStore {
    next: SpectraHostValue,
    states: HashMap<SpectraHostValue, SpectraHostValue>,
}

impl ServerStore {
    fn new() -> Self {
        Self {
            next: 1,
            states: HashMap::new(),
        }
    }
}

fn store() -> &'static Mutex<ServerStore> {
    static STORE: OnceLock<Mutex<ServerStore>> = OnceLock::new();
    STORE.get_or_init(|| Mutex::new(ServerStore::new()))
}

pub extern "C" fn server_new(ctx: *mut SpectraHostCallContext) -> i32 {
    let mut store = store().lock().unwrap_or_else(|e| e.into_inner());
    let handle = store.next;
    store.next = store.next.saturating_add(1).max(1);
    store.states.insert(handle, SERVER_STATE_CREATED);
    write_result(ctx, handle)
}

pub extern "C" fn server_state(ctx: *mut SpectraHostCallContext) -> i32 {
    let Ok(args) = read_args(ctx, 1) else {
        return HOST_STATUS_INVALID_ARGUMENT;
    };
    let store = store().lock().unwrap_or_else(|e| e.into_inner());
    let Some(state) = store.states.get(&args[0]).copied() else {
        return HOST_STATUS_INVALID_ARGUMENT;
    };
    write_result(ctx, state)
}

pub extern "C" fn server_shutdown(ctx: *mut SpectraHostCallContext) -> i32 {
    let Ok(args) = read_args(ctx, 1) else {
        return HOST_STATUS_INVALID_ARGUMENT;
    };
    let mut store = store().lock().unwrap_or_else(|e| e.into_inner());
    let Some(state) = store.states.get_mut(&args[0]) else {
        return HOST_STATUS_INVALID_ARGUMENT;
    };
    *state = SERVER_STATE_STOPPED;
    write_result(ctx, 1)
}
