use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use std::{ptr, slice, str};

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
        if let Some(frame) = self.frames.pop() {
            if frame.id == frame_id {
                return frame.allocations;
            }
            // Unexpected ordering, restore and ignore.
            self.frames.push(frame);
        }
        Vec::new()
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

/// Begins a manual allocation frame and returns its identifier.
#[no_mangle]
pub extern "C" fn spectra_rt_manual_frame_enter() -> usize {
    let table = allocation_table();
    let mut guard = table
        .lock()
        .expect("manual allocation table mutex poisoned");
    guard.push_frame()
}

/// Ends a manual allocation frame, freeing all allocations created since it began.
#[no_mangle]
pub extern "C" fn spectra_rt_manual_frame_exit(frame_id: usize) {
    let table = allocation_table();
    let mut guard = table
        .lock()
        .expect("manual allocation table mutex poisoned");
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
        .expect("manual allocation table mutex poisoned");

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
        .expect("manual allocation table mutex poisoned");

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
        .expect("manual allocation table mutex poisoned");
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
    let mut guard = registry.lock().expect("host registry mutex poisoned");
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
    let mut guard = registry.lock().expect("host registry mutex poisoned");
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
    let guard = registry.lock().expect("host registry mutex poisoned");
    guard.lookup(&name)
}

/// Clears all registered host functions.
#[no_mangle]
pub extern "C" fn spectra_rt_host_clear() {
    let registry = host_registry();
    let mut guard = registry.lock().expect("host registry mutex poisoned");
    guard.clear();
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
