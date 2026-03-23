use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use std::{mem, ptr, slice, str};

use crate::initialize;
use crate::memory::ManualBox;

struct ManualRaw {
    bytes: Vec<u8>,
}

impl ManualRaw {
    fn new(size: usize) -> Self {
        Self {
            bytes: vec![0u8; size],
        }
    }

    fn ptr(&mut self) -> *mut u8 {
        self.bytes.as_mut_ptr()
    }
}

struct ManualAllocation {
    frame_id: usize,
    _storage: ManualBox<ManualRaw>,
}

struct Frame {
    id: usize,
    allocations: Vec<usize>,
}

struct AllocationTable {
    allocations: HashMap<usize, ManualAllocation>,
    frames: Vec<Frame>,
    next_frame: usize,
}

impl AllocationTable {
    fn new() -> Self {
        Self {
            allocations: HashMap::new(),
            frames: vec![Frame {
                id: 0,
                allocations: Vec::new(),
            }],
            next_frame: 1,
        }
    }

    fn push_frame(&mut self) -> usize {
        let id = self.next_frame;
        self.next_frame = self.next_frame.wrapping_add(1).max(1);
        self.frames.push(Frame {
            id,
            allocations: Vec::new(),
        });
        id
    }

    fn pop_frame(&mut self, frame_id: usize) -> Vec<usize> {
        // Pop frames from the top until we find the target frame, collecting
        // all allocations from every frame that we remove (including those
        // above the target).  This prevents leaks when frames are closed out
        // of order — which should not happen in well-formed code, but we
        // handle it defensively.
        let mut collected: Vec<usize> = Vec::new();
        while let Some(frame) = self.frames.last() {
            // Never remove the implicit base frame (id == 0).
            if frame.id == 0 {
                break;
            }
            let frame = self.frames.pop().unwrap();
            let found = frame.id == frame_id;
            collected.extend(frame.allocations);
            if found {
                return collected;
            }
        }
        // frame_id was not found — return whatever we collected so far
        // (callers will still free those allocations).
        collected
    }

    fn current_frame_mut(&mut self) -> Option<&mut Frame> {
        self.frames.last_mut()
    }

    fn remove_from_frame(&mut self, frame_id: usize, ptr: usize) {
        if let Some(frame) = self
            .frames
            .iter_mut()
            .rev()
            .find(|frame| frame.id == frame_id)
        {
            if let Some((index, _)) = frame
                .allocations
                .iter()
                .enumerate()
                .find(|(_, &stored)| stored == ptr)
            {
                frame.allocations.swap_remove(index);
            }
        }
    }

    fn clear_all(&mut self) {
        self.allocations.clear();
        self.frames.clear();
        self.frames.push(Frame {
            id: 0,
            allocations: Vec::new(),
        });
        self.next_frame = 1;
    }
}

fn allocation_table() -> &'static Mutex<AllocationTable> {
    static TABLE: OnceLock<Mutex<AllocationTable>> = OnceLock::new();
    TABLE.get_or_init(|| Mutex::new(AllocationTable::new()))
}

/// Primary scalar type exchanged through host call contexts.
pub type SpectraHostValue = i64;

/// Status codes returned by host functions.
pub const HOST_STATUS_SUCCESS: i32 = 0;
pub const HOST_STATUS_INVALID_ARGUMENT: i32 = 1;
pub const HOST_STATUS_NOT_FOUND: i32 = 2;
pub const HOST_STATUS_INTERNAL_ERROR: i32 = 3;

/// Context passed to host functions containing argument and result buffers.
#[repr(C)]
pub struct SpectraHostCallContext {
    pub args: *const SpectraHostValue,
    pub arg_len: usize,
    pub results: *mut SpectraHostValue,
    pub result_len: usize,
    /// Populated by the runtime dispatcher before invoking a host function.
    /// Allows host functions to call back into JIT-compiled Spectra closures
    /// (e.g., for higher-order functions like `list_map` and `list_filter`).
    ///
    /// Signature: `fn(fn_ptr: i64, args: *const i64, n_args: usize, result: *mut i64) -> i32`.
    /// Use [`spectra_rt_invoke_closure`] as the concrete implementation.
    /// `None` when the runtime does not support closure callbacks in this context.
    pub invoke_fn: Option<unsafe extern "C" fn(i64, *const i64, usize, *mut i64) -> i32>,
}

impl SpectraHostCallContext {
    /// Returns a slice view over the argument buffer.
    pub unsafe fn args_slice(&self) -> &[SpectraHostValue] {
        if self.args.is_null() || self.arg_len == 0 {
            &[]
        } else {
            slice::from_raw_parts(self.args, self.arg_len)
        }
    }

    /// Returns a mutable slice view over the result buffer.
    pub unsafe fn results_slice_mut(&mut self) -> &mut [SpectraHostValue] {
        if self.results.is_null() || self.result_len == 0 {
            &mut []
        } else {
            slice::from_raw_parts_mut(self.results, self.result_len)
        }
    }
}

/// Signature expected for runtime host functions.
pub type HostFunction = extern "C" fn(*mut SpectraHostCallContext) -> i32;

struct HostRegistry {
    functions: HashMap<String, usize>,
}

impl HostRegistry {
    fn new() -> Self {
        Self {
            functions: HashMap::new(),
        }
    }

    fn insert(&mut self, name: &str, ptr: *const ()) -> bool {
        self.functions
            .insert(name.to_string(), ptr as usize)
            .is_none()
    }

    fn remove(&mut self, name: &str) -> bool {
        self.functions.remove(name).is_some()
    }

    fn lookup(&self, name: &str) -> *const () {
        self.functions
            .get(name)
            .copied()
            .and_then(|value| {
                if value == 0 {
                    None
                } else {
                    Some(value as *const ())
                }
            })
            .unwrap_or(ptr::null())
    }

    fn clear(&mut self) {
        self.functions.clear();
    }
}

fn host_registry() -> &'static Mutex<HostRegistry> {
    static REGISTRY: OnceLock<Mutex<HostRegistry>> = OnceLock::new();
    REGISTRY.get_or_init(|| Mutex::new(HostRegistry::new()))
}

fn read_host_name(name_ptr: *const u8, name_len: usize) -> Option<String> {
    if name_ptr.is_null() {
        return None;
    }

    let bytes = unsafe { slice::from_raw_parts(name_ptr, name_len) };
    str::from_utf8(bytes).ok().map(|s| s.to_string())
}

/// Registers a host function accessible to JITed code.
pub fn register_host_function(name: &str, func: HostFunction) -> bool {
    let registry = host_registry();
    let mut guard = registry.lock().unwrap_or_else(|e| e.into_inner());
    guard.insert(name, func as *const ())
}

/// Removes a previously registered host function.
pub fn unregister_host_function(name: &str) -> bool {
    let registry = host_registry();
    let mut guard = registry.lock().unwrap_or_else(|e| e.into_inner());
    guard.remove(name)
}

/// Returns the host function pointer associated with the provided name.
pub fn lookup_host_function(name: &str) -> Option<HostFunction> {
    let registry = host_registry();
    let guard = registry.lock().unwrap_or_else(|e| e.into_inner());
    let ptr = guard.lookup(name);
    if ptr.is_null() {
        None
    } else {
        Some(unsafe { mem::transmute(ptr) })
    }
}

/// Clears all registered host functions.
pub fn clear_host_functions() {
    let registry = host_registry();
    let mut guard = registry.lock().unwrap_or_else(|e| e.into_inner());
    guard.clear();
}

/// Registers the built-in standard library host calls.
#[no_mangle]
pub extern "C" fn spectra_rt_std_register() {
    crate::stdlib::register();
}

/// Begins a manual allocation frame and returns its identifier.
#[no_mangle]
pub extern "C" fn spectra_rt_manual_frame_enter() -> usize {
    let table = allocation_table();
    let mut guard = table
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    guard.push_frame()
}

/// Ends a manual allocation frame, freeing all allocations created since it began.
#[no_mangle]
pub extern "C" fn spectra_rt_manual_frame_exit(frame_id: usize) {
    let table = allocation_table();
    let mut guard = table
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    let allocations = guard.pop_frame(frame_id);

    for ptr in allocations {
        guard.allocations.remove(&ptr);
    }
}

/// Allocates zero-initialised manual memory tracked by the runtime.
#[no_mangle]
pub extern "C" fn spectra_rt_manual_alloc(size: usize) -> *mut u8 {
    if size == 0 {
        return ptr::null_mut();
    }

    let state = initialize();
    let memory = state.memory();

    let mut allocation = match memory.allocate_manual(ManualRaw::new(size)) {
        Ok(allocation) => allocation,
        Err(_) => return ptr::null_mut(),
    };

    let ptr = allocation.as_mut().ptr();
    let ptr_value = ptr as usize;

    let table = allocation_table();
    let mut guard = table
        .lock()
        .unwrap_or_else(|e| e.into_inner());

    let frame_id = guard.current_frame_mut().map(|frame| frame.id).unwrap_or(0);

    guard.allocations.insert(
        ptr_value,
        ManualAllocation {
            frame_id,
            _storage: allocation,
        },
    );

    if let Some(frame) = guard.current_frame_mut() {
        frame.allocations.push(ptr_value);
    }

    ptr
}

/// Releases a manual allocation previously returned by `spectra_rt_manual_alloc`.
#[no_mangle]
pub extern "C" fn spectra_rt_manual_free(ptr: *mut u8) {
    if ptr.is_null() {
        return;
    }

    let ptr_value = ptr as usize;
    let table = allocation_table();
    let mut guard = table
        .lock()
        .unwrap_or_else(|e| e.into_inner());

    if let Some(entry) = guard.allocations.remove(&ptr_value) {
        guard.remove_from_frame(entry.frame_id, ptr_value);
    }
}

/// Clears all outstanding manual allocations owned by the runtime.
#[no_mangle]
pub extern "C" fn spectra_rt_manual_clear() {
    let table = allocation_table();
    let mut guard = table
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    guard.clear_all();
}

/// Registers a host function that JITed code can invoke by name.
#[no_mangle]
pub extern "C" fn spectra_rt_host_register(
    name_ptr: *const u8,
    name_len: usize,
    fn_ptr: *const (),
) -> bool {
    if fn_ptr.is_null() || name_len == 0 {
        return false;
    }

    let Some(name) = read_host_name(name_ptr, name_len) else {
        return false;
    };

    let registry = host_registry();
    let mut guard = registry.lock().unwrap_or_else(|e| e.into_inner());
    guard.insert(&name, fn_ptr)
}

/// Unregisters a previously registered host function.
#[no_mangle]
pub extern "C" fn spectra_rt_host_unregister(name_ptr: *const u8, name_len: usize) -> bool {
    if name_len == 0 {
        return false;
    }

    let Some(name) = read_host_name(name_ptr, name_len) else {
        return false;
    };

    let registry = host_registry();
    let mut guard = registry.lock().unwrap_or_else(|e| e.into_inner());
    guard.remove(&name)
}

/// Looks up a host function by name, returning `NULL` if not found or invalid.
#[no_mangle]
pub extern "C" fn spectra_rt_host_lookup(name_ptr: *const u8, name_len: usize) -> *const () {
    if name_len == 0 {
        return ptr::null();
    }

    let Some(name) = read_host_name(name_ptr, name_len) else {
        return ptr::null();
    };

    let registry = host_registry();
    let guard = registry.lock().unwrap_or_else(|e| e.into_inner());
    guard.lookup(&name)
}

/// Looks up a host function and invokes it with the provided context buffers.
#[no_mangle]
pub extern "C" fn spectra_rt_host_invoke(
    name_ptr: *const u8,
    name_len: usize,
    args_ptr: *const SpectraHostValue,
    arg_len: usize,
    results_ptr: *mut SpectraHostValue,
    result_len: usize,
) -> i32 {
    if name_len == 0 || name_ptr.is_null() {
        return HOST_STATUS_INVALID_ARGUMENT;
    }

    if (arg_len > 0 && args_ptr.is_null()) || (result_len > 0 && results_ptr.is_null()) {
        return HOST_STATUS_INVALID_ARGUMENT;
    }

    let Some(name) = read_host_name(name_ptr, name_len) else {
        return HOST_STATUS_INVALID_ARGUMENT;
    };

    let registry = host_registry();
    let guard = registry.lock().unwrap_or_else(|e| e.into_inner());
    let func_ptr = guard.lookup(&name);
    drop(guard);

    if func_ptr.is_null() {
        return HOST_STATUS_NOT_FOUND;
    }

    let func: HostFunction = unsafe { mem::transmute(func_ptr) };
    let mut ctx = SpectraHostCallContext {
        args: args_ptr,
        arg_len,
        results: results_ptr,
        result_len,
        invoke_fn: Some(spectra_rt_invoke_closure),
    };

    func(&mut ctx as *mut _)
}

/// Invokes a JIT-compiled Spectra function (closure or regular) by its native pointer.
///
/// # Parameters
/// - `fn_ptr`: raw i64 holding the native JIT function pointer
/// - `args`: pointer to an array of `n_args` i64 argument values (may be null when
///   `n_args == 0`)
/// - `n_args`: number of arguments — currently 0, 1, or 2 are supported
/// - `result`: output slot written with the returned i64 value; may be null for
///   unit-returning functions
///
/// # Returns
/// `HOST_STATUS_SUCCESS` on success, `HOST_STATUS_INVALID_ARGUMENT` if `fn_ptr == 0`,
/// or `HOST_STATUS_INTERNAL_ERROR` if `n_args` is outside the supported range.
///
/// # Safety
/// `fn_ptr` must be a valid JIT-compiled function pointer whose calling convention
/// matches the expected signature for the given `n_args`.
#[no_mangle]
pub unsafe extern "C" fn spectra_rt_invoke_closure(
    fn_ptr: i64,
    args: *const i64,
    n_args: usize,
    result: *mut i64,
) -> i32 {
    if fn_ptr == 0 {
        return HOST_STATUS_INVALID_ARGUMENT;
    }
    let returned: i64 = match n_args {
        0 => {
            let f: unsafe extern "C" fn() -> i64 = mem::transmute(fn_ptr as usize);
            f()
        }
        1 => {
            let f: unsafe extern "C" fn(i64) -> i64 = mem::transmute(fn_ptr as usize);
            f(if args.is_null() { 0 } else { *args })
        }
        2 => {
            let f: unsafe extern "C" fn(i64, i64) -> i64 = mem::transmute(fn_ptr as usize);
            let a0 = if args.is_null() { 0 } else { *args };
            let a1 = if args.is_null() { 0 } else { *args.add(1) };
            f(a0, a1)
        }
        _ => return HOST_STATUS_INTERNAL_ERROR,
    };
    if !result.is_null() {
        *result = returned;
    }
    HOST_STATUS_SUCCESS
}

/// Clears all registered host functions.
#[no_mangle]
pub extern "C" fn spectra_rt_host_clear() {
    let registry = host_registry();
    let mut guard = registry.lock().unwrap_or_else(|e| e.into_inner());
    guard.clear();
}

/// One-shot startup for AOT executables: initialises the runtime and registers
/// all built-in stdlib host functions.  Must be called before any Spectra code runs.
#[no_mangle]
pub extern "C" fn spectra_rt_startup() {
    initialize();
    crate::register_standard_library();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{initialize, MemoryStats};
    use std::mem;
    use std::sync::{Mutex, MutexGuard, OnceLock};

    fn test_guard() -> MutexGuard<'static, ()> {
        static GUARD: OnceLock<Mutex<()>> = OnceLock::new();
        GUARD
            .get_or_init(|| Mutex::new(()))
            .lock()
            .expect("test guard poisoned")
    }

    fn manual_stats() -> MemoryStats {
        initialize().memory_stats()
    }

    #[test]
    fn frame_exit_releases_manual_allocations() {
        let _lock = test_guard();
        spectra_rt_manual_clear();

        let baseline = manual_stats().manual;

        let frame = spectra_rt_manual_frame_enter();
        let ptr = spectra_rt_manual_alloc(32);
        assert!(!ptr.is_null());

        let after_alloc = manual_stats().manual;
        assert_eq!(after_alloc.allocations, baseline.allocations + 1);
        assert!(after_alloc.bytes >= baseline.bytes);

        spectra_rt_manual_frame_exit(frame);

        let after_exit = manual_stats().manual;
        assert_eq!(after_exit.allocations, baseline.allocations);
        assert_eq!(after_exit.bytes, baseline.bytes);

        spectra_rt_manual_clear();
    }

    #[test]
    fn manual_clear_resets_frames_and_allocations() {
        let _lock = test_guard();
        spectra_rt_manual_clear();

        let baseline = manual_stats().manual;

        let _frame_one = spectra_rt_manual_frame_enter();
        let _frame_two = spectra_rt_manual_frame_enter();
        assert!(!spectra_rt_manual_alloc(8).is_null());
        assert!(!spectra_rt_manual_alloc(16).is_null());

        let raised = manual_stats().manual;
        assert!(raised.allocations >= baseline.allocations + 2);
        assert!(raised.bytes >= baseline.bytes);

        spectra_rt_manual_clear();

        let after_clear = manual_stats().manual;
        assert_eq!(after_clear.allocations, baseline.allocations);
        assert_eq!(after_clear.bytes, baseline.bytes);

        let frame = spectra_rt_manual_frame_enter();
        assert!(!spectra_rt_manual_alloc(24).is_null());
        spectra_rt_manual_frame_exit(frame);

        let after_reuse = manual_stats().manual;
        assert_eq!(after_reuse.allocations, baseline.allocations);
        assert_eq!(after_reuse.bytes, baseline.bytes);

        spectra_rt_manual_clear();
    }

    extern "C" fn host_const() -> i64 {
        42
    }

    extern "C" fn host_inc(value: i64) -> i64 {
        value + 1
    }

    #[test]
    fn host_register_lookup_and_clear() {
        let _lock = test_guard();
        spectra_rt_host_clear();

        let name = b"spectra.test.const";
        let inserted = spectra_rt_host_register(name.as_ptr(), name.len(), host_const as *const ());
        assert!(inserted);

        let ptr = spectra_rt_host_lookup(name.as_ptr(), name.len());
        assert!(!ptr.is_null());
        let func: extern "C" fn() -> i64 = unsafe { mem::transmute(ptr) };
        assert_eq!(func(), 42);

        let replaced = spectra_rt_host_register(name.as_ptr(), name.len(), host_inc as *const ());
        assert!(!replaced);

        let ptr = spectra_rt_host_lookup(name.as_ptr(), name.len());
        let func: extern "C" fn(i64) -> i64 = unsafe { mem::transmute(ptr) };
        assert_eq!(func(41), 42);

        spectra_rt_host_clear();
        assert!(spectra_rt_host_lookup(name.as_ptr(), name.len()).is_null());
    }

    #[test]
    fn host_unregister_removes_entry() {
        let _lock = test_guard();
        spectra_rt_host_clear();

        let name = b"spectra.test.inc";
        spectra_rt_host_register(name.as_ptr(), name.len(), host_inc as *const ());
        assert!(!spectra_rt_host_lookup(name.as_ptr(), name.len()).is_null());

        assert!(spectra_rt_host_unregister(name.as_ptr(), name.len()));
        assert!(spectra_rt_host_lookup(name.as_ptr(), name.len()).is_null());

        assert!(!spectra_rt_host_unregister(name.as_ptr(), name.len()));

        spectra_rt_host_clear();
    }
}
