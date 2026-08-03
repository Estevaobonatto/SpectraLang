use std::borrow::Cow;
use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex, OnceLock};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GpuBinaryOp {
    Add,
    Sub,
    Mul,
    Div,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GpuUnaryOp {
    Neg,
    Relu,
}

/// Stable kind tag for a GPU error, used by `std_tensor_stats_gpu_errors` to
/// surface per-kind counters (R-3023). The previous `Err(String)` path
/// swallowed every error into a single silent `stats_cpu_fallbacks++`
/// counter; this enum restores the diagnostic signal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GpuErrorKind {
    /// Input shape or size does not satisfy the kernel's contract
    ShapeMismatch,
    /// WGSL compilation failed
    ShaderCompile,
    /// wgpu buffer allocation failed
    BufferAlloc,
    /// wgpu command submission failed
    Dispatch,
    /// wgpu readback (map_async) failed
    Readback,
    /// Requested capability (e.g. f16) is not supported by the adapter
    FeatureUnsupported,
    /// Anything else (wgpu adapter missing, poisoned lock, etc.)
    Other,
}

impl GpuErrorKind {
    /// Stable integer code for the public API.
    pub fn code(self) -> i32 {
        match self {
            Self::ShapeMismatch => 0,
            Self::ShaderCompile => 1,
            Self::BufferAlloc => 2,
            Self::Dispatch => 3,
            Self::Readback => 4,
            Self::FeatureUnsupported => 5,
            Self::Other => 6,
        }
    }

    fn from_message(message: &str) -> Self {
        if message.contains("shape") {
            Self::ShapeMismatch
        } else if message.contains("shader") || message.contains("compil") {
            Self::ShaderCompile
        } else if message.contains("buffer") || message.contains("alloc") {
            Self::BufferAlloc
        } else if message.contains("readback") || message.contains("map") {
            Self::Readback
        } else if message.contains("feature") || message.contains("supported") {
            Self::FeatureUnsupported
        } else {
            Self::Other
        }
    }
}

/// A typed GPU error surfaced to `std_tensor_stats_gpu_errors`.
#[derive(Debug, Clone)]
pub struct GpuError {
    pub kind: GpuErrorKind,
    pub message: String,
}

impl GpuError {
    pub fn new(kind: GpuErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
        }
    }

    /// Classify a free-form message into a `GpuErrorKind`. Used by callers
    /// that only have the historical `String` payload.
    pub fn from_message(message: impl Into<String>) -> Self {
        let message = message.into();
        let kind = GpuErrorKind::from_message(&message);
        Self { kind, message }
    }
}

struct GpuContext {
    device: wgpu::Device,
    queue: wgpu::Queue,
}

static CONTEXT: OnceLock<Result<Mutex<GpuContext>, GpuError>> = OnceLock::new();

pub fn is_available() -> bool {
    context().is_ok()
}

/// Lock the GPU context and run a closure with `&wgpu::Device` and
/// `&wgpu::Queue`. Used by `to_device` to write into pool buffers; the
/// closure receives the same queue that `run_compute` uses, so an upload
/// followed by a kernel dispatch is correctly ordered.
pub fn with_device_queue<R>(
    f: impl FnOnce(&wgpu::Device, &wgpu::Queue) -> R,
) -> Result<R, GpuError> {
    let guard = context()?
        .lock()
        .map_err(|_| GpuError::new(GpuErrorKind::Other, "gpu context poisoned"))?;
    Ok(f(&guard.device, &guard.queue))
}

/// Mirror of the runtime's `TensorDevice` enum used to key the device
/// buffer pool. Only `Wgpu` is exercised by the arena today; the others
/// are reserved for future native backends (R-3201..R-3204).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PoolDevice {
    Wgpu,
}

/// Mirror of the runtime's `TensorDType` for pool keying. Only `Float`
/// is exercised today; the rest are reserved for the f16/bf16 work in
/// R-3071.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PoolDType {
    Float,
}

impl PoolDType {
    pub fn byte_size(self) -> u64 {
        match self {
            Self::Float => 4,
        }
    }
}

/// Owned, type-erased device buffer that is safe to hand back to a pool
/// when the last `Arc` is dropped. The Arc-wrapped `wgpu::Buffer` keeps
/// the GPU resource alive for any other holder while a buffer is in the
/// free list (R-3051, D1).
#[derive(Debug, Clone)]
pub struct DeviceBuffer {
    pub buffer: Arc<wgpu::Buffer>,
    pub size: u64,
    pub elements: usize,
    pub device: PoolDevice,
    pub dtype: PoolDType,
}

/// Bucket key. A buffer of bucket `B` is reusable for any `n <= B`; the
/// pool acquires it for a request of `n` and reuses the storage as long
/// as the caller writes the full `n` elements before releasing.
pub type BucketKey = (PoolDevice, PoolDType, u64);

/// Free-list pool of device buffers keyed by `(device, dtype, size_bucket)`
/// (R-3051, D2). Held inside the existing `TensorRegistry` mutex, so it
/// adds no new lock surface. The `bytes_resident` counter is the single
/// source of truth for `stats_device_pool_bytes_resident`.
///
/// Invariant for reuse safety: every `acquire` is followed by either
/// `queue.submit` (with a full data overwrite) before the buffer is
/// released back to the pool, or the buffer is dropped without being
/// pooled. See `acquire` doc-comment.
#[derive(Debug, Default)]
pub struct DeviceArena {
    free: HashMap<BucketKey, VecDeque<DeviceBuffer>>,
    hits: u64,
    misses: u64,
    bytes_resident: u64,
}

pub const MAX_FREE_PER_BUCKET: usize = 16;

fn bucket_for(elements: usize) -> u64 {
    (elements.max(16) as u64).next_power_of_two()
}

impl DeviceArena {
    pub fn new() -> Self {
        Self::default()
    }

    /// Acquire a buffer of bucket `bucket_for(n)`. Increments `hits` on
    /// pool reuse, `misses` on a fresh allocation. `MAX_FREE_PER_BUCKET`
    /// caps the free list per bucket.
    pub fn acquire(
        &mut self,
        device: PoolDevice,
        dtype: PoolDType,
        elements: usize,
        device_for_buffer: &wgpu::Device,
    ) -> DeviceBuffer {
        let bucket = bucket_for(elements);
        let key = (device, dtype, bucket);
        if let Some(list) = self.free.get_mut(&key) {
            if let Some(mut buf) = list.pop_front() {
                self.hits = self.hits.saturating_add(1);
                buf.elements = elements;
                return buf;
            }
        }
        self.misses = self.misses.saturating_add(1);
        let size = bucket * dtype.byte_size();
        let buffer = device_for_buffer.create_buffer(&wgpu::BufferDescriptor {
            label: Some("spectra-runtime-device-pool"),
            size,
            usage: wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::COPY_DST
                | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        self.bytes_resident = self.bytes_resident.saturating_add(size);
        DeviceBuffer {
            buffer: Arc::new(buffer),
            size,
            elements,
            device,
            dtype,
        }
    }

    /// Return a buffer to the pool. The free list per bucket is capped
    /// at `MAX_FREE_PER_BUCKET`; over-cap releases are dropped, which
    /// drops the `Arc` and lets wgpu reclaim the underlying buffer.
    pub fn release(&mut self, buf: DeviceBuffer) {
        let bucket = bucket_for(buf.elements);
        let key = (buf.device, buf.dtype, bucket);
        let list = self.free.entry(key).or_default();
        if list.len() >= MAX_FREE_PER_BUCKET {
            self.bytes_resident = self.bytes_resident.saturating_sub(buf.size);
            return;
        }
        list.push_back(buf);
    }

    pub fn hits(&self) -> u64 {
        self.hits
    }

    pub fn misses(&self) -> u64 {
        self.misses
    }

    pub fn bytes_resident(&self) -> u64 {
        self.bytes_resident
    }

    pub fn reset(&mut self) {
        self.free.clear();
        self.hits = 0;
        self.misses = 0;
        self.bytes_resident = 0;
    }
}

pub fn adapter_name() -> Option<String> {
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
        backends: wgpu::Backends::PRIMARY,
        ..Default::default()
    });
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::HighPerformance,
        compatible_surface: None,
        force_fallback_adapter: false,
    }))?;
    Some(adapter.get_info().name)
}

pub fn binary(left: &[f32], right: &[f32], op: GpuBinaryOp) -> Result<Vec<f32>, GpuError> {
    if left.len() != right.len() {
        return Err(GpuError::new(
            GpuErrorKind::ShapeMismatch,
            "gpu binary shape mismatch",
        ));
    }
    if left.is_empty() {
        return Ok(Vec::new());
    }
    let shader = format!(
        r#"
@group(0) @binding(0) var<storage, read> left: array<f32>;
@group(0) @binding(1) var<storage, read> right: array<f32>;
@group(0) @binding(2) var<storage, read_write> out: array<f32>;

@compute @workgroup_size(64)
fn main(@builtin(global_invocation_id) id: vec3<u32>) {{
    let i = id.x;
    if (i >= {len}u) {{
        return;
    }}
    out[i] = {expr};
}}
"#,
        len = left.len(),
        expr = match op {
            GpuBinaryOp::Add => "left[i] + right[i]",
            GpuBinaryOp::Sub => "left[i] - right[i]",
            GpuBinaryOp::Mul => "left[i] * right[i]",
            GpuBinaryOp::Div => "select(left[i] / right[i], 0.0, right[i] == 0.0)",
        }
    );
    dispatch_two_inputs(left, right, left.len(), &shader, [left.len() as u32, 1, 1])
}

pub fn unary(input: &[f32], op: GpuUnaryOp) -> Result<Vec<f32>, GpuError> {
    if input.is_empty() {
        return Ok(Vec::new());
    }
    let shader = format!(
        r#"
@group(0) @binding(0) var<storage, read> input_values: array<f32>;
@group(0) @binding(1) var<storage, read_write> out: array<f32>;

@compute @workgroup_size(64)
fn main(@builtin(global_invocation_id) id: vec3<u32>) {{
    let i = id.x;
    if (i >= {len}u) {{
        return;
    }}
    out[i] = {expr};
}}
"#,
        len = input.len(),
        expr = match op {
            GpuUnaryOp::Neg => "-input_values[i]",
            GpuUnaryOp::Relu => "max(input_values[i], 0.0)",
        }
    );
    dispatch_one_input(input, input.len(), &shader, [input.len() as u32, 1, 1])
}

pub fn sum(input: &[f32]) -> Result<f32, GpuError> {
    if input.is_empty() {
        return Err(GpuError::new(
            GpuErrorKind::ShapeMismatch,
            "gpu reduction requires at least one element",
        ));
    }
    let shader = format!(
        r#"
@group(0) @binding(0) var<storage, read> input_values: array<f32>;
@group(0) @binding(1) var<storage, read_write> out: array<f32>;

@compute @workgroup_size(1)
fn main() {{
    var acc = 0.0;
    for (var i = 0u; i < {len}u; i = i + 1u) {{
        acc = acc + input_values[i];
    }}
    out[0] = acc;
}}
"#,
        len = input.len()
    );
    dispatch_one_input(input, 1, &shader, [1, 1, 1]).map(|values| values[0])
}

pub fn matmul(
    left: &[f32],
    right: &[f32],
    m: usize,
    k: usize,
    n: usize,
) -> Result<Vec<f32>, GpuError> {
    if left.len() != m.saturating_mul(k) || right.len() != k.saturating_mul(n) {
        return Err(GpuError::new(
            GpuErrorKind::ShapeMismatch,
            "gpu matmul shape mismatch",
        ));
    }
    let shader = format!(
        r#"
@group(0) @binding(0) var<storage, read> left: array<f32>;
@group(0) @binding(1) var<storage, read> right: array<f32>;
@group(0) @binding(2) var<storage, read_write> out: array<f32>;

@compute @workgroup_size(64)
fn main(@builtin(global_invocation_id) id: vec3<u32>) {{
    let index = id.x;
    if (index >= {out_len}u) {{
        return;
    }}
    let row = index / {n}u;
    let col = index % {n}u;
    var acc = 0.0;
    for (var inner = 0u; inner < {k}u; inner = inner + 1u) {{
        acc = acc + left[row * {k}u + inner] * right[inner * {n}u + col];
    }}
    out[index] = acc;
}}
"#,
        k = k,
        n = n,
        out_len = m * n
    );
    dispatch_two_inputs(left, right, m * n, &shader, [(m * n) as u32, 1, 1])
}

pub fn conv2d(
    input: &[f32],
    kernel: &[f32],
    bias: &[f32],
    dims: [usize; 7],
) -> Result<Vec<f32>, GpuError> {
    let [batch, in_ch, h, w, out_ch, kh, kw] = dims;
    if h < kh || w < kw {
        return Err(GpuError::new(
            GpuErrorKind::ShapeMismatch,
            "gpu conv2d invalid kernel dimensions",
        ));
    }
    if input.len() != batch * in_ch * h * w
        || kernel.len() != out_ch * in_ch * kh * kw
        || bias.len() != out_ch
    {
        return Err(GpuError::new(
            GpuErrorKind::ShapeMismatch,
            "gpu conv2d shape mismatch",
        ));
    }
    let out_h = h - kh + 1;
    let out_w = w - kw + 1;
    let out_len = batch * out_ch * out_h * out_w;
    let shader = format!(
        r#"
@group(0) @binding(0) var<storage, read> input_values: array<f32>;
@group(0) @binding(1) var<storage, read> kernel_values: array<f32>;
@group(0) @binding(2) var<storage, read> bias_values: array<f32>;
@group(0) @binding(3) var<storage, read_write> out: array<f32>;

@compute @workgroup_size(64)
fn main(@builtin(global_invocation_id) id: vec3<u32>) {{
    let index = id.x;
    if (index >= {out_len}u) {{
        return;
    }}
    let ox = index % {out_w}u;
    let oy = (index / {out_w}u) % {out_h}u;
    let oc = (index / ({out_w}u * {out_h}u)) % {out_ch}u;
    let n = index / ({out_w}u * {out_h}u * {out_ch}u);
    var acc = bias_values[oc];
    for (var ic = 0u; ic < {in_ch}u; ic = ic + 1u) {{
        for (var ky = 0u; ky < {kh}u; ky = ky + 1u) {{
            for (var kx = 0u; kx < {kw}u; kx = kx + 1u) {{
                let input_idx = ((n * {in_ch}u + ic) * {h}u + oy + ky) * {w}u + ox + kx;
                let kernel_idx = ((oc * {in_ch}u + ic) * {kh}u + ky) * {kw}u + kx;
                acc = acc + input_values[input_idx] * kernel_values[kernel_idx];
            }}
        }}
    }}
    out[index] = acc;
}}
"#,
        in_ch = in_ch,
        h = h,
        w = w,
        out_ch = out_ch,
        kh = kh,
        kw = kw,
        out_h = out_h,
        out_w = out_w,
        out_len = out_len
    );
    dispatch_three_inputs(
        input,
        kernel,
        bias,
        out_len,
        &shader,
        [out_len as u32, 1, 1],
    )
}

// ===== R-3080: GPU backward kernels =====
//
// Each `backward_*` returns a freshly-allocated `Vec<f32>` in row-major
// layout that matches the equivalent CPU implementation in
// `runtime/src/stdlib/mod.rs::autograd_parent_grads`. The CPU path is
// the source of truth for tolerance; tests in
// `tensor_runtime_r1603_backward_*` cross-check both paths via
// finite differences.

/// `out[i, j] = sum_l grad[i, l] * right[j, l]` for `C = A @ B`.
/// Equivalent to `grad @ right.T` in row-major. Output length `m*k`.
pub fn backward_matmul_left(
    grad: &[f32],
    right: &[f32],
    m: usize,
    k: usize,
    n: usize,
) -> Result<Vec<f32>, GpuError> {
    if grad.len() != m.saturating_mul(n) || right.len() != k.saturating_mul(n) {
        return Err(GpuError::new(
            GpuErrorKind::ShapeMismatch,
            "gpu backward_matmul_left shape mismatch",
        ));
    }
    if m == 0 || k == 0 {
        return Ok(Vec::new());
    }
    let shader = format!(
        r#"
@group(0) @binding(0) var<storage, read> grad_values: array<f32>;
@group(0) @binding(1) var<storage, read> right_values: array<f32>;
@group(0) @binding(2) var<storage, read_write> out: array<f32>;

@compute @workgroup_size(64)
fn main(@builtin(global_invocation_id) id: vec3<u32>) {{
    let index = id.x;
    let total = {m}u * {k}u;
    if (index >= total) {{
        return;
    }}
    let i = index / {k}u;
    let j = index % {k}u;
    var acc = 0.0;
    for (var l = 0u; l < {n}u; l = l + 1u) {{
        acc = acc + grad_values[i * {n}u + l] * right_values[j * {n}u + l];
    }}
    out[index] = acc;
}}
"#,
        m = m,
        k = k,
        n = n
    );
    dispatch_two_inputs(grad, right, m * k, &shader, [(m * k) as u32, 1, 1])
}

/// `out[j, l] = sum_i left[i, j] * grad[i, l]` for `C = A @ B`.
/// Equivalent to `left.T @ grad` in row-major. Output length `k*n`.
pub fn backward_matmul_right(
    left: &[f32],
    grad: &[f32],
    m: usize,
    k: usize,
    n: usize,
) -> Result<Vec<f32>, GpuError> {
    if left.len() != m.saturating_mul(k) || grad.len() != m.saturating_mul(n) {
        return Err(GpuError::new(
            GpuErrorKind::ShapeMismatch,
            "gpu backward_matmul_right shape mismatch",
        ));
    }
    if k == 0 || n == 0 {
        return Ok(Vec::new());
    }
    let shader = format!(
        r#"
@group(0) @binding(0) var<storage, read> left_values: array<f32>;
@group(0) @binding(1) var<storage, read> grad_values: array<f32>;
@group(0) @binding(2) var<storage, read_write> out: array<f32>;

@compute @workgroup_size(64)
fn main(@builtin(global_invocation_id) id: vec3<u32>) {{
    let index = id.x;
    let total = {k}u * {n}u;
    if (index >= total) {{
        return;
    }}
    let j = index / {n}u;
    let l = index % {n}u;
    var acc = 0.0;
    for (var i = 0u; i < {m}u; i = i + 1u) {{
        acc = acc + left_values[i * {k}u + j] * grad_values[i * {n}u + l];
    }}
    out[index] = acc;
}}
"#,
        m = m,
        k = k,
        n = n
    );
    dispatch_two_inputs(left, grad, k * n, &shader, [(k * n) as u32, 1, 1])
}

/// `out[i] = grad[i] if output[i] > 0 else 0` for `output = relu(input)`.
/// Output length `n`; `output` carries the post-activation values.
pub fn backward_relu(grad: &[f32], output: &[f32]) -> Result<Vec<f32>, GpuError> {
    if grad.len() != output.len() {
        return Err(GpuError::new(
            GpuErrorKind::ShapeMismatch,
            "gpu backward_relu shape mismatch",
        ));
    }
    if grad.is_empty() {
        return Ok(Vec::new());
    }
    let shader = format!(
        r#"
@group(0) @binding(0) var<storage, read> grad_values: array<f32>;
@group(0) @binding(1) var<storage, read> output_values: array<f32>;
@group(0) @binding(2) var<storage, read_write> out: array<f32>;

@compute @workgroup_size(64)
fn main(@builtin(global_invocation_id) id: vec3<u32>) {{
    let i = id.x;
    if (i >= {len}u) {{
        return;
    }}
    out[i] = select(0.0, grad_values[i], output_values[i] > 0.0);
}}
"#,
        len = grad.len()
    );
    dispatch_two_inputs(grad, output, grad.len(), &shader, [grad.len() as u32, 1, 1])
}

/// Backward pass for `out = sigmoid(input)` (autograd uses
/// `grad * out * (1 - out)`). Kept here because the forward path in
/// `autograd_parent_grads` for `Sigmoid` references a host
/// implementation; when a `Sigmoid` forward is added to the GPU
/// surface, the backward can be reused as-is.
pub fn backward_sigmoid(grad: &[f32], output: &[f32]) -> Result<Vec<f32>, GpuError> {
    if grad.len() != output.len() {
        return Err(GpuError::new(
            GpuErrorKind::ShapeMismatch,
            "gpu backward_sigmoid shape mismatch",
        ));
    }
    if grad.is_empty() {
        return Ok(Vec::new());
    }
    let shader = format!(
        r#"
@group(0) @binding(0) var<storage, read> grad_values: array<f32>;
@group(0) @binding(1) var<storage, read> output_values: array<f32>;
@group(0) @binding(2) var<storage, read_write> out: array<f32>;

@compute @workgroup_size(64)
fn main(@builtin(global_invocation_id) id: vec3<u32>) {{
    let i = id.x;
    if (i >= {len}u) {{
        return;
    }}
    out[i] = grad_values[i] * output_values[i] * (1.0 - output_values[i]);
}}
"#,
        len = grad.len()
    );
    dispatch_two_inputs(grad, output, grad.len(), &shader, [grad.len() as u32, 1, 1])
}

// ===== R-3052 full: device-buffer dispatchers (no readback in hot path) =====
//
// Each `*_device` variant binds pre-acquired `DeviceBuffer`s from the
// pool, runs the same WGSL as the host-materializing counterpart, and
// submits without copy-to-readback. The caller owns the output buffer
// and is responsible for storing it on the new tensor's
// `device_storage`. Acquire happens inside the `with_device_queue`
// closure so the GPU context lock is held for the whole sequence.

/// Device-binary: `out[i] = left[i] <op> right[i]`. Inputs and output
/// are pool buffers; no host readback.
pub fn binary_device(
    left: &DeviceBuffer,
    right: &DeviceBuffer,
    op: GpuBinaryOp,
    out: &DeviceBuffer,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
) -> Result<(), GpuError> {
    if left.elements != right.elements || out.elements != left.elements {
        return Err(GpuError::new(
            GpuErrorKind::ShapeMismatch,
            "gpu binary_device shape mismatch",
        ));
    }
    let len = left.elements;
    let shader = format!(
        r#"
@group(0) @binding(0) var<storage, read> left: array<f32>;
@group(0) @binding(1) var<storage, read> right: array<f32>;
@group(0) @binding(2) var<storage, read_write> out: array<f32>;

@compute @workgroup_size(64)
fn main(@builtin(global_invocation_id) id: vec3<u32>) {{
    let i = id.x;
    if (i >= {len}u) {{
        return;
    }}
    out[i] = {expr};
}}
"#,
        len = len,
        expr = match op {
            GpuBinaryOp::Add => "left[i] + right[i]",
            GpuBinaryOp::Sub => "left[i] - right[i]",
            GpuBinaryOp::Mul => "left[i] * right[i]",
            GpuBinaryOp::Div => "select(left[i] / right[i], 0.0, right[i] == 0.0)",
        }
    );
    run_compute_no_readback(
        device,
        queue,
        &shader,
        &[
            (0, &left.buffer, true),
            (1, &right.buffer, true),
            (2, &out.buffer, false),
        ],
        [len as u32, 1, 1],
    )
}

/// Device-unary: `out[i] = <op>(input[i])`. Inputs and output are pool
/// buffers; no host readback.
pub fn unary_device(
    input: &DeviceBuffer,
    op: GpuUnaryOp,
    out: &DeviceBuffer,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
) -> Result<(), GpuError> {
    if out.elements != input.elements {
        return Err(GpuError::new(
            GpuErrorKind::ShapeMismatch,
            "gpu unary_device shape mismatch",
        ));
    }
    let len = input.elements;
    let shader = format!(
        r#"
@group(0) @binding(0) var<storage, read> input_values: array<f32>;
@group(0) @binding(1) var<storage, read_write> out: array<f32>;

@compute @workgroup_size(64)
fn main(@builtin(global_invocation_id) id: vec3<u32>) {{
    let i = id.x;
    if (i >= {len}u) {{
        return;
    }}
    out[i] = {expr};
}}
"#,
        len = len,
        expr = match op {
            GpuUnaryOp::Neg => "-input_values[i]",
            GpuUnaryOp::Relu => "max(input_values[i], 0.0)",
        }
    );
    run_compute_no_readback(
        device,
        queue,
        &shader,
        &[(0, &input.buffer, true), (1, &out.buffer, false)],
        [len as u32, 1, 1],
    )
}

/// Device-matmul: `out[i, j] = sum_k left[i, k] * right[k, j]`. Pool
/// buffers in and out; no host readback.
pub fn matmul_device(
    left: &DeviceBuffer,
    right: &DeviceBuffer,
    m: usize,
    k: usize,
    n: usize,
    out: &DeviceBuffer,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
) -> Result<(), GpuError> {
    if left.elements != m.saturating_mul(k)
        || right.elements != k.saturating_mul(n)
        || out.elements != m.saturating_mul(n)
    {
        return Err(GpuError::new(
            GpuErrorKind::ShapeMismatch,
            "gpu matmul_device shape mismatch",
        ));
    }
    let shader = format!(
        r#"
@group(0) @binding(0) var<storage, read> left: array<f32>;
@group(0) @binding(1) var<storage, read> right: array<f32>;
@group(0) @binding(2) var<storage, read_write> out: array<f32>;

@compute @workgroup_size(64)
fn main(@builtin(global_invocation_id) id: vec3<u32>) {{
    let index = id.x;
    if (index >= {out_len}u) {{
        return;
    }}
    let row = index / {n}u;
    let col = index % {n}u;
    var acc = 0.0;
    for (var inner = 0u; inner < {k}u; inner = inner + 1u) {{
        acc = acc + left[row * {k}u + inner] * right[inner * {n}u + col];
    }}
    out[index] = acc;
}}
"#,
        k = k,
        n = n,
        out_len = m * n
    );
    run_compute_no_readback(
        device,
        queue,
        &shader,
        &[
            (0, &left.buffer, true),
            (1, &right.buffer, true),
            (2, &out.buffer, false),
        ],
        [(m * n) as u32, 1, 1],
    )
}

/// Device-conv2d: standard 2D convolution with bias. Pool buffers in
/// and out; no host readback.
pub fn conv2d_device(
    input: &DeviceBuffer,
    kernel: &DeviceBuffer,
    bias: &DeviceBuffer,
    dims: [usize; 7],
    out: &DeviceBuffer,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
) -> Result<(), GpuError> {
    let [batch, in_ch, h, w, out_ch, kh, kw] = dims;
    if h < kh || w < kw {
        return Err(GpuError::new(
            GpuErrorKind::ShapeMismatch,
            "gpu conv2d_device invalid kernel dimensions",
        ));
    }
    let out_h = h - kh + 1;
    let out_w = w - kw + 1;
    let out_len = batch * out_ch * out_h * out_w;
    if input.elements != batch * in_ch * h * w
        || kernel.elements != out_ch * in_ch * kh * kw
        || bias.elements != out_ch
        || out.elements != out_len
    {
        return Err(GpuError::new(
            GpuErrorKind::ShapeMismatch,
            "gpu conv2d_device shape mismatch",
        ));
    }
    let shader = format!(
        r#"
@group(0) @binding(0) var<storage, read> input_values: array<f32>;
@group(0) @binding(1) var<storage, read> kernel_values: array<f32>;
@group(0) @binding(2) var<storage, read> bias_values: array<f32>;
@group(0) @binding(3) var<storage, read_write> out: array<f32>;

@compute @workgroup_size(64)
fn main(@builtin(global_invocation_id) id: vec3<u32>) {{
    let index = id.x;
    if (index >= {out_len}u) {{
        return;
    }}
    let ox = index % {out_w}u;
    let oy = (index / {out_w}u) % {out_h}u;
    let oc = (index / ({out_w}u * {out_h}u)) % {out_ch}u;
    let n = index / ({out_w}u * {out_h}u * {out_ch}u);
    var acc = bias_values[oc];
    for (var ic = 0u; ic < {in_ch}u; ic = ic + 1u) {{
        for (var ky = 0u; ky < {kh}u; ky = ky + 1u) {{
            for (var kx = 0u; kx < {kw}u; kx = kx + 1u) {{
                let input_idx = ((n * {in_ch}u + ic) * {h}u + oy + ky) * {w}u + ox + kx;
                let kernel_idx = ((oc * {in_ch}u + ic) * {kh}u + ky) * {kw}u + kx;
                acc = acc + input_values[input_idx] * kernel_values[kernel_idx];
            }}
        }}
    }}
    out[index] = acc;
}}
"#,
        in_ch = in_ch,
        h = h,
        w = w,
        out_ch = out_ch,
        kh = kh,
        kw = kw,
        out_h = out_h,
        out_w = out_w,
        out_len = out_len
    );
    run_compute_no_readback(
        device,
        queue,
        &shader,
        &[
            (0, &input.buffer, true),
            (1, &kernel.buffer, true),
            (2, &bias.buffer, true),
            (3, &out.buffer, false),
        ],
        [out_len as u32, 1, 1],
    )
}

/// Device-sum: writes 1 f32 into `out`. Caller is expected to read it
/// back as the loss scalar (the only allowed readback in the hot path).
pub fn sum_device(
    input: &DeviceBuffer,
    out: &DeviceBuffer,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
) -> Result<(), GpuError> {
    if input.elements == 0 || out.elements != 1 {
        return Err(GpuError::new(
            GpuErrorKind::ShapeMismatch,
            "gpu sum_device shape mismatch",
        ));
    }
    let len = input.elements;
    let shader = format!(
        r#"
@group(0) @binding(0) var<storage, read> input_values: array<f32>;
@group(0) @binding(1) var<storage, read_write> out: array<f32>;

@compute @workgroup_size(1)
fn main() {{
    var acc = 0.0;
    for (var i = 0u; i < {len}u; i = i + 1u) {{
        acc = acc + input_values[i];
    }}
    out[0] = acc;
}}
"#,
        len = len
    );
    run_compute_no_readback(
        device,
        queue,
        &shader,
        &[(0, &input.buffer, true), (1, &out.buffer, false)],
        [1, 1, 1],
    )
}

/// Device column sum: for a row-major matrix `input[rows, cols]`, writes
/// `out[col] = sum_row input[row, col]`. Used for `ml.linear` bias grads.
pub fn sum_columns_device(
    input: &DeviceBuffer,
    rows: usize,
    cols: usize,
    out: &DeviceBuffer,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
) -> Result<(), GpuError> {
    if rows == 0 || cols == 0 || input.elements != rows.saturating_mul(cols) || out.elements != cols
    {
        return Err(GpuError::new(
            GpuErrorKind::ShapeMismatch,
            "gpu sum_columns_device shape mismatch",
        ));
    }
    let shader = format!(
        r#"
@group(0) @binding(0) var<storage, read> input_values: array<f32>;
@group(0) @binding(1) var<storage, read_write> out: array<f32>;

@compute @workgroup_size(64)
fn main(@builtin(global_invocation_id) id: vec3<u32>) {{
    let col = id.x;
    if (col >= {cols}u) {{
        return;
    }}
    var acc = 0.0;
    for (var row = 0u; row < {rows}u; row = row + 1u) {{
        acc = acc + input_values[row * {cols}u + col];
    }}
    out[col] = acc;
}}
"#,
        rows = rows,
        cols = cols
    );
    run_compute_no_readback(
        device,
        queue,
        &shader,
        &[(0, &input.buffer, true), (1, &out.buffer, false)],
        [cols as u32, 1, 1],
    )
}

/// Device MSE loss: writes one scalar `mean((prediction-target)^2)` into `out`.
pub fn mse_loss_device(
    prediction: &DeviceBuffer,
    target: &DeviceBuffer,
    out: &DeviceBuffer,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
) -> Result<(), GpuError> {
    if prediction.elements == 0 || prediction.elements != target.elements || out.elements != 1 {
        return Err(GpuError::new(
            GpuErrorKind::ShapeMismatch,
            "gpu mse_loss_device shape mismatch",
        ));
    }
    let len = prediction.elements;
    let shader = format!(
        r#"
@group(0) @binding(0) var<storage, read> prediction: array<f32>;
@group(0) @binding(1) var<storage, read> target_values: array<f32>;
@group(0) @binding(2) var<storage, read_write> out: array<f32>;

@compute @workgroup_size(1)
fn main() {{
    var acc = 0.0;
    for (var i = 0u; i < {len}u; i = i + 1u) {{
        let diff = prediction[i] - target_values[i];
        acc = acc + diff * diff;
    }}
    out[0] = acc / f32({len}u);
}}
"#,
        len = len
    );
    run_compute_no_readback(
        device,
        queue,
        &shader,
        &[
            (0, &prediction.buffer, true),
            (1, &target.buffer, true),
            (2, &out.buffer, false),
        ],
        [1, 1, 1],
    )
}

/// Device MSE backward: `out[i] = seed * 2 * (prediction[i]-target[i]) / len`.
pub fn mse_backward_device(
    prediction: &DeviceBuffer,
    target: &DeviceBuffer,
    seed: &DeviceBuffer,
    out: &DeviceBuffer,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
) -> Result<(), GpuError> {
    if prediction.elements == 0
        || prediction.elements != target.elements
        || seed.elements != 1
        || out.elements != prediction.elements
    {
        return Err(GpuError::new(
            GpuErrorKind::ShapeMismatch,
            "gpu mse_backward_device shape mismatch",
        ));
    }
    let len = prediction.elements;
    let shader = format!(
        r#"
@group(0) @binding(0) var<storage, read> prediction: array<f32>;
@group(0) @binding(1) var<storage, read> target_values: array<f32>;
@group(0) @binding(2) var<storage, read> seed: array<f32>;
@group(0) @binding(3) var<storage, read_write> out: array<f32>;

@compute @workgroup_size(64)
fn main(@builtin(global_invocation_id) id: vec3<u32>) {{
    let i = id.x;
    if (i >= {len}u) {{
        return;
    }}
    out[i] = seed[0] * 2.0 * (prediction[i] - target_values[i]) / f32({len}u);
}}
"#,
        len = len
    );
    run_compute_no_readback(
        device,
        queue,
        &shader,
        &[
            (0, &prediction.buffer, true),
            (1, &target.buffer, true),
            (2, &seed.buffer, true),
            (3, &out.buffer, false),
        ],
        [len as u32, 1, 1],
    )
}

/// Device add-bias: `out[i, j] += bias[j]`. Used by `ml.linear` to add
/// the bias in-place on the matmul output buffer. Pool buffers in and
/// out.
pub fn add_bias_device(
    matmul_out: &DeviceBuffer,
    bias: &DeviceBuffer,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
) -> Result<(), GpuError> {
    if matmul_out.elements == 0 || bias.elements == 0 || matmul_out.elements % bias.elements != 0 {
        return Err(GpuError::new(
            GpuErrorKind::ShapeMismatch,
            "gpu add_bias_device shape mismatch",
        ));
    }
    let total = matmul_out.elements;
    let cols = bias.elements;
    let shader = format!(
        r#"
@group(0) @binding(0) var<storage, read> bias_values: array<f32>;
@group(0) @binding(1) var<storage, read_write> out: array<f32>;

@compute @workgroup_size(64)
fn main(@builtin(global_invocation_id) id: vec3<u32>) {{
    let i = id.x;
    if (i >= {total}u) {{
        return;
    }}
    out[i] = out[i] + bias_values[i % {cols}u];
}}
"#,
        total = total,
        cols = cols
    );
    run_compute_no_readback(
        device,
        queue,
        &shader,
        &[(0, &bias.buffer, true), (1, &matmul_out.buffer, false)],
        [total as u32, 1, 1],
    )
}

/// Device add-into: `dst[i] += src[i]`. Used for device-grad
/// accumulation in the autograd path.
pub fn add_into_device(
    dst: &DeviceBuffer,
    src: &DeviceBuffer,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
) -> Result<(), GpuError> {
    if dst.elements != src.elements {
        return Err(GpuError::new(
            GpuErrorKind::ShapeMismatch,
            "gpu add_into_device shape mismatch",
        ));
    }
    let len = dst.elements;
    let shader = format!(
        r#"
@group(0) @binding(0) var<storage, read> src: array<f32>;
@group(0) @binding(1) var<storage, read_write> dst: array<f32>;

@compute @workgroup_size(64)
fn main(@builtin(global_invocation_id) id: vec3<u32>) {{
    let i = id.x;
    if (i >= {len}u) {{
        return;
    }}
    dst[i] = dst[i] + src[i];
}}
"#,
        len = len
    );
    run_compute_no_readback(
        device,
        queue,
        &shader,
        &[(0, &src.buffer, true), (1, &dst.buffer, false)],
        [len as u32, 1, 1],
    )
}

/// Device SGD update: `out[i] = param[i] - lr * grad[i]`. The caller
/// typically passes `out == param` for an in-place overwrite; the
/// underlying `Arc<wgpu::Buffer>` is kept alive so any other holder
/// sees the updated values after the next submit.
pub fn sgd_update_device(
    param: &DeviceBuffer,
    grad: &DeviceBuffer,
    lr: f32,
    out: &DeviceBuffer,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
) -> Result<(), GpuError> {
    if param.elements != grad.elements || out.elements != param.elements {
        return Err(GpuError::new(
            GpuErrorKind::ShapeMismatch,
            "gpu sgd_update_device shape mismatch",
        ));
    }
    let len = param.elements;
    if Arc::ptr_eq(&param.buffer, &out.buffer) {
        let shader = format!(
            r#"
@group(0) @binding(0) var<storage, read_write> param: array<f32>;
@group(0) @binding(1) var<storage, read> grad: array<f32>;

@compute @workgroup_size(64)
fn main(@builtin(global_invocation_id) id: vec3<u32>) {{
    let i = id.x;
    if (i >= {len}u) {{
        return;
    }}
    param[i] = param[i] - {lr}f * grad[i];
}}
"#,
            len = len,
            lr = lr
        );
        return run_compute_no_readback(
            device,
            queue,
            &shader,
            &[(0, &param.buffer, false), (1, &grad.buffer, true)],
            [len as u32, 1, 1],
        );
    }
    let shader = format!(
        r#"
@group(0) @binding(0) var<storage, read> param: array<f32>;
@group(0) @binding(1) var<storage, read> grad: array<f32>;
@group(0) @binding(2) var<storage, read_write> out: array<f32>;

@compute @workgroup_size(64)
fn main(@builtin(global_invocation_id) id: vec3<u32>) {{
    let i = id.x;
    if (i >= {len}u) {{
        return;
    }}
    out[i] = param[i] - {lr}f * grad[i];
}}
"#,
        len = len,
        lr = lr
    );
    run_compute_no_readback(
        device,
        queue,
        &shader,
        &[
            (0, &param.buffer, true),
            (1, &grad.buffer, true),
            (2, &out.buffer, false),
        ],
        [len as u32, 1, 1],
    )
}

/// Device backward matmul-left: `out[i, j] = sum_l grad[i, l] * right[j, l]`
/// for `C = A @ B`. Pool buffers; no readback.
pub fn backward_matmul_left_device(
    grad: &DeviceBuffer,
    right: &DeviceBuffer,
    m: usize,
    k: usize,
    n: usize,
    out: &DeviceBuffer,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
) -> Result<(), GpuError> {
    if grad.elements != m.saturating_mul(n)
        || right.elements != k.saturating_mul(n)
        || out.elements != m.saturating_mul(k)
    {
        return Err(GpuError::new(
            GpuErrorKind::ShapeMismatch,
            "gpu backward_matmul_left_device shape mismatch",
        ));
    }
    if m == 0 || k == 0 {
        return Ok(());
    }
    let shader = format!(
        r#"
@group(0) @binding(0) var<storage, read> grad_values: array<f32>;
@group(0) @binding(1) var<storage, read> right_values: array<f32>;
@group(0) @binding(2) var<storage, read_write> out: array<f32>;

@compute @workgroup_size(64)
fn main(@builtin(global_invocation_id) id: vec3<u32>) {{
    let index = id.x;
    let total = {m}u * {k}u;
    if (index >= total) {{
        return;
    }}
    let i = index / {k}u;
    let j = index % {k}u;
    var acc = 0.0;
    for (var l = 0u; l < {n}u; l = l + 1u) {{
        acc = acc + grad_values[i * {n}u + l] * right_values[j * {n}u + l];
    }}
    out[index] = acc;
}}
"#,
        m = m,
        k = k,
        n = n
    );
    run_compute_no_readback(
        device,
        queue,
        &shader,
        &[
            (0, &grad.buffer, true),
            (1, &right.buffer, true),
            (2, &out.buffer, false),
        ],
        [(m * k) as u32, 1, 1],
    )
}

/// Device backward matmul-right: `out[j, l] = sum_i left[i, j] * grad[i, l]`
/// for `C = A @ B`. Pool buffers; no readback.
pub fn backward_matmul_right_device(
    left: &DeviceBuffer,
    grad: &DeviceBuffer,
    m: usize,
    k: usize,
    n: usize,
    out: &DeviceBuffer,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
) -> Result<(), GpuError> {
    if left.elements != m.saturating_mul(k)
        || grad.elements != m.saturating_mul(n)
        || out.elements != k.saturating_mul(n)
    {
        return Err(GpuError::new(
            GpuErrorKind::ShapeMismatch,
            "gpu backward_matmul_right_device shape mismatch",
        ));
    }
    if k == 0 || n == 0 {
        return Ok(());
    }
    let shader = format!(
        r#"
@group(0) @binding(0) var<storage, read> left_values: array<f32>;
@group(0) @binding(1) var<storage, read> grad_values: array<f32>;
@group(0) @binding(2) var<storage, read_write> out: array<f32>;

@compute @workgroup_size(64)
fn main(@builtin(global_invocation_id) id: vec3<u32>) {{
    let index = id.x;
    let total = {k}u * {n}u;
    if (index >= total) {{
        return;
    }}
    let j = index / {n}u;
    let l = index % {n}u;
    var acc = 0.0;
    for (var i = 0u; i < {m}u; i = i + 1u) {{
        acc = acc + left_values[i * {k}u + j] * grad_values[i * {n}u + l];
    }}
    out[index] = acc;
}}
"#,
        m = m,
        k = k,
        n = n
    );
    run_compute_no_readback(
        device,
        queue,
        &shader,
        &[
            (0, &left.buffer, true),
            (1, &grad.buffer, true),
            (2, &out.buffer, false),
        ],
        [(k * n) as u32, 1, 1],
    )
}

/// Device ML linear input grad: for `Y = X[batch,in] @ W[in,out] + b`,
/// writes `out_grad_input[batch,in] = grad[batch,out] @ W^T` while reading
/// `W` in its normal row-major `[in,out]` layout.
pub fn linear_grad_input_device(
    grad: &DeviceBuffer,
    weight: &DeviceBuffer,
    batch: usize,
    in_features: usize,
    out_features: usize,
    out: &DeviceBuffer,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
) -> Result<(), GpuError> {
    if grad.elements != batch.saturating_mul(out_features)
        || weight.elements != in_features.saturating_mul(out_features)
        || out.elements != batch.saturating_mul(in_features)
    {
        return Err(GpuError::new(
            GpuErrorKind::ShapeMismatch,
            "gpu linear_grad_input_device shape mismatch",
        ));
    }
    let shader = format!(
        r#"
@group(0) @binding(0) var<storage, read> grad_values: array<f32>;
@group(0) @binding(1) var<storage, read> weight_values: array<f32>;
@group(0) @binding(2) var<storage, read_write> out: array<f32>;

@compute @workgroup_size(64)
fn main(@builtin(global_invocation_id) id: vec3<u32>) {{
    let index = id.x;
    let total = {batch}u * {in_features}u;
    if (index >= total) {{
        return;
    }}
    let row = index / {in_features}u;
    let in_col = index % {in_features}u;
    var acc = 0.0;
    for (var out_col = 0u; out_col < {out_features}u; out_col = out_col + 1u) {{
        acc = acc + grad_values[row * {out_features}u + out_col] * weight_values[in_col * {out_features}u + out_col];
    }}
    out[index] = acc;
}}
"#,
        batch = batch,
        in_features = in_features,
        out_features = out_features
    );
    run_compute_no_readback(
        device,
        queue,
        &shader,
        &[
            (0, &grad.buffer, true),
            (1, &weight.buffer, true),
            (2, &out.buffer, false),
        ],
        [(batch * in_features) as u32, 1, 1],
    )
}

/// Device backward relu: `out[i] = grad[i] if output[i] > 0 else 0`.
/// Pool buffers; no readback.
pub fn backward_relu_device(
    grad: &DeviceBuffer,
    output: &DeviceBuffer,
    out: &DeviceBuffer,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
) -> Result<(), GpuError> {
    if grad.elements != output.elements || out.elements != grad.elements {
        return Err(GpuError::new(
            GpuErrorKind::ShapeMismatch,
            "gpu backward_relu_device shape mismatch",
        ));
    }
    if grad.elements == 0 {
        return Ok(());
    }
    let len = grad.elements;
    let shader = format!(
        r#"
@group(0) @binding(0) var<storage, read> grad_values: array<f32>;
@group(0) @binding(1) var<storage, read> output_values: array<f32>;
@group(0) @binding(2) var<storage, read_write> out: array<f32>;

@compute @workgroup_size(64)
fn main(@builtin(global_invocation_id) id: vec3<u32>) {{
    let i = id.x;
    if (i >= {len}u) {{
        return;
    }}
    out[i] = select(0.0, grad_values[i], output_values[i] > 0.0);
}}
"#,
        len = len
    );
    run_compute_no_readback(
        device,
        queue,
        &shader,
        &[
            (0, &grad.buffer, true),
            (1, &output.buffer, true),
            (2, &out.buffer, false),
        ],
        [len as u32, 1, 1],
    )
}

/// Upload a 1-element f32 to a 1-element pool buffer. Used to promote
/// the host scalar backward seed (the loss gradient) to a device
/// buffer so the GPU backward path can consume it without a per-step
/// readback in the chained backward.
pub fn upload_scalar_device(
    value: f32,
    out: &DeviceBuffer,
    _device: &wgpu::Device,
    queue: &wgpu::Queue,
) -> Result<(), GpuError> {
    if out.elements != 1 {
        return Err(GpuError::new(
            GpuErrorKind::ShapeMismatch,
            "gpu upload_scalar_device requires a 1-element buffer",
        ));
    }
    queue.write_buffer(&out.buffer, 0, bytemuck::cast_slice(&[value]));
    Ok(())
}

/// Read back a 1-element pool buffer as an f32. Used by the GPU sum
/// path and by tests that need to inspect a device scalar.
pub fn readback_scalar_device(
    buf: &DeviceBuffer,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
) -> Result<f32, GpuError> {
    if buf.elements != 1 {
        return Err(GpuError::new(
            GpuErrorKind::ShapeMismatch,
            "gpu readback_scalar_device requires a 1-element buffer",
        ));
    }
    let readback = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("spectra-runtime-gpu-scalar-readback"),
        size: std::mem::size_of::<f32>() as u64,
        usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("spectra-runtime-gpu-scalar-readback-encoder"),
    });
    encoder.copy_buffer_to_buffer(
        &buf.buffer,
        0,
        &readback,
        0,
        std::mem::size_of::<f32>() as u64,
    );
    queue.submit(Some(encoder.finish()));
    let slice = readback.slice(..);
    let (sender, receiver) = std::sync::mpsc::channel();
    slice.map_async(wgpu::MapMode::Read, move |result| {
        let _ = sender.send(result);
    });
    device.poll(wgpu::Maintain::Wait);
    receiver
        .recv()
        .map_err(|_| GpuError::new(GpuErrorKind::Readback, "scalar readback callback failed"))?
        .map_err(|err| {
            GpuError::new(
                GpuErrorKind::Readback,
                format!("scalar readback failed: {err:?}"),
            )
        })?;
    let mapped = slice.get_mapped_range();
    let value: f32 = *bytemuck::from_bytes(&mapped);
    drop(mapped);
    readback.unmap();
    Ok(value)
}

/// Read back an N-element pool buffer as a `Vec<f32>`. Used by tests
/// that need to inspect device-side values for correctness
/// assertions. Not part of the hot path.
pub fn readback_device_f32(
    buf: &DeviceBuffer,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
) -> Result<Vec<f32>, GpuError> {
    if buf.elements == 0 {
        return Ok(Vec::new());
    }
    let size = (buf.elements * std::mem::size_of::<f32>()) as u64;
    let readback = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("spectra-runtime-gpu-test-readback"),
        size,
        usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("spectra-runtime-gpu-test-readback-encoder"),
    });
    encoder.copy_buffer_to_buffer(&buf.buffer, 0, &readback, 0, size);
    queue.submit(Some(encoder.finish()));
    let slice = readback.slice(..);
    let (sender, receiver) = std::sync::mpsc::channel();
    slice.map_async(wgpu::MapMode::Read, move |result| {
        let _ = sender.send(result);
    });
    device.poll(wgpu::Maintain::Wait);
    receiver
        .recv()
        .map_err(|_| GpuError::new(GpuErrorKind::Readback, "test readback callback failed"))?
        .map_err(|err| {
            GpuError::new(
                GpuErrorKind::Readback,
                format!("test readback failed: {err:?}"),
            )
        })?;
    let mapped = slice.get_mapped_range();
    let values = bytemuck::cast_slice(&mapped).to_vec();
    drop(mapped);
    readback.unmap();
    Ok(values)
}

/// `out = relu(input)`. Forward pass needed by `backward_relu`'s caller
/// so the autograd step can recover the post-activation values without
/// recomputing on the host. Mirrors the formula in
/// `autograd_parent_grads::AutogradOp::Relu` (CPU reference).
pub fn relu_forward(input: &[f32]) -> Result<Vec<f32>, GpuError> {
    if input.is_empty() {
        return Ok(Vec::new());
    }
    let shader = format!(
        r#"
@group(0) @binding(0) var<storage, read> input_values: array<f32>;
@group(0) @binding(1) var<storage, read_write> out: array<f32>;

@compute @workgroup_size(64)
fn main(@builtin(global_invocation_id) id: vec3<u32>) {{
    let i = id.x;
    if (i >= {len}u) {{
        return;
    }}
    out[i] = max(input_values[i], 0.0);
}}
"#,
        len = input.len()
    );
    dispatch_one_input(input, input.len(), &shader, [input.len() as u32, 1, 1])
}

fn context() -> Result<&'static Mutex<GpuContext>, GpuError> {
    CONTEXT
        .get_or_init(|| pollster::block_on(create_context()).map(Mutex::new))
        .as_ref()
        .map_err(|err| err.clone())
}

async fn create_context() -> Result<GpuContext, GpuError> {
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
        backends: wgpu::Backends::PRIMARY,
        ..Default::default()
    });
    let adapter = instance
        .request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: None,
            force_fallback_adapter: false,
        })
        .await
        .ok_or_else(|| GpuError::new(GpuErrorKind::Other, "no GPU adapter available"))?;
    let (device, queue) = adapter
        .request_device(
            &wgpu::DeviceDescriptor {
                label: Some("spectra-runtime-gpu-device"),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::default(),
            },
            None,
        )
        .await
        .map_err(|err| {
            GpuError::new(
                GpuErrorKind::BufferAlloc,
                format!("failed to create GPU device: {err}"),
            )
        })?;
    Ok(GpuContext { device, queue })
}

fn dispatch_one_input(
    input: &[f32],
    output_len: usize,
    shader_source: &str,
    dispatch_size: [u32; 3],
) -> Result<Vec<f32>, GpuError> {
    let mut guard = context()?
        .lock()
        .map_err(|_| GpuError::new(GpuErrorKind::Other, "gpu context poisoned"))?;
    let input_buffer = storage_buffer(&guard.device, bytemuck::cast_slice(input), "input");
    let output_buffer = output_buffer(&guard.device, output_len);
    let readback_buffer = readback_buffer(&guard.device, output_len);
    run_compute(
        &mut guard,
        shader_source,
        &[
            binding(0, &input_buffer, true),
            binding(1, &output_buffer, false),
        ],
        &[&input_buffer, &output_buffer],
        &output_buffer,
        &readback_buffer,
        output_len,
        dispatch_size,
    )
}

fn dispatch_two_inputs(
    left: &[f32],
    right: &[f32],
    output_len: usize,
    shader_source: &str,
    dispatch_size: [u32; 3],
) -> Result<Vec<f32>, GpuError> {
    let mut guard = context()?
        .lock()
        .map_err(|_| GpuError::new(GpuErrorKind::Other, "gpu context poisoned"))?;
    let left_buffer = storage_buffer(&guard.device, bytemuck::cast_slice(left), "left");
    let right_buffer = storage_buffer(&guard.device, bytemuck::cast_slice(right), "right");
    let output_buffer = output_buffer(&guard.device, output_len);
    let readback_buffer = readback_buffer(&guard.device, output_len);
    run_compute(
        &mut guard,
        shader_source,
        &[
            binding(0, &left_buffer, true),
            binding(1, &right_buffer, true),
            binding(2, &output_buffer, false),
        ],
        &[&left_buffer, &right_buffer, &output_buffer],
        &output_buffer,
        &readback_buffer,
        output_len,
        dispatch_size,
    )
}

fn dispatch_three_inputs(
    first: &[f32],
    second: &[f32],
    third: &[f32],
    output_len: usize,
    shader_source: &str,
    dispatch_size: [u32; 3],
) -> Result<Vec<f32>, GpuError> {
    let mut guard = context()?
        .lock()
        .map_err(|_| GpuError::new(GpuErrorKind::Other, "gpu context poisoned"))?;
    let first_buffer = storage_buffer(&guard.device, bytemuck::cast_slice(first), "first");
    let second_buffer = storage_buffer(&guard.device, bytemuck::cast_slice(second), "second");
    let third_buffer = storage_buffer(&guard.device, bytemuck::cast_slice(third), "third");
    let output_buffer = output_buffer(&guard.device, output_len);
    let readback_buffer = readback_buffer(&guard.device, output_len);
    run_compute(
        &mut guard,
        shader_source,
        &[
            binding(0, &first_buffer, true),
            binding(1, &second_buffer, true),
            binding(2, &third_buffer, true),
            binding(3, &output_buffer, false),
        ],
        &[&first_buffer, &second_buffer, &third_buffer, &output_buffer],
        &output_buffer,
        &readback_buffer,
        output_len,
        dispatch_size,
    )
}

struct BindingSpec<'a> {
    binding: u32,
    buffer: &'a wgpu::Buffer,
    readonly: bool,
}

fn binding(binding: u32, buffer: &wgpu::Buffer, readonly: bool) -> BindingSpec<'_> {
    BindingSpec {
        binding,
        buffer,
        readonly,
    }
}

fn storage_buffer(device: &wgpu::Device, bytes: &[u8], label: &str) -> wgpu::Buffer {
    use wgpu::util::DeviceExt;
    device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some(label),
        contents: bytes,
        usage: wgpu::BufferUsages::STORAGE,
    })
}

fn output_buffer(device: &wgpu::Device, len: usize) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("output"),
        size: byte_len(len),
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
        mapped_at_creation: false,
    })
}

fn readback_buffer(device: &wgpu::Device, len: usize) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("readback"),
        size: byte_len(len),
        usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    })
}

fn byte_len(len: usize) -> u64 {
    (len * std::mem::size_of::<f32>()) as u64
}

fn run_compute(
    ctx: &mut GpuContext,
    shader_source: &str,
    bindings: &[BindingSpec<'_>],
    _keep_alive: &[&wgpu::Buffer],
    output: &wgpu::Buffer,
    readback: &wgpu::Buffer,
    output_len: usize,
    dispatch_size: [u32; 3],
) -> Result<Vec<f32>, GpuError> {
    let shader = ctx
        .device
        .create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("spectra-runtime-gpu-shader"),
            source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(shader_source)),
        });
    let bind_group_layout_entries = bindings
        .iter()
        .map(|entry| wgpu::BindGroupLayoutEntry {
            binding: entry.binding,
            visibility: wgpu::ShaderStages::COMPUTE,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Storage {
                    read_only: entry.readonly,
                },
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        })
        .collect::<Vec<_>>();
    let bind_group_layout = ctx
        .device
        .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("spectra-runtime-gpu-bind-layout"),
            entries: &bind_group_layout_entries,
        });
    let bind_group_entries = bindings
        .iter()
        .map(|entry| wgpu::BindGroupEntry {
            binding: entry.binding,
            resource: entry.buffer.as_entire_binding(),
        })
        .collect::<Vec<_>>();
    let bind_group = ctx.device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("spectra-runtime-gpu-bind-group"),
        layout: &bind_group_layout,
        entries: &bind_group_entries,
    });
    let pipeline_layout = ctx
        .device
        .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("spectra-runtime-gpu-pipeline-layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });
    let pipeline = ctx
        .device
        .create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("spectra-runtime-gpu-pipeline"),
            layout: Some(&pipeline_layout),
            module: &shader,
            entry_point: "main",
            compilation_options: wgpu::PipelineCompilationOptions::default(),
        });
    let mut encoder = ctx
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("spectra-runtime-gpu-encoder"),
        });
    {
        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("spectra-runtime-gpu-pass"),
            timestamp_writes: None,
        });
        pass.set_pipeline(&pipeline);
        pass.set_bind_group(0, &bind_group, &[]);
        pass.dispatch_workgroups(
            dispatch_size[0].div_ceil(64).max(1),
            dispatch_size[1].max(1),
            dispatch_size[2].max(1),
        );
    }
    encoder.copy_buffer_to_buffer(output, 0, readback, 0, byte_len(output_len));
    ctx.queue.submit(Some(encoder.finish()));
    let slice = readback.slice(..);
    let (sender, receiver) = std::sync::mpsc::channel();
    slice.map_async(wgpu::MapMode::Read, move |result| {
        let _ = sender.send(result);
    });
    ctx.device.poll(wgpu::Maintain::Wait);
    receiver
        .recv()
        .map_err(|_| GpuError::new(GpuErrorKind::Readback, "gpu readback callback failed"))?
        .map_err(|err| {
            GpuError::new(
                GpuErrorKind::Readback,
                format!("gpu readback failed: {err:?}"),
            )
        })?;
    let mapped = slice.get_mapped_range();
    let values = bytemuck::cast_slice(&mapped).to_vec();
    drop(mapped);
    readback.unmap();
    Ok(values)
}

/// R-3052 full: same pipeline construction as `run_compute` but skips
/// the copy-to-readback + map_async step. The output stays on device
/// for the next op in the chain. The caller owns the output buffer.
#[allow(clippy::too_many_arguments)]
fn run_compute_no_readback(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    shader_source: &str,
    bindings: &[(u32, &Arc<wgpu::Buffer>, bool)],
    dispatch_size: [u32; 3],
) -> Result<(), GpuError> {
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("spectra-runtime-gpu-device-shader"),
        source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(shader_source)),
    });
    let bind_group_layout_entries = bindings
        .iter()
        .map(|(binding, _buf, readonly)| wgpu::BindGroupLayoutEntry {
            binding: *binding,
            visibility: wgpu::ShaderStages::COMPUTE,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Storage {
                    read_only: *readonly,
                },
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        })
        .collect::<Vec<_>>();
    let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("spectra-runtime-gpu-device-bind-layout"),
        entries: &bind_group_layout_entries,
    });
    let bind_group_entries = bindings
        .iter()
        .map(|(binding, buf, _readonly)| wgpu::BindGroupEntry {
            binding: *binding,
            resource: buf.as_entire_binding(),
        })
        .collect::<Vec<_>>();
    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("spectra-runtime-gpu-device-bind-group"),
        layout: &bind_group_layout,
        entries: &bind_group_entries,
    });
    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("spectra-runtime-gpu-device-pipeline-layout"),
        bind_group_layouts: &[&bind_group_layout],
        push_constant_ranges: &[],
    });
    let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: Some("spectra-runtime-gpu-device-pipeline"),
        layout: Some(&pipeline_layout),
        module: &shader,
        entry_point: "main",
        compilation_options: wgpu::PipelineCompilationOptions::default(),
    });
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("spectra-runtime-gpu-device-encoder"),
    });
    {
        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("spectra-runtime-gpu-device-pass"),
            timestamp_writes: None,
        });
        pass.set_pipeline(&pipeline);
        pass.set_bind_group(0, &bind_group, &[]);
        pass.dispatch_workgroups(
            dispatch_size[0].div_ceil(64).max(1),
            dispatch_size[1].max(1),
            dispatch_size[2].max(1),
        );
    }
    queue.submit(Some(encoder.finish()));
    Ok(())
}
