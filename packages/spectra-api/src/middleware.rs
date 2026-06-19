use crate::handler::HandlerError;
use crate::http::{self, Request, Response};
use crate::{alloc_spectra_string, read_args, read_spectra_string, write_result};
use spectra_runtime::ffi::{
    SpectraHostCallContext, SpectraHostValue, HOST_STATUS_INVALID_ARGUMENT,
};
use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex, OnceLock};
use std::task::{Context, Poll, RawWaker, RawWakerVTable, Waker};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MiddlewareDecision {
    Continue(Request),
    ShortCircuit(Response),
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MiddlewareTrace {
    events: Vec<String>,
    short_circuited: bool,
}

impl MiddlewareTrace {
    pub fn push(&mut self, event: impl Into<String>) {
        self.events.push(event.into());
    }

    pub fn events(&self) -> &[String] {
        &self.events
    }

    pub fn short_circuited(&self) -> bool {
        self.short_circuited
    }
}

#[derive(Clone, Debug, Default)]
pub struct MiddlewareContext {
    trace: MiddlewareTrace,
}

impl MiddlewareContext {
    pub fn trace(&self) -> &MiddlewareTrace {
        &self.trace
    }

    pub fn trace_mut(&mut self) -> &mut MiddlewareTrace {
        &mut self.trace
    }
}

pub trait Middleware: Send + Sync + 'static {
    fn on_request(
        &self,
        request: Request,
        context: &mut MiddlewareContext,
    ) -> Result<MiddlewareDecision, HandlerError>;

    fn on_response(
        &self,
        response: Response,
        _context: &mut MiddlewareContext,
    ) -> Result<Response, HandlerError> {
        Ok(response)
    }
}

pub trait AsyncMiddleware: Send + Sync + 'static {
    fn on_request<'a>(
        &'a self,
        request: Request,
        context: &'a mut MiddlewareContext,
    ) -> Pin<Box<dyn Future<Output = Result<MiddlewareDecision, HandlerError>> + Send + 'a>>;

    fn on_response<'a>(
        &'a self,
        response: Response,
        _context: &'a mut MiddlewareContext,
    ) -> Pin<Box<dyn Future<Output = Result<Response, HandlerError>> + Send + 'a>> {
        Box::pin(async move { Ok(response) })
    }
}

#[derive(Clone)]
enum MiddlewareEntry {
    Sync(Arc<dyn Middleware>),
    Async(Arc<dyn AsyncMiddleware>),
}

#[derive(Clone, Default)]
pub struct MiddlewareChain {
    entries: Vec<MiddlewareEntry>,
}

impl MiddlewareChain {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn use_sync<M>(mut self, middleware: M) -> Self
    where
        M: Middleware,
    {
        self.entries
            .push(MiddlewareEntry::Sync(Arc::new(middleware)));
        self
    }

    pub fn use_async<M>(mut self, middleware: M) -> Self
    where
        M: AsyncMiddleware,
    {
        self.entries
            .push(MiddlewareEntry::Async(Arc::new(middleware)));
        self
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn execute_sync(
        &self,
        request: Request,
        terminal_response: Response,
    ) -> Result<(Response, MiddlewareTrace), HandlerError> {
        let mut context = MiddlewareContext::default();
        let mut current_request = request;
        let mut executed = Vec::new();
        let mut response = None;

        for (index, entry) in self.entries.iter().enumerate() {
            match entry {
                MiddlewareEntry::Sync(middleware) => {
                    match middleware.on_request(current_request.clone(), &mut context)? {
                        MiddlewareDecision::Continue(next) => {
                            current_request = next;
                            executed.push(index);
                        }
                        MiddlewareDecision::ShortCircuit(short) => {
                            context.trace.short_circuited = true;
                            executed.push(index);
                            response = Some(short);
                            break;
                        }
                    }
                }
                MiddlewareEntry::Async(_) => {
                    return Err(HandlerError::new(
                        500,
                        "async middleware requires execute_async",
                    ));
                }
            }
        }

        let mut response = response.unwrap_or(terminal_response);
        for index in executed.into_iter().rev() {
            if let MiddlewareEntry::Sync(middleware) = &self.entries[index] {
                response = middleware.on_response(response, &mut context)?;
            }
        }

        Ok((response, context.trace))
    }

    pub async fn execute_async(
        &self,
        request: Request,
        terminal_response: Response,
    ) -> Result<(Response, MiddlewareTrace), HandlerError> {
        let mut context = MiddlewareContext::default();
        let mut current_request = request;
        let mut executed = Vec::new();
        let mut response = None;

        for (index, entry) in self.entries.iter().enumerate() {
            let decision = match entry {
                MiddlewareEntry::Sync(middleware) => {
                    middleware.on_request(current_request.clone(), &mut context)?
                }
                MiddlewareEntry::Async(middleware) => {
                    middleware
                        .on_request(current_request.clone(), &mut context)
                        .await?
                }
            };

            match decision {
                MiddlewareDecision::Continue(next) => {
                    current_request = next;
                    executed.push(index);
                }
                MiddlewareDecision::ShortCircuit(short) => {
                    context.trace.short_circuited = true;
                    executed.push(index);
                    response = Some(short);
                    break;
                }
            }
        }

        let mut response = response.unwrap_or(terminal_response);
        for index in executed.into_iter().rev() {
            response = match &self.entries[index] {
                MiddlewareEntry::Sync(middleware) => {
                    middleware.on_response(response, &mut context)?
                }
                MiddlewareEntry::Async(middleware) => {
                    middleware.on_response(response, &mut context).await?
                }
            };
        }

        Ok((response, context.trace))
    }
}

#[derive(Clone, Debug)]
struct RecordedMiddleware {
    before_marker: String,
    after_marker: String,
    short_circuit_response: Option<SpectraHostValue>,
}

impl Middleware for RecordedMiddleware {
    fn on_request(
        &self,
        request: Request,
        context: &mut MiddlewareContext,
    ) -> Result<MiddlewareDecision, HandlerError> {
        context.trace_mut().push(self.before_marker.clone());
        if let Some(response_handle) = self.short_circuit_response {
            let response = http::clone_response(response_handle)
                .ok_or_else(|| HandlerError::new(500, "invalid middleware response handle"))?;
            Ok(MiddlewareDecision::ShortCircuit(response))
        } else {
            Ok(MiddlewareDecision::Continue(request))
        }
    }

    fn on_response(
        &self,
        response: Response,
        context: &mut MiddlewareContext,
    ) -> Result<Response, HandlerError> {
        context.trace_mut().push(self.after_marker.clone());
        response
            .with_header("x-spectra-middleware", self.after_marker.clone())
            .map_err(|err| HandlerError::new(500, err.to_string()))
    }
}

impl AsyncMiddleware for RecordedMiddleware {
    fn on_request<'a>(
        &'a self,
        request: Request,
        context: &'a mut MiddlewareContext,
    ) -> Pin<Box<dyn Future<Output = Result<MiddlewareDecision, HandlerError>> + Send + 'a>> {
        Box::pin(async move { Middleware::on_request(self, request, context) })
    }

    fn on_response<'a>(
        &'a self,
        response: Response,
        context: &'a mut MiddlewareContext,
    ) -> Pin<Box<dyn Future<Output = Result<Response, HandlerError>> + Send + 'a>> {
        Box::pin(async move { Middleware::on_response(self, response, context) })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum StoredMiddlewareKind {
    Sync,
    Async,
}

#[derive(Clone, Debug)]
struct StoredMiddleware {
    kind: StoredMiddlewareKind,
    recorded: RecordedMiddleware,
}

struct MiddlewareStore {
    next_chain: SpectraHostValue,
    next_middleware: SpectraHostValue,
    next_trace: SpectraHostValue,
    chains: HashMap<SpectraHostValue, Vec<SpectraHostValue>>,
    middlewares: HashMap<SpectraHostValue, StoredMiddleware>,
    traces: HashMap<SpectraHostValue, MiddlewareTrace>,
    last_trace: SpectraHostValue,
}

impl MiddlewareStore {
    fn new() -> Self {
        Self {
            next_chain: 1,
            next_middleware: 1,
            next_trace: 1,
            chains: HashMap::new(),
            middlewares: HashMap::new(),
            traces: HashMap::new(),
            last_trace: 0,
        }
    }

    fn insert_chain(&mut self, entries: Vec<SpectraHostValue>) -> SpectraHostValue {
        let handle = self.next_chain;
        self.next_chain = self.next_chain.saturating_add(1).max(1);
        self.chains.insert(handle, entries);
        handle
    }

    fn insert_middleware(&mut self, middleware: StoredMiddleware) -> SpectraHostValue {
        let handle = self.next_middleware;
        self.next_middleware = self.next_middleware.saturating_add(1).max(1);
        self.middlewares.insert(handle, middleware);
        handle
    }

    fn insert_trace(&mut self, trace: MiddlewareTrace) -> SpectraHostValue {
        let handle = self.next_trace;
        self.next_trace = self.next_trace.saturating_add(1).max(1);
        self.traces.insert(handle, trace);
        self.last_trace = handle;
        handle
    }
}

fn store() -> &'static Mutex<MiddlewareStore> {
    static STORE: OnceLock<Mutex<MiddlewareStore>> = OnceLock::new();
    STORE.get_or_init(|| Mutex::new(MiddlewareStore::new()))
}

fn build_chain(
    chain_handle: SpectraHostValue,
    allow_async: bool,
) -> Result<MiddlewareChain, HandlerError> {
    let store = store().lock().unwrap_or_else(|e| e.into_inner());
    let handles = store
        .chains
        .get(&chain_handle)
        .cloned()
        .ok_or_else(|| HandlerError::new(500, "middleware chain not found"))?;
    let mut chain = MiddlewareChain::new();
    for handle in handles {
        let middleware = store
            .middlewares
            .get(&handle)
            .cloned()
            .ok_or_else(|| HandlerError::new(500, "middleware handle not found"))?;
        match middleware.kind {
            StoredMiddlewareKind::Sync => {
                chain = chain.use_sync(middleware.recorded);
            }
            StoredMiddlewareKind::Async if allow_async => {
                chain = chain.use_async(middleware.recorded);
            }
            StoredMiddlewareKind::Async => {
                return Err(HandlerError::new(
                    500,
                    "async middleware requires execute_async",
                ));
            }
        }
    }
    Ok(chain)
}

fn write_response_and_trace(
    ctx: *mut SpectraHostCallContext,
    result: Result<(Response, MiddlewareTrace), HandlerError>,
) -> i32 {
    match result {
        Ok((response, trace)) => {
            let response = http::store_response(response);
            store()
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .insert_trace(trace);
            write_result(ctx, response)
        }
        Err(_) => HOST_STATUS_INVALID_ARGUMENT,
    }
}

fn block_on_ready_middleware(
    mut future: Pin<
        Box<dyn Future<Output = Result<(Response, MiddlewareTrace), HandlerError>> + Send>,
    >,
) -> Result<(Response, MiddlewareTrace), HandlerError> {
    fn clone(_: *const ()) -> RawWaker {
        RawWaker::new(std::ptr::null(), &VTABLE)
    }
    fn wake(_: *const ()) {}
    fn wake_by_ref(_: *const ()) {}
    fn drop(_: *const ()) {}
    static VTABLE: RawWakerVTable = RawWakerVTable::new(clone, wake, wake_by_ref, drop);
    let raw = RawWaker::new(std::ptr::null(), &VTABLE);
    let waker = unsafe { Waker::from_raw(raw) };
    let mut cx = Context::from_waker(&waker);
    match future.as_mut().poll(&mut cx) {
        Poll::Ready(value) => value,
        Poll::Pending => Err(HandlerError::new(
            500,
            "middleware future returned pending without a reactor",
        )),
    }
}

fn register_recorded(ctx: *mut SpectraHostCallContext, kind: StoredMiddlewareKind) -> i32 {
    let Ok(args) = read_args(ctx, 2) else {
        return HOST_STATUS_INVALID_ARGUMENT;
    };
    let (Some(before_marker), Some(after_marker)) =
        (read_spectra_string(args[0]), read_spectra_string(args[1]))
    else {
        return HOST_STATUS_INVALID_ARGUMENT;
    };
    let recorded = RecordedMiddleware {
        before_marker,
        after_marker,
        short_circuit_response: None,
    };
    let mut store = store().lock().unwrap_or_else(|e| e.into_inner());
    let handle = store.insert_middleware(StoredMiddleware { kind, recorded });
    write_result(ctx, handle)
}

fn register_short_circuit(ctx: *mut SpectraHostCallContext, kind: StoredMiddlewareKind) -> i32 {
    let Ok(args) = read_args(ctx, 3) else {
        return HOST_STATUS_INVALID_ARGUMENT;
    };
    let (Some(before_marker), Some(after_marker)) =
        (read_spectra_string(args[0]), read_spectra_string(args[1]))
    else {
        return HOST_STATUS_INVALID_ARGUMENT;
    };
    if http::clone_response(args[2]).is_none() {
        return HOST_STATUS_INVALID_ARGUMENT;
    }
    let recorded = RecordedMiddleware {
        before_marker,
        after_marker,
        short_circuit_response: Some(args[2]),
    };
    let mut store = store().lock().unwrap_or_else(|e| e.into_inner());
    let handle = store.insert_middleware(StoredMiddleware { kind, recorded });
    write_result(ctx, handle)
}

pub extern "C" fn chain(ctx: *mut SpectraHostCallContext) -> i32 {
    chain_new(ctx)
}

pub extern "C" fn chain_new(ctx: *mut SpectraHostCallContext) -> i32 {
    let mut store = store().lock().unwrap_or_else(|e| e.into_inner());
    let handle = store.insert_chain(Vec::new());
    write_result(ctx, handle)
}

pub extern "C" fn chain_len(ctx: *mut SpectraHostCallContext) -> i32 {
    let Ok(args) = read_args(ctx, 1) else {
        return HOST_STATUS_INVALID_ARGUMENT;
    };
    let store = store().lock().unwrap_or_else(|e| e.into_inner());
    let Some(entries) = store.chains.get(&args[0]) else {
        return HOST_STATUS_INVALID_ARGUMENT;
    };
    write_result(ctx, entries.len() as SpectraHostValue)
}

pub extern "C" fn register_sync(ctx: *mut SpectraHostCallContext) -> i32 {
    register_recorded(ctx, StoredMiddlewareKind::Sync)
}

pub extern "C" fn register_sync_short_circuit(ctx: *mut SpectraHostCallContext) -> i32 {
    register_short_circuit(ctx, StoredMiddlewareKind::Sync)
}

pub extern "C" fn register_async(ctx: *mut SpectraHostCallContext) -> i32 {
    register_recorded(ctx, StoredMiddlewareKind::Async)
}

pub extern "C" fn register_async_short_circuit(ctx: *mut SpectraHostCallContext) -> i32 {
    register_short_circuit(ctx, StoredMiddlewareKind::Async)
}

pub extern "C" fn use_sync(ctx: *mut SpectraHostCallContext) -> i32 {
    append_middleware(ctx, StoredMiddlewareKind::Sync)
}

pub extern "C" fn use_async(ctx: *mut SpectraHostCallContext) -> i32 {
    append_middleware(ctx, StoredMiddlewareKind::Async)
}

fn append_middleware(ctx: *mut SpectraHostCallContext, expected: StoredMiddlewareKind) -> i32 {
    let Ok(args) = read_args(ctx, 2) else {
        return HOST_STATUS_INVALID_ARGUMENT;
    };
    let mut store = store().lock().unwrap_or_else(|e| e.into_inner());
    let Some(entries) = store.chains.get(&args[0]).cloned() else {
        return HOST_STATUS_INVALID_ARGUMENT;
    };
    let Some(middleware) = store.middlewares.get(&args[1]) else {
        return HOST_STATUS_INVALID_ARGUMENT;
    };
    if middleware.kind != expected {
        return HOST_STATUS_INVALID_ARGUMENT;
    }
    let mut next_entries = entries;
    next_entries.push(args[1]);
    let handle = store.insert_chain(next_entries);
    write_result(ctx, handle)
}

pub extern "C" fn execute_sync(ctx: *mut SpectraHostCallContext) -> i32 {
    let Ok(args) = read_args(ctx, 3) else {
        return HOST_STATUS_INVALID_ARGUMENT;
    };
    let Some(request) = http::clone_request(args[1]) else {
        return HOST_STATUS_INVALID_ARGUMENT;
    };
    let Some(response) = http::clone_response(args[2]) else {
        return HOST_STATUS_INVALID_ARGUMENT;
    };
    let result =
        build_chain(args[0], false).and_then(|chain| chain.execute_sync(request, response));
    write_response_and_trace(ctx, result)
}

pub extern "C" fn execute_async(ctx: *mut SpectraHostCallContext) -> i32 {
    let Ok(args) = read_args(ctx, 3) else {
        return HOST_STATUS_INVALID_ARGUMENT;
    };
    let Some(request) = http::clone_request(args[1]) else {
        return HOST_STATUS_INVALID_ARGUMENT;
    };
    let Some(response) = http::clone_response(args[2]) else {
        return HOST_STATUS_INVALID_ARGUMENT;
    };
    let result = build_chain(args[0], true).and_then(|chain| {
        block_on_ready_middleware(Box::pin(async move {
            chain.execute_async(request, response).await
        }))
    });
    write_response_and_trace(ctx, result)
}

pub extern "C" fn last_trace(ctx: *mut SpectraHostCallContext) -> i32 {
    let store = store().lock().unwrap_or_else(|e| e.into_inner());
    write_result(ctx, store.last_trace)
}

pub extern "C" fn trace_len(ctx: *mut SpectraHostCallContext) -> i32 {
    let Ok(args) = read_args(ctx, 1) else {
        return HOST_STATUS_INVALID_ARGUMENT;
    };
    let store = store().lock().unwrap_or_else(|e| e.into_inner());
    let Some(trace) = store.traces.get(&args[0]) else {
        return HOST_STATUS_INVALID_ARGUMENT;
    };
    write_result(ctx, trace.events().len() as SpectraHostValue)
}

pub extern "C" fn trace_event(ctx: *mut SpectraHostCallContext) -> i32 {
    let Ok(args) = read_args(ctx, 2) else {
        return HOST_STATUS_INVALID_ARGUMENT;
    };
    let Ok(index) = usize::try_from(args[1]) else {
        return HOST_STATUS_INVALID_ARGUMENT;
    };
    let store = store().lock().unwrap_or_else(|e| e.into_inner());
    let Some(trace) = store.traces.get(&args[0]) else {
        return HOST_STATUS_INVALID_ARGUMENT;
    };
    let Some(event) = trace.events().get(index) else {
        return HOST_STATUS_INVALID_ARGUMENT;
    };
    write_result(ctx, alloc_spectra_string(event))
}

pub extern "C" fn trace_short_circuited(ctx: *mut SpectraHostCallContext) -> i32 {
    let Ok(args) = read_args(ctx, 1) else {
        return HOST_STATUS_INVALID_ARGUMENT;
    };
    let store = store().lock().unwrap_or_else(|e| e.into_inner());
    let Some(trace) = store.traces.get(&args[0]) else {
        return HOST_STATUS_INVALID_ARGUMENT;
    };
    write_result(ctx, if trace.short_circuited() { 1 } else { 0 })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::http::{Method, Status};

    fn request() -> Request {
        Request::new(Method::Get, "/middleware").expect("valid request")
    }

    fn response(status: u16) -> Response {
        Response::new(Status::new(status).expect("valid status"))
    }

    fn block_on_ready<F: Future + ?Sized>(mut future: Pin<Box<F>>) -> F::Output {
        fn clone(_: *const ()) -> RawWaker {
            RawWaker::new(std::ptr::null(), &VTABLE)
        }
        fn wake(_: *const ()) {}
        fn wake_by_ref(_: *const ()) {}
        fn drop(_: *const ()) {}
        static VTABLE: RawWakerVTable = RawWakerVTable::new(clone, wake, wake_by_ref, drop);
        let raw = RawWaker::new(std::ptr::null(), &VTABLE);
        let waker = unsafe { Waker::from_raw(raw) };
        let mut cx = Context::from_waker(&waker);
        match future.as_mut().poll(&mut cx) {
            Poll::Ready(value) => value,
            Poll::Pending => panic!("test future unexpectedly pending"),
        }
    }

    #[test]
    fn sync_chain_runs_request_order_and_response_reverse_order() {
        let chain = MiddlewareChain::new()
            .use_sync(RecordedMiddleware {
                before_marker: "first:request".to_string(),
                after_marker: "first:response".to_string(),
                short_circuit_response: None,
            })
            .use_sync(RecordedMiddleware {
                before_marker: "second:request".to_string(),
                after_marker: "second:response".to_string(),
                short_circuit_response: None,
            });

        let (response, trace) = chain
            .execute_sync(request(), response(200))
            .expect("middleware response");

        assert_eq!(response.status.code(), 200);
        assert_eq!(
            trace.events(),
            &[
                "first:request".to_string(),
                "second:request".to_string(),
                "second:response".to_string(),
                "first:response".to_string(),
            ]
        );
        assert!(!trace.short_circuited());
    }

    #[test]
    fn short_circuit_stops_remaining_requests_and_unwinds_executed_hooks() {
        let short = http::store_response(response(429));
        let chain = MiddlewareChain::new()
            .use_sync(RecordedMiddleware {
                before_marker: "first:request".to_string(),
                after_marker: "first:response".to_string(),
                short_circuit_response: None,
            })
            .use_sync(RecordedMiddleware {
                before_marker: "limit:request".to_string(),
                after_marker: "limit:response".to_string(),
                short_circuit_response: Some(short),
            })
            .use_sync(RecordedMiddleware {
                before_marker: "never:request".to_string(),
                after_marker: "never:response".to_string(),
                short_circuit_response: None,
            });

        let (response, trace) = chain
            .execute_sync(request(), response(200))
            .expect("short circuit response");

        assert_eq!(response.status.code(), 429);
        assert_eq!(
            trace.events(),
            &[
                "first:request".to_string(),
                "limit:request".to_string(),
                "limit:response".to_string(),
                "first:response".to_string(),
            ]
        );
        assert!(trace.short_circuited());
    }

    #[test]
    fn async_chain_accepts_sync_and_async_middleware() {
        let chain = MiddlewareChain::new()
            .use_sync(RecordedMiddleware {
                before_marker: "sync:request".to_string(),
                after_marker: "sync:response".to_string(),
                short_circuit_response: None,
            })
            .use_async(RecordedMiddleware {
                before_marker: "async:request".to_string(),
                after_marker: "async:response".to_string(),
                short_circuit_response: None,
            });

        let (response, trace) =
            block_on_ready(Box::pin(chain.execute_async(request(), response(204))))
                .expect("async middleware response");

        assert_eq!(response.status.code(), 204);
        assert_eq!(
            trace.events(),
            &[
                "sync:request".to_string(),
                "async:request".to_string(),
                "async:response".to_string(),
                "sync:response".to_string(),
            ]
        );
    }
}
