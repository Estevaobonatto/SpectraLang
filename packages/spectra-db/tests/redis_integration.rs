use spectra_db::redis::{RedisConfig, RedisConnection, RedisValue};
use std::future::Future;
use std::sync::{Arc, Condvar, Mutex};
use std::task::{Context, Poll, RawWaker, RawWakerVTable, Waker};
use std::time::Duration;

fn block_on<F: Future>(future: F) -> F::Output {
    let signal = Arc::new((Mutex::new(false), Condvar::new()));
    unsafe fn clone(data: *const ()) -> RawWaker {
        let arc = Arc::<(Mutex<bool>, Condvar)>::from_raw(data as *const _);
        let cloned = arc.clone();
        std::mem::forget(arc);
        RawWaker::new(Arc::into_raw(cloned) as *const (), &VTABLE)
    }
    unsafe fn wake(data: *const ()) {
        let arc = Arc::<(Mutex<bool>, Condvar)>::from_raw(data as *const _);
        *arc.0.lock().unwrap() = true;
        arc.1.notify_one();
    }
    unsafe fn wake_by_ref(data: *const ()) {
        let arc = Arc::<(Mutex<bool>, Condvar)>::from_raw(data as *const _);
        *arc.0.lock().unwrap() = true;
        arc.1.notify_one();
        std::mem::forget(arc);
    }
    unsafe fn drop_waker(data: *const ()) {
        drop(Arc::<(Mutex<bool>, Condvar)>::from_raw(data as *const _));
    }
    static VTABLE: RawWakerVTable = RawWakerVTable::new(clone, wake, wake_by_ref, drop_waker);
    let waker = unsafe {
        Waker::from_raw(RawWaker::new(
            Arc::into_raw(signal.clone()) as *const (),
            &VTABLE,
        ))
    };
    let mut cx = Context::from_waker(&waker);
    let mut future = Box::pin(future);
    loop {
        if let Poll::Ready(value) = future.as_mut().poll(&mut cx) {
            return value;
        }
        let mut ready = signal.0.lock().unwrap();
        while !*ready {
            ready = signal.1.wait(ready).unwrap();
        }
        *ready = false;
    }
}

fn config() -> Option<RedisConfig> {
    std::env::var("SPECTRA_REDIS_URL")
        .ok()
        .and_then(|url| RedisConfig::from_url(&url).ok())
}

#[test]
fn redis_config_redacts_credentials_and_parses_url() {
    let config = RedisConfig::from_url("redis://user:secret@127.0.0.1:6379/3").unwrap();
    assert_eq!(config.database, 3);
    assert!(!format!("{config:?}").contains("secret"));
}

#[test]
fn real_redis_commands_ttl_and_pubsub() {
    let Some(config) = config() else {
        eprintln!("skipped_environment: SPECTRA_REDIS_URL not configured");
        return;
    };
    let connection = RedisConnection::open(config.clone()).unwrap();
    block_on(connection.set("spectra:r2507:empty", RedisValue::Text(String::new()), None)).unwrap();
    assert_eq!(
        block_on(connection.get("spectra:r2507:missing")).unwrap(),
        None
    );
    assert_eq!(
        block_on(connection.get("spectra:r2507:empty")).unwrap(),
        Some(RedisValue::Text(String::new()))
    );
    block_on(connection.set("spectra:r2507:number", RedisValue::Integer(1), None)).unwrap();
    assert_eq!(
        block_on(connection.incr("spectra:r2507:number", 2)).unwrap(),
        3
    );
    block_on(connection.set("spectra:r2507:ttl", RedisValue::Text("v".into()), None)).unwrap();
    assert!(block_on(connection.expire("spectra:r2507:ttl", Duration::from_secs(1))).unwrap());
    assert!(block_on(connection.exists("spectra:r2507:ttl")).unwrap());
    let pubsub = connection.subscribe("spectra:r2507:channel").unwrap();
    let next = pubsub.next_notification();
    let publisher = redis::Client::open(std::env::var("SPECTRA_REDIS_URL").unwrap()).unwrap();
    let mut publisher = publisher.get_connection().unwrap();
    let _: i64 = redis::cmd("PUBLISH")
        .arg("spectra:r2507:channel")
        .arg(b"payload")
        .query(&mut publisher)
        .unwrap();
    let notification = block_on(next).unwrap().unwrap();
    assert_eq!(notification.payload, b"payload");
    pubsub.unsubscribe().unwrap();
    block_on(connection.delete("spectra:r2507:empty")).unwrap();
    block_on(connection.delete("spectra:r2507:number")).unwrap();
    block_on(connection.delete("spectra:r2507:ttl")).unwrap();
}
