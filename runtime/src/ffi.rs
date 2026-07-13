use std::collections::HashMap;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::{Mutex, OnceLock};
use std::{mem, ptr, slice, str};

use crate::initialize;

// ── Program argument store ───────────────────────────────────────────────────

/// Program arguments forwarded from the host to Spectra code.
/// Set by the JIT runner before execution or by [`spectra_rt_startup_with_args`]
/// in AOT executables. Uses `OnceLock` so it can be set exactly once per process.
static PROGRAM_ARGV: OnceLock<Vec<String>> = OnceLock::new();

/// Returns the program arguments if they have been set, otherwise `None`.
pub(crate) fn get_program_args() -> Option<&'static Vec<String>> {
    PROGRAM_ARGV.get()
}

/// Sets the program arguments visible to `std.env` host functions.
/// Subsequent calls are silently ignored (can only be set once per process).
pub fn set_program_args(args: Vec<String>) {
    let _ = PROGRAM_ARGV.set(args);
}
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

    fn check_invariants(&self) -> bool {
        if self.frames.first().map(|frame| frame.id) != Some(0) {
            return false;
        }

        let mut frame_ids = std::collections::HashSet::new();
        let mut frame_allocations = std::collections::HashSet::new();
        for frame in &self.frames {
            if !frame_ids.insert(frame.id) {
                return false;
            }
            for ptr in &frame.allocations {
                if !frame_allocations.insert(*ptr) {
                    return false;
                }
                match self.allocations.get(ptr) {
                    Some(allocation) if allocation.frame_id == frame.id => {}
                    _ => return false,
                }
            }
        }

        self.allocations.iter().all(|(ptr, allocation)| {
            frame_allocations.contains(ptr) && frame_ids.contains(&allocation.frame_id)
        })
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

    fn check_invariants(&self) -> bool {
        self.functions
            .iter()
            .all(|(name, ptr)| !name.is_empty() && *ptr != 0)
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
    let mut guard = table.lock().unwrap_or_else(|e| e.into_inner());
    guard.push_frame()
}

/// Ends a manual allocation frame, freeing all allocations created since it began.
#[no_mangle]
pub extern "C" fn spectra_rt_manual_frame_exit(frame_id: usize) {
    let table = allocation_table();
    let mut guard = table.lock().unwrap_or_else(|e| e.into_inner());
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
    let mut guard = table.lock().unwrap_or_else(|e| e.into_inner());

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

const SPECTRA_STRING_SCAN_LIMIT: usize = 16 * 1024 * 1024;

/// Fast ABI entry for `std.string.len`.
///
/// Spectra strings are null-terminated buffers with one byte per i64 slot.
/// This keeps the hot path out of the generic host-call dispatcher while
/// preserving the same null handling as the stdlib host function.
#[no_mangle]
pub extern "C" fn spectra_rt_string_len(ptr_val: SpectraHostValue) -> SpectraHostValue {
    if ptr_val == 0 {
        return 0;
    }

    let raw = ptr_val as *const i64;
    for offset in 0..SPECTRA_STRING_SCAN_LIMIT {
        let slot = unsafe { *raw.add(offset) };
        if slot == 0 {
            return offset as SpectraHostValue;
        }
    }

    0
}

/// Fast ABI entry for `std.string.char_at`.
///
/// Returns -1 for null strings, negative indexes, and indexes at or after the
/// null terminator, matching the public stdlib contract.
#[no_mangle]
pub extern "C" fn spectra_rt_string_char_at(
    ptr_val: SpectraHostValue,
    index: SpectraHostValue,
) -> SpectraHostValue {
    if ptr_val == 0 || index < 0 {
        return -1;
    }

    let target = index as usize;
    if target >= SPECTRA_STRING_SCAN_LIMIT {
        return -1;
    }

    let raw = ptr_val as *const i64;
    for offset in 0..=target {
        let slot = unsafe { *raw.add(offset) };
        if slot == 0 {
            return -1;
        }
        if offset == target {
            return (slot as u8) as SpectraHostValue;
        }
    }

    -1
}

/// Fast ABI entry for `concurrent.task_spawn(value)`.
///
/// Skips the generic host-call dispatch (no `manual_alloc`/`free` for the
/// args/result buffer pair, no host name lookup, no `catch_unwind`). Called
/// directly from JIT code when the backend inlines the `task_spawn` call.
///
/// Returns the new task_id (>0 on success) or 0 on internal error
/// (poisoned registry mutex). Task 0 is reserved as the invalid sentinel.
#[no_mangle]
#[inline(never)]
pub extern "C" fn spectra_rt_concurrent_spawn_fast(value: SpectraHostValue) -> SpectraHostValue {
    crate::stdlib::concurrent_spawn_fast(value)
}

/// Fast ABI entry for `concurrent.task_join(task_id)`.
///
/// Skips the generic host-call dispatch. Called directly from JIT code when
/// the backend inlines the `task_join` call.
///
/// Returns the value written by the matching `task_spawn`, or 0 if the
/// task_id is invalid (out of range, recycled, or never existed).
#[no_mangle]
#[inline(never)]
pub extern "C" fn spectra_rt_concurrent_join_fast(task_id: SpectraHostValue) -> SpectraHostValue {
    crate::stdlib::concurrent_join_fast(task_id)
}

#[no_mangle]
#[inline(never)]
pub extern "C" fn spectra_rt_concurrent_spawn_batch_fast(
    first_value: SpectraHostValue,
    count: SpectraHostValue,
) -> SpectraHostValue {
    crate::stdlib::concurrent_spawn_batch_fast(first_value, count)
}

#[no_mangle]
#[inline(never)]
pub extern "C" fn spectra_rt_concurrent_join_batch_sum_fast(
    batch_id: SpectraHostValue,
) -> SpectraHostValue {
    crate::stdlib::concurrent_join_batch_sum_fast(batch_id)
}

/// Fast ABI entry for an immediately paired concurrent spawn and join.
#[no_mangle]
#[inline(never)]
pub extern "C" fn spectra_rt_concurrent_spawn_join_fast(
    value: SpectraHostValue,
) -> SpectraHostValue {
    crate::stdlib::concurrent_spawn_join_fast(value)
}

/// Fast ABI entry for `concurrent.reset()`.
#[no_mangle]
#[inline(never)]
pub extern "C" fn spectra_rt_concurrent_reset_fast() -> SpectraHostValue {
    crate::stdlib::concurrent_reset_fast()
}

/// Fast ABI entry for `str.builder_new(capacity)`.
///
/// Skips the generic host-call dispatch. Called directly from JIT code
/// when the backend inlines the `builder_new` call.
///
/// Returns the new builder handle (>0 on success) or 0 on internal error.
#[no_mangle]
#[inline(never)]
pub extern "C" fn spectra_rt_builder_new(capacity: SpectraHostValue) -> SpectraHostValue {
    crate::stdlib::string_builder_new_fast(capacity as usize)
}

/// Fast ABI entry for `str.builder_push(handle, str_ptr)`.
///
/// Skips the generic host-call dispatch and the intermediate `String`
/// allocation in `read_spectra_string`. Reads the Spectra string bytes
/// directly into the builder buffer. Called directly from JIT code when
/// the backend inlines the `builder_push` call.
#[no_mangle]
#[inline(never)]
pub extern "C" fn spectra_rt_builder_push(handle: SpectraHostValue, str_ptr: SpectraHostValue) {
    crate::stdlib::string_builder_push_fast(handle as usize, str_ptr)
}

/// Fast ABI entry for `str.builder_len(handle)`.
///
/// Skips the generic host-call dispatch. Returns the current byte count
/// of the builder, or 0 for an invalid handle.
#[no_mangle]
#[inline(never)]
pub extern "C" fn spectra_rt_builder_len(handle: SpectraHostValue) -> SpectraHostValue {
    crate::stdlib::string_builder_len_fast(handle as usize)
}

/// Fast ABI entry for `str.builder_finish(handle)`.
///
/// Skips the generic host-call dispatch. Returns a Spectra string handle
/// for the accumulated bytes, or 0 for an invalid handle.
#[no_mangle]
#[inline(never)]
pub extern "C" fn spectra_rt_builder_finish(handle: SpectraHostValue) -> SpectraHostValue {
    crate::stdlib::string_builder_finish_fast(handle as usize)
}

/// Fast ABI entry for `str.builder_free(handle)`.
///
/// Skips the generic host-call dispatch. Frees the builder resources.
#[no_mangle]
#[inline(never)]
pub extern "C" fn spectra_rt_builder_free(handle: SpectraHostValue) {
    crate::stdlib::string_builder_free_fast(handle as usize)
}

/// Fast ABI entry for `col.map_set(handle, key, value)`.
///
/// Skips the generic host-call dispatch. Called directly from JIT code
/// when the backend inlines the `map_set` call. Returns `HOST_STATUS_SUCCESS`
/// (0) on success or `HOST_STATUS_NOT_FOUND` if the handle is invalid.
#[no_mangle]
#[inline(never)]
pub extern "C" fn spectra_rt_map_set_fast(
    handle: SpectraHostValue,
    key: SpectraHostValue,
    value: SpectraHostValue,
) -> i32 {
    crate::stdlib::map_set_fast(handle as usize, key, value)
}

/// Fast ABI entry for `col.map_get(handle, key)`.
///
/// Skips the generic host-call dispatch. Returns the value for the key,
/// or 0 if the key is absent or the handle is invalid. Cannot distinguish
/// "stored value is 0" from "key absent / invalid handle".
#[no_mangle]
#[inline(never)]
pub extern "C" fn spectra_rt_map_get_fast(
    handle: SpectraHostValue,
    key: SpectraHostValue,
) -> SpectraHostValue {
    crate::stdlib::map_get_fast(handle as usize, key)
}

/// Fast ABI entry for `col.map_contains(handle, key)`.
///
/// Skips the generic host-call dispatch. Returns 1 if the key is present
/// in the map, 0 otherwise (including invalid handle).
#[no_mangle]
#[inline(never)]
pub extern "C" fn spectra_rt_map_contains_fast(
    handle: SpectraHostValue,
    key: SpectraHostValue,
) -> SpectraHostValue {
    crate::stdlib::map_contains_fast(handle as usize, key)
}

/// Fast ABI entry for `col.map_new()`.
///
/// Creates a new empty map and returns its handle (>0) or 0 on internal
/// error (poisoned registry mutex).
#[no_mangle]
#[inline(never)]
pub extern "C" fn spectra_rt_map_new_fast() -> SpectraHostValue {
    crate::stdlib::map_new_fast()
}

/// Fast ABI entry for `col.map_remove(handle, key)`.
///
/// Skips the generic host-call dispatch. Returns the removed value, or
/// 0 if the key was absent / handle is invalid. Same caveat as
/// `map_get_fast` regarding stored 0.
#[no_mangle]
#[inline(never)]
pub extern "C" fn spectra_rt_map_remove_fast(
    handle: SpectraHostValue,
    key: SpectraHostValue,
) -> SpectraHostValue {
    crate::stdlib::map_remove_fast(handle as usize, key)
}

/// Fast ABI entry for `col.map_len(handle)`.
///
/// Skips the generic host-call dispatch. Returns the number of entries
/// in the map, or 0 for an invalid handle.
#[no_mangle]
#[inline(never)]
pub extern "C" fn spectra_rt_map_len_fast(handle: SpectraHostValue) -> SpectraHostValue {
    crate::stdlib::map_len_fast(handle as usize)
}

/// Fast ABI entry for `col.map_clear(handle)`.
///
/// Skips the generic host-call dispatch. Removes all entries from the
/// map. No-op for an invalid handle.
#[no_mangle]
#[inline(never)]
pub extern "C" fn spectra_rt_map_clear_fast(handle: SpectraHostValue) {
    crate::stdlib::map_clear_fast(handle as usize)
}

/// Fast ABI entry for `col.map_free(handle)`.
///
/// Skips the generic host-call dispatch. Removes the map from the
/// registry. No-op for an invalid handle.
#[no_mangle]
#[inline(never)]
pub extern "C" fn spectra_rt_map_free_fast(handle: SpectraHostValue) {
    crate::stdlib::map_free_fast(handle as usize)
}

/// Fast ABI entry for `concurrent.channel_new()`.
///
/// Skips the generic host-call dispatch. Returns the new channel id
/// (>0 on success) or 0 on internal error.
#[no_mangle]
#[inline(never)]
pub extern "C" fn spectra_rt_channel_new_fast() -> SpectraHostValue {
    crate::stdlib::concurrent_channel_new_fast()
}

/// Fast ABI entry for `concurrent.channel_send(channel, value)`.
///
/// Skips the generic host-call dispatch. Returns 1 on success, 0 if the
/// channel is closed. Returns `HOST_STATUS_NOT_FOUND` (-2) if the channel
/// id is invalid.
#[no_mangle]
#[inline(never)]
pub extern "C" fn spectra_rt_channel_send_fast(
    channel: SpectraHostValue,
    value: SpectraHostValue,
) -> i32 {
    crate::stdlib::concurrent_channel_send_fast(channel, value)
}

/// Fast ABI entry for `concurrent.channel_recv(channel)`.
///
/// Skips the generic host-call dispatch. Returns the next value in the
/// channel, or -1 if the channel is empty / closed / invalid id.
#[no_mangle]
#[inline(never)]
pub extern "C" fn spectra_rt_channel_recv_fast(channel: SpectraHostValue) -> SpectraHostValue {
    crate::stdlib::concurrent_channel_recv_fast(channel)
}

/// Fast ABI entry for `concurrent.channel_close(channel)`.
///
/// Skips the generic host-call dispatch. Returns `HOST_STATUS_SUCCESS` (0)
/// on success or `HOST_STATUS_NOT_FOUND` (-2) if the channel id is invalid.
#[no_mangle]
#[inline(never)]
pub extern "C" fn spectra_rt_channel_close_fast(channel: SpectraHostValue) -> i32 {
    crate::stdlib::concurrent_channel_close_fast(channel)
}

/// Fast ABI entry for `concurrent.channel_len(channel)`.
///
/// Skips the generic host-call dispatch. Returns the queue length, or 0
/// for an invalid channel id.
#[no_mangle]
#[inline(never)]
pub extern "C" fn spectra_rt_channel_len_fast(channel: SpectraHostValue) -> SpectraHostValue {
    crate::stdlib::concurrent_channel_len_fast(channel)
}

/// Fast ABI entry for `ml.linear(input, weight, bias)`.
///
/// Skips the generic host-call dispatch. Returns the new tensor handle
/// (>0) on success or 0 on error.
#[no_mangle]
#[inline(never)]
pub extern "C" fn spectra_rt_ml_linear_fast(
    input_h: SpectraHostValue,
    weight_h: SpectraHostValue,
    bias_h: SpectraHostValue,
) -> SpectraHostValue {
    crate::stdlib::ml_linear_fast(input_h as usize, weight_h as usize, bias_h as usize)
}

/// Fast ABI entry for `ml.mse_loss(prediction, target)`.
///
/// Skips the generic host-call dispatch. Returns the new loss tensor
/// handle (>0) on success or 0 on error.
#[no_mangle]
#[inline(never)]
pub extern "C" fn spectra_rt_ml_mse_loss_fast(
    prediction_h: SpectraHostValue,
    target_h: SpectraHostValue,
) -> SpectraHostValue {
    crate::stdlib::ml_mse_loss_fast(prediction_h as usize, target_h as usize)
}

/// Fast ABI entry for `tensor.backward(loss)`.
///
/// Skips the generic host-call dispatch. Returns `HOST_STATUS_SUCCESS` (0)
/// on success or the error code on failure.
#[no_mangle]
#[inline(never)]
pub extern "C" fn spectra_rt_tensor_backward_fast(loss_h: SpectraHostValue) -> i32 {
    crate::stdlib::tensor_backward_fast(loss_h as usize)
}

/// Fast ABI entry for `ml.sgd_step(param, lr)`.
///
/// Skips the generic host-call dispatch. `lr` is the raw `f64` learning
/// rate passed directly across the FFI boundary. Returns `HOST_STATUS_SUCCESS` (0)
/// on success, `HOST_STATUS_INVALID_ARGUMENT` if the LR is invalid, or
/// `HOST_STATUS_NOT_FOUND` if the handle is invalid.
#[no_mangle]
#[inline(never)]
pub extern "C" fn spectra_rt_ml_sgd_step_fast(param_h: SpectraHostValue, lr: f64) -> i32 {
    crate::stdlib::ml_sgd_step_fast(param_h as usize, lr)
}

/// Fast ABI entry for `tensor.full_f(n, value)`.
///
/// Skips the generic host-call dispatch. `value` is the raw `f64` fill value.
/// Returns the new tensor handle (>0) on success or 0 on error.
#[no_mangle]
#[inline(never)]
pub extern "C" fn spectra_rt_tensor_full_f_fast(
    n: SpectraHostValue,
    value: f64,
) -> SpectraHostValue {
    crate::stdlib::tensor_full_f_fast(n as usize, value)
}

/// Fast ABI entry for `str.len(s)`.
///
/// Skips the generic host-call dispatch AND the intermediate `String`
/// allocation in `read_spectra_string`. Walks the null-terminated `i64`
/// array directly to count bytes. Called from JIT/AOT code when the inline
/// path is not available (e.g., AOT path with non-statically-known length).
///
/// Returns the string length (>0) or `0` for an invalid handle.
#[no_mangle]
#[inline(never)]
pub extern "C" fn spectra_rt_string_len_fast(s: SpectraHostValue) -> SpectraHostValue {
    crate::stdlib::string_len_fast(s)
}

/// Fast ABI entry for `str.char_at(s, index)`.
///
/// Skips the generic host-call dispatch AND the intermediate `String`
/// allocation in `read_spectra_string`. Reads the byte at the given index
/// directly from the null-terminated `i64` array. Called from JIT/AOT code
/// when the inline path is not available.
///
/// Returns the byte value (0-255) on success or `-1` for an out-of-bounds
/// access (null handle, negative index, or index past the null terminator).
#[no_mangle]
#[inline(never)]
pub extern "C" fn spectra_rt_string_char_at_fast(
    s: SpectraHostValue,
    index: SpectraHostValue,
) -> SpectraHostValue {
    crate::stdlib::string_char_at_fast(s, index)
}

/// Releases a manual allocation previously returned by `spectra_rt_manual_alloc`.
#[no_mangle]
pub extern "C" fn spectra_rt_manual_free(ptr: *mut u8) {
    if ptr.is_null() {
        return;
    }

    let ptr_value = ptr as usize;
    let table = allocation_table();
    let mut guard = table.lock().unwrap_or_else(|e| e.into_inner());

    if let Some(entry) = guard.allocations.remove(&ptr_value) {
        guard.remove_from_frame(entry.frame_id, ptr_value);
    }
}

/// Moves a manual allocation from the current function's frame to its parent frame,
/// so that it survives the current function's `frame_exit` call.
///
/// Only moves the allocation if it currently belongs to `current_frame_id` — this
/// prevents accidentally re-parenting allocations that were passed in from the caller.
/// If `ptr` is not a tracked allocation (e.g. a scalar value), this is a no-op.
#[no_mangle]
pub extern "C" fn spectra_rt_manual_escape(ptr: *mut u8, current_frame_id: usize) {
    if ptr.is_null() {
        return;
    }
    let ptr_value = ptr as usize;
    let table = allocation_table();
    let mut guard = table.lock().unwrap_or_else(|e| e.into_inner());

    // Only escape allocations that belong to the current frame.
    let old_frame_id = match guard.allocations.get(&ptr_value) {
        Some(entry) if entry.frame_id == current_frame_id => entry.frame_id,
        _ => return,
    };

    guard.remove_from_frame(old_frame_id, ptr_value);

    // Find the parent frame (second from the top of the stack).
    let parent_frame_id = if guard.frames.len() >= 2 {
        guard.frames[guard.frames.len() - 2].id
    } else {
        0 // base frame
    };

    if let Some(entry) = guard.allocations.get_mut(&ptr_value) {
        entry.frame_id = parent_frame_id;
    }

    if let Some(parent) = guard
        .frames
        .iter_mut()
        .rev()
        .find(|f| f.id == parent_frame_id)
    {
        parent.allocations.push(ptr_value);
    }
}

/// Clears all outstanding manual allocations owned by the runtime.
#[no_mangle]
pub extern "C" fn spectra_rt_manual_clear() {
    let table = allocation_table();
    let mut guard = table.lock().unwrap_or_else(|e| e.into_inner());
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

    match catch_unwind(AssertUnwindSafe(|| func(&mut ctx as *mut _))) {
        Ok(status) => status,
        Err(_) => HOST_STATUS_INTERNAL_ERROR,
    }
}

/// Checks runtime debug invariants for host registry and manual allocation state.
///
/// This function is cheap enough to use in stress/soak validation and returns
/// false instead of panicking so automation can report a normal failure.
#[no_mangle]
pub extern "C" fn spectra_rt_debug_invariants_check() -> bool {
    let host_ok = host_registry()
        .lock()
        .map(|registry| registry.check_invariants())
        .unwrap_or(false);
    let allocation_ok = allocation_table()
        .lock()
        .map(|table| table.check_invariants())
        .unwrap_or(false);
    host_ok && allocation_ok
}

/// Invokes a JIT-compiled Spectra closure by its runtime closure handle.
///
/// # Parameters
/// - `fn_ptr`: raw i64 holding the closure object pointer. Slot 0 stores the
///   native code pointer; the closure handle itself is passed as hidden env.
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
/// `fn_ptr` must be a valid closure handle whose code pointer calling convention
/// matches `fn(env, args...) -> i64` for the given `n_args`.
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
    let closure_slots = fn_ptr as *const i64;
    if closure_slots.is_null() {
        return HOST_STATUS_INVALID_ARGUMENT;
    }
    let code_ptr = *closure_slots;
    if code_ptr == 0 {
        return HOST_STATUS_INVALID_ARGUMENT;
    }
    let returned: i64 = match n_args {
        0 => {
            let f: unsafe extern "C" fn(i64) -> i64 = mem::transmute(code_ptr as usize);
            f(fn_ptr)
        }
        1 => {
            let f: unsafe extern "C" fn(i64, i64) -> i64 = mem::transmute(code_ptr as usize);
            f(fn_ptr, if args.is_null() { 0 } else { *args })
        }
        2 => {
            let f: unsafe extern "C" fn(i64, i64, i64) -> i64 = mem::transmute(code_ptr as usize);
            let a0 = if args.is_null() { 0 } else { *args };
            let a1 = if args.is_null() { 0 } else { *args.add(1) };
            f(fn_ptr, a0, a1)
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

/// Startup for AOT executables with argument forwarding.
/// Initialises the runtime, registers the standard library, and stores the
/// process arguments so that `std.env.env_args_count` and `std.env.env_arg`
/// return the program's own arguments rather than the CLI's arguments.
///
/// # Safety
/// `argv` must be a valid C-style array of `argc` null-terminated UTF-8
/// strings, as provided by the OS through the C `main(argc, argv)` signature.
#[no_mangle]
pub extern "C" fn spectra_rt_startup_with_args(argc: i32, argv: *const *const u8) {
    initialize();
    crate::register_standard_library();
    if argv.is_null() || argc <= 0 {
        return;
    }
    let args: Vec<String> = (0..argc as usize)
        .filter_map(|i| unsafe {
            let ptr = *argv.add(i);
            if ptr.is_null() {
                return None;
            }
            let len = (0..).take_while(|&j| *ptr.add(j) != 0).count();
            str::from_utf8(slice::from_raw_parts(ptr, len))
                .ok()
                .map(str::to_owned)
        })
        .collect();
    let _ = PROGRAM_ARGV.set(args);
}

/// Called at the end of every AOT executable's native `main` shim.
///
/// On Windows, if the process owns its console (i.e. it was launched by
/// double-clicking in Explorer rather than from a terminal), prints a
/// "press any key" prompt and waits so that the output window stays open
/// long enough for the user to read the output.
///
/// On all other platforms (and on Windows when running from a terminal)
/// this is a no-op.
#[no_mangle]
pub extern "C" fn spectra_rt_maybe_pause() {
    #[cfg(target_os = "windows")]
    {
        // GetConsoleProcessList returns the number of processes attached to the
        // current console.  When the value is <= 1 this process is the sole
        // owner, which happens when the user double-clicks the .exe.
        extern "system" {
            fn GetConsoleProcessList(lpdwProcessList: *mut u32, dwProcessCount: u32) -> u32;
        }
        let standalone = unsafe {
            let mut pids = [0u32; 2];
            GetConsoleProcessList(pids.as_mut_ptr(), 2) <= 1
        };
        if standalone {
            use std::io::{Read, Write};
            let _ = std::io::stdout().flush();
            let _ = write!(
                std::io::stderr(),
                "\nAperte qualquer tecla para continuar..."
            );
            let _ = std::io::stderr().flush();
            // Read one byte — waits until the user presses Enter (or any key
            // that produces input on the console's stdin stream).
            let _ = std::io::stdin().read(&mut [0u8; 1]);
        }
    }
    // On non-Windows platforms terminals remain open on exit — nothing to do.
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{initialize, MemoryStats};
    use std::mem;
    fn test_guard() -> std::sync::MutexGuard<'static, ()> {
        crate::runtime_test_guard()
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

    #[test]
    fn string_fast_abi_matches_std_contract() {
        let _lock = test_guard();
        spectra_rt_manual_clear();

        let raw = spectra_rt_manual_alloc(4 * std::mem::size_of::<i64>()) as *mut i64;
        assert!(!raw.is_null());
        unsafe {
            *raw.add(0) = b'a' as i64;
            *raw.add(1) = b'b' as i64;
            *raw.add(2) = b'c' as i64;
            *raw.add(3) = 0;
        }

        let ptr = raw as SpectraHostValue;
        assert_eq!(spectra_rt_string_len(ptr), 3);
        assert_eq!(spectra_rt_string_char_at(ptr, 0), b'a' as i64);
        assert_eq!(spectra_rt_string_char_at(ptr, 2), b'c' as i64);
        assert_eq!(spectra_rt_string_char_at(ptr, 3), -1);
        assert_eq!(spectra_rt_string_char_at(ptr, -1), -1);
        assert_eq!(spectra_rt_string_len(0), 0);
        assert_eq!(spectra_rt_string_char_at(0, 0), -1);

        spectra_rt_manual_clear();
    }

    extern "C" fn host_const() -> i64 {
        42
    }

    extern "C" fn host_inc(value: i64) -> i64 {
        value + 1
    }

    extern "C" fn host_context_add(ctx: *mut SpectraHostCallContext) -> i32 {
        if ctx.is_null() {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        unsafe {
            let ctx_ref = &mut *ctx;
            if ctx_ref.arg_len != 2 || ctx_ref.args.is_null() {
                return HOST_STATUS_INVALID_ARGUMENT;
            }
            if ctx_ref.result_len != 1 || ctx_ref.results.is_null() {
                return HOST_STATUS_INVALID_ARGUMENT;
            }
            let args = std::slice::from_raw_parts(ctx_ref.args, ctx_ref.arg_len);
            let results = std::slice::from_raw_parts_mut(ctx_ref.results, ctx_ref.result_len);
            results[0] = args[0] + args[1];
            HOST_STATUS_SUCCESS
        }
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

    #[test]
    fn debug_invariants_cover_host_registry_and_manual_allocations() {
        let _lock = test_guard();
        spectra_rt_host_clear();
        spectra_rt_manual_clear();
        assert!(spectra_rt_debug_invariants_check());

        let frame = spectra_rt_manual_frame_enter();
        let ptr = spectra_rt_manual_alloc(64);
        assert!(!ptr.is_null());
        assert!(spectra_rt_debug_invariants_check());
        spectra_rt_manual_frame_exit(frame);
        assert!(spectra_rt_debug_invariants_check());

        let name = b"spectra.test.context_add";
        assert!(spectra_rt_host_register(
            name.as_ptr(),
            name.len(),
            host_context_add as *const ()
        ));
        assert!(spectra_rt_debug_invariants_check());

        spectra_rt_host_clear();
        spectra_rt_manual_clear();
    }

    #[test]
    fn host_invoke_returns_status_and_writes_results() {
        let _lock = test_guard();
        spectra_rt_host_clear();

        let name = b"spectra.test.context_add";
        assert!(spectra_rt_host_register(
            name.as_ptr(),
            name.len(),
            host_context_add as *const ()
        ));
        let args = [20, 22];
        let mut results = [0];
        let status = spectra_rt_host_invoke(
            name.as_ptr(),
            name.len(),
            args.as_ptr(),
            args.len(),
            results.as_mut_ptr(),
            results.len(),
        );
        assert_eq!(status, HOST_STATUS_SUCCESS);
        assert_eq!(results[0], 42);

        let missing = b"spectra.test.missing";
        let status = spectra_rt_host_invoke(
            missing.as_ptr(),
            missing.len(),
            args.as_ptr(),
            args.len(),
            results.as_mut_ptr(),
            results.len(),
        );
        assert_eq!(status, HOST_STATUS_NOT_FOUND);

        spectra_rt_host_clear();
    }
}

// ── Fast-path symbol retention ────────────────────────────────────────────────
//
// The `spectra_rt_*_fast` functions are not called from any Rust code: they
// are resolved at runtime by the JIT's symbol resolver (cranelift-jit calls
// `GetProcAddress` / `dlsym` against the running process image). Because
// nothing in `spectra-cli` references them, the linker treats them as dead
// code and strips them, which then causes runtime panics such as
// `can't resolve symbol spectra_rt_channel_new_fast`.
//
// Two complementary mechanisms keep every fast-path symbol alive in the
// final binary across all targets (including MSVC, which strips
// unreferenced functions even when other functions in the same TU call
// them through a `#[no_mangle]` re-export):
//
//  1. Each `pub extern "C" fn spectra_rt_*_fast` is also marked
//     `#[inline(never)]`, so the compiler emits a real function body that
//     can be addressed by the JIT.
//  2. `pub fn keep_fast_symbols` calls each fast function with safe dummy
//     inputs and discards the results. The call is invoked once at startup
//     from `crate::stdlib::register`, which itself is called from
//     `spectra_runtime::register_standard_library` in `spectra-cli`, so
//     every fast-path symbol survives dead-code elimination.
//
// To keep them across all targets (including MSVC, where `#[used]` on a
// `fn` itself is not supported and `#[used]` statics do not pull in the
// functions they reference), `pub fn keep_fast_symbols` calls each one
// with safe dummy inputs and discards the results. The call is invoked
// once at startup from `crate::stdlib::register`, which itself is called
// from `spectra_runtime::register_standard_library` in `spectra-cli`, so
// every fast-path symbol survives dead-code elimination.
//
// The functions are designed to be side-effect-safe when called with valid
// registry state: `*_new_*` allocates a fresh handle (cheap), `*_get_*` /
// `*_contains_*` / `*_len_*` are read-only, and the rest are no-ops on
// invalid handles (they return NOT_FOUND / 0). Because the handles they
// return are immediately dropped without being used again, no observable
// state changes.
#[doc(hidden)]
#[inline(never)]
pub fn keep_fast_symbols() {
    // Concurrent: spawn a task we never join, then drop the channel we open.
    let task = spectra_rt_concurrent_spawn_fast(0);
    let _ = spectra_rt_concurrent_join_fast(task);
    let channel = spectra_rt_channel_new_fast();
    let _ = spectra_rt_channel_len_fast(channel);
    let _ = spectra_rt_channel_recv_fast(channel);
    let _ = spectra_rt_channel_send_fast(channel, 0);
    let _ = spectra_rt_channel_close_fast(channel);

    // Map: create a map, write / read / check, then free.
    let m = spectra_rt_map_new_fast();
    let _ = spectra_rt_map_set_fast(m, 0, 0);
    let _ = spectra_rt_map_get_fast(m, 0);
    let _ = spectra_rt_map_contains_fast(m, 0);
    let _ = spectra_rt_map_remove_fast(m, 0);
    let _ = spectra_rt_map_len_fast(m);
    spectra_rt_map_clear_fast(m);
    spectra_rt_map_free_fast(m);

    // Tensor / ML: exercise the full pipeline with a 1-element tensor.
    let t = spectra_rt_tensor_full_f_fast(1, 0.0);
    let _ = spectra_rt_ml_linear_fast(t, t, t);
    let loss = spectra_rt_ml_mse_loss_fast(t, t);
    let _ = spectra_rt_tensor_backward_fast(loss);
    let _ = spectra_rt_ml_sgd_step_fast(t, 0.0);

    // String fast-path: handle 0 is the no-op sentinel (returns 0).
    let _ = spectra_rt_string_len_fast(0);
    let _ = spectra_rt_string_char_at_fast(0, 0);
}
