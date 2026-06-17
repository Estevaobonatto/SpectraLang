use crate::{read_args, write_result};
use spectra_runtime::ffi::{
    SpectraHostCallContext, SpectraHostValue, HOST_STATUS_INVALID_ARGUMENT,
};
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

struct RouterStore {
    next: SpectraHostValue,
    route_counts: HashMap<SpectraHostValue, SpectraHostValue>,
}

impl RouterStore {
    fn new() -> Self {
        Self {
            next: 1,
            route_counts: HashMap::new(),
        }
    }
}

fn store() -> &'static Mutex<RouterStore> {
    static STORE: OnceLock<Mutex<RouterStore>> = OnceLock::new();
    STORE.get_or_init(|| Mutex::new(RouterStore::new()))
}

pub extern "C" fn router_new(ctx: *mut SpectraHostCallContext) -> i32 {
    let mut store = store().lock().unwrap_or_else(|e| e.into_inner());
    let handle = store.next;
    store.next = store.next.saturating_add(1).max(1);
    store.route_counts.insert(handle, 0);
    write_result(ctx, handle)
}

pub extern "C" fn route_count(ctx: *mut SpectraHostCallContext) -> i32 {
    let Ok(args) = read_args(ctx, 1) else {
        return HOST_STATUS_INVALID_ARGUMENT;
    };
    let store = store().lock().unwrap_or_else(|e| e.into_inner());
    let Some(count) = store.route_counts.get(&args[0]).copied() else {
        return HOST_STATUS_INVALID_ARGUMENT;
    };
    write_result(ctx, count)
}
