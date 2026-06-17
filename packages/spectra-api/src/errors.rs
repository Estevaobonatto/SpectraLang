use crate::{alloc_spectra_string, write_result};
use spectra_runtime::ffi::{SpectraHostCallContext, SpectraHostValue};

pub const ERROR_NONE: SpectraHostValue = 0;

pub extern "C" fn last_code(ctx: *mut SpectraHostCallContext) -> i32 {
    write_result(ctx, ERROR_NONE)
}

pub extern "C" fn last_message(ctx: *mut SpectraHostCallContext) -> i32 {
    write_result(ctx, alloc_spectra_string(""))
}
