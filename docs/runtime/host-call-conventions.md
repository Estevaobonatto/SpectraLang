# Spectra Runtime Host-Call Conventions

This document defines how native host functions are exposed to JIT-compiled Spectra programs through the runtime FFI layer.

## Registration Lifecycle

1. Initialise the runtime (or allow lazy initialisation) before launching JIT code.
2. Register each host function exactly once per process by calling `spectra_rt_host_register(name_ptr, name_len, fn_ptr)` (or, from Rust, `spectra_runtime::ffi::register_host_function(name, func)`).
    - `name_ptr`/`name_len` identify the function using a UTF-8 name (for example `b"spectra.std.io.print"`).
    - `fn_ptr` must be an `extern "C"` function pointer cast to `*const ()`.
    - The call returns `true` when the name was newly inserted and `false` when an existing entry was replaced.
3. When a host function should be removed, call `spectra_rt_host_unregister` with the same name. The function returns `true` if a registration existed.
4. To reset the registry (typically in tests or process teardown) invoke `spectra_rt_host_clear`.

The registry is process-wide and protected by a mutex. No additional synchronisation is required when accessing it exclusively through the FFI helpers.

## Lookup From JIT Code

JIT code (or the backend during code generation) resolves host calls lazily:

```c
const char *name = "spectra.io.print";
const void *fn = spectra_rt_host_lookup(name, strlen(name));
if (fn == NULL) {
    /* surface an error to Spectra code */
}
```

`NULL` indicates that no function is registered under the requested name or that the name was not valid UTF-8. The backend is responsible for emitting error paths when lookups fail.

## Calling Convention

Host functions are treated as raw pointers; Spectra does not impose a fixed signature yet. For the alpha milestone we standardise on the following default signature, which matches the current backend expectations and mirrors the symbols exported from `runtime::ffi`:

```c
typedef int64_t SpectraHostValue;
typedef struct SpectraHostCallContext {
    const SpectraHostValue *args;
    size_t arg_len;
    SpectraHostValue *results;
    size_t result_len;
} SpectraHostCallContext;

typedef int32_t (*SpectraHostFn)(SpectraHostCallContext *ctx);
```

- Arguments and results are ABI-aligned 64-bit slots that mirror Spectra's primitive representation.
- The callee writes return values into `results` and returns `HOST_STATUS_SUCCESS` (`0`) on success. The runtime also exposes `HOST_STATUS_INVALID_ARGUMENT`, `HOST_STATUS_NOT_FOUND`, and `HOST_STATUS_INTERNAL_ERROR` for common failure modes.
- Host code may ignore fields it does not need, but must treat the context pointer as mutable.

Future milestones may introduce richer metadata, but compiled alpha releases must continue honouring this contract to remain compatible.

## Example: Registering a Native Function

```rust
use spectra_runtime::ffi::{
    clear_host_functions,
    register_host_function,
    SpectraHostCallContext,
    HOST_STATUS_SUCCESS,
};

extern "C" fn host_print(ctx: *mut SpectraHostCallContext) -> i32 {
    // Implementation elided – decode ctx->args and print to stdout.
    HOST_STATUS_SUCCESS
}

pub fn install_io_host_calls() {
    // Ensure a clean slate when embedding in tests.
    clear_host_functions();

    let inserted = register_host_function("spectra.std.io.print", host_print);
    assert!(inserted);
}
```

The backend can now emit a lookup for `spectra.std.io.print` and patch the resulting function pointer into the generated machine code.
