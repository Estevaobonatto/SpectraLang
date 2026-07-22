use spectra_db::postgres::{open_pool, PostgresConfig, PostgresConnection, PostgresValue};
use spectra_db::PoolConfig;
use spectra_db::query::{Dialect, PostgresDialect};
use std::env;
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll, RawWaker, RawWakerVTable, Waker};
use std::thread;
use std::time::Duration;

fn noop_waker() -> Waker {
    unsafe fn clone(_: *const ()) -> RawWaker { RawWaker::new(std::ptr::null(), &VTABLE) }
    unsafe fn wake(_: *const ()) {}
    unsafe fn wake_by_ref(_: *const ()) {}
    unsafe fn drop(_: *const ()) {}
    static VTABLE: RawWakerVTable = RawWakerVTable::new(clone, wake, wake_by_ref, drop);
    unsafe { Waker::from_raw(RawWaker::new(std::ptr::null(), &VTABLE)) }
}

#[test]
fn postgres_config_parses_without_exposing_secret() {
    let config =
        PostgresConfig::from_url("postgres://alice:super-secret@localhost:5432/app").unwrap();
    assert_eq!(config.user, "alice");
    assert_eq!(config.database, "app");
    assert!(!format!("{config:?}").contains("super-secret"));
}

#[test]
fn postgres_dialect_uses_dollar_placeholders() {
    let dialect = PostgresDialect;
    assert_eq!(dialect.placeholder(1), "$1");
    assert_eq!(dialect.quote_identifier("users"), Ok("\"users\"".into()));
    assert_eq!(
        dialect.quote_identifier("user\"name"),
        Ok("\"user\"\"name\"".into())
    );
}

#[test]
fn real_postgres_crud_and_transaction_when_configured() {
    let Ok(url) = env::var("SPECTRA_POSTGRES_URL") else {
        eprintln!("skipped_environment: SPECTRA_POSTGRES_URL is not configured");
        return;
    };
    let connection = PostgresConnection::open(PostgresConfig::from_url(&url).unwrap()).unwrap();
    connection.execute_batch("DROP TABLE IF EXISTS spectra_r2505_users; CREATE TABLE spectra_r2505_users (id BIGINT PRIMARY KEY, name TEXT NOT NULL);").unwrap();
    let mut statement = connection
        .prepare("INSERT INTO spectra_r2505_users (id, name) VALUES ($1, $2)")
        .unwrap();
    statement.bind(1, PostgresValue::Int64(1)).unwrap();
    statement
        .bind(2, PostgresValue::Text("Ada".into()))
        .unwrap();
    assert_eq!(statement.execute().unwrap().affected_rows, 1);
    let result = connection
        .execute_query(spectra_db::CompiledQuery {
            sql: "SELECT id, name FROM spectra_r2505_users ORDER BY id".into(),
            params: vec![],
        })
        .unwrap();
    assert_eq!(result.rows.len(), 1);
    assert_eq!(result.rows[0][0], PostgresValue::Int64(1));
    assert_eq!(result.rows[0][1], PostgresValue::Text("Ada".into()));
    connection
        .execute_batch("DROP TABLE spectra_r2505_users")
        .unwrap();
}

#[test]
fn real_postgres_transactions_copy_and_notify_when_configured() {
    let Ok(url) = env::var("SPECTRA_POSTGRES_URL") else {
        eprintln!("skipped_environment: SPECTRA_POSTGRES_URL is not configured");
        return;
    };
    let connection = PostgresConnection::open(PostgresConfig::from_url(&url).unwrap()).unwrap();
    connection
        .execute_batch("CREATE TEMP TABLE spectra_r2505_copy(id BIGINT, name TEXT)")
        .unwrap();
    let tx = connection.begin().unwrap();
    tx.execute("INSERT INTO spectra_r2505_copy VALUES (1, 'before')")
        .unwrap();
    tx.savepoint("nested").unwrap();
    tx.execute("INSERT INTO spectra_r2505_copy VALUES (2, 'after')")
        .unwrap();
    tx.rollback_to("nested").unwrap();
    tx.commit().unwrap();
    let count = connection
        .execute_query(spectra_db::CompiledQuery {
            sql: "SELECT count(*) FROM spectra_r2505_copy".into(),
            params: vec![],
        })
        .unwrap();
    assert_eq!(count.rows.len(), 1);

    let rows = (3..=1026)
        .map(|id| vec![PostgresValue::Int64(id), PostgresValue::Text(format!("copy-{id}"))])
        .collect::<Vec<_>>();
    let copied = connection
        .copy_in_rows("COPY spectra_r2505_copy (id, name) FROM STDIN", rows)
        .unwrap();
    assert_eq!(copied, 1024);
    let bytes = connection
        .copy_out_bytes("COPY (SELECT id, name FROM spectra_r2505_copy ORDER BY id) TO STDOUT")
        .unwrap();
    let output = String::from_utf8(bytes).unwrap();
    assert!(output.contains("copy-3"));
    assert!(output.contains("copy-1026"));

    let listener = connection.listen("spectra_r2505_channel").unwrap();
    let publisher = PostgresConnection::open(PostgresConfig::from_url(&url).unwrap()).unwrap();
    publisher
        .execute_batch("SELECT pg_notify('spectra_r2505_channel', 'payload')")
        .unwrap();
    let notification = listener
        .next_timeout(std::time::Duration::from_secs(3))
        .unwrap()
        .unwrap();
    assert_eq!(notification.channel, "spectra_r2505_channel");
    assert_eq!(notification.payload, "payload");
    publisher.close().unwrap();
    connection.close().unwrap();
}

#[test]
fn real_postgres_pool_and_async_bridge_when_configured() {
    let Ok(url) = env::var("SPECTRA_POSTGRES_URL") else {
        eprintln!("skipped_environment: SPECTRA_POSTGRES_URL is not configured");
        return;
    };
    let config = PostgresConfig::from_url(&url).unwrap();
    let pool = open_pool(config, PoolConfig { max_size: 2, ..PoolConfig::default() }).unwrap();
    let lease = pool.acquire_blocking().unwrap();
    lease.connection().unwrap().health_check().unwrap();
    lease.release().unwrap();
    let connection = PostgresConnection::open(PostgresConfig::from_url(&url).unwrap()).unwrap();
    let mut future = Box::pin(connection.query_async(spectra_db::CompiledQuery {
        sql: "SELECT 1".into(),
        params: vec![],
    }));
    let waker = noop_waker();
    let mut context = Context::from_waker(&waker);
    let result = loop {
        match Future::poll(Pin::as_mut(&mut future), &mut context) {
            Poll::Ready(result) => break result,
            Poll::Pending => thread::sleep(Duration::from_millis(2)),
        }
    };
    assert_eq!(result.unwrap().rows.len(), 1);
    connection.close().unwrap();
    pool.shutdown().unwrap();
}
