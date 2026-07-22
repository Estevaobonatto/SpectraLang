use spectra_db::postgres::{PostgresConfig, PostgresConnection, PostgresValue};
use spectra_db::query::{Dialect, PostgresDialect};
use std::env;

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

    let copied = connection
        .copy_in_rows(
            "COPY spectra_r2505_copy (id, name) FROM STDIN",
            vec![vec![
                PostgresValue::Int64(3),
                PostgresValue::Text("copy".into()),
            ]],
        )
        .unwrap();
    assert_eq!(copied, 1);
    let bytes = connection
        .copy_out_bytes("COPY (SELECT id, name FROM spectra_r2505_copy ORDER BY id) TO STDOUT")
        .unwrap();
    assert!(String::from_utf8(bytes).unwrap().contains("copy"));

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
