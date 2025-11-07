use std::collections::HashMap;
use std::ptr;
use std::sync::{Mutex, OnceLock};

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
