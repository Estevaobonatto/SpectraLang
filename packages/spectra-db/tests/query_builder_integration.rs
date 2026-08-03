use spectra_db::query::{
    Boolean, Column, Delete, Insert, Integer, Order, Predicate, Query, Real, Select, SqliteDialect,
    Text, Update, Value,
};
use spectra_db::sqlite::{open_pool, SqliteConnection, SqliteValue};
use spectra_db::PoolConfig;
use std::time::{SystemTime, UNIX_EPOCH};

fn database() -> SqliteConnection {
    let path = std::env::temp_dir().join(format!(
        "spectra-r2502-{}.sqlite",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let connection = SqliteConnection::open(&path, std::time::Duration::from_secs(1)).unwrap();
    connection.execute_batch("CREATE TABLE items (id INTEGER PRIMARY KEY, name TEXT NOT NULL, score REAL NOT NULL, active INTEGER NOT NULL);").unwrap();
    connection
}

#[test]
fn compiles_parameterized_crud_with_deterministic_placeholders() {
    let id = Column::<Integer>::new("id");
    let name = Column::<Text>::new("name");
    let score = Column::<Real>::new("score");
    let dialect = SqliteDialect;

    let insert = Insert::into("items")
        .set(id.clone(), Value::integer(7))
        .set(name.clone(), Value::text("O'Reilly"))
        .set(score.clone(), Value::real(9.5));
    let compiled = insert.compile(&dialect).unwrap();
    assert_eq!(
        compiled.sql,
        "INSERT INTO \"items\" (\"id\", \"name\", \"score\") VALUES (?1, ?2, ?3)"
    );
    assert_eq!(
        compiled.params,
        vec![
            SqliteValue::Integer(7),
            SqliteValue::Text("O'Reilly".into()),
            SqliteValue::Real(9.5)
        ]
    );

    let select = Select::from("items")
        .columns_named(&[id.reference(), name.reference(), score.reference()])
        .where_(id.equals(Value::integer(7)))
        .order_by(score, Order::Desc)
        .limit(1);
    assert_eq!(select.compile(&dialect).unwrap().sql, "SELECT \"id\", \"name\", \"score\" FROM \"items\" WHERE \"id\" = ?1 ORDER BY \"score\" DESC LIMIT 1");

    let update = Update::table("items")
        .set(name.clone(), Value::text("updated"))
        .where_(id.equals(Value::integer(7)));
    assert_eq!(
        update.compile(&dialect).unwrap().sql,
        "UPDATE \"items\" SET \"name\" = ?1 WHERE \"id\" = ?2"
    );
    let delete = Delete::from("items").where_(Predicate::eq(id.expr(), Value::integer(7).expr()));
    assert_eq!(
        delete.compile(&dialect).unwrap().sql,
        "DELETE FROM \"items\" WHERE \"id\" = ?1"
    );
}

#[test]
fn executes_real_sqlite_crud_and_preserves_parameters() {
    let connection = database();
    let id = Column::<Integer>::new("id");
    let name = Column::<Text>::new("name");
    let score = Column::<Real>::new("score");
    let active = Column::<Boolean>::new("active");
    let dialect = SqliteDialect;

    connection
        .execute_query(
            Insert::into("items")
                .set(id.clone(), Value::integer(1))
                .set(name.clone(), Value::text("first"))
                .set(score.clone(), Value::real(1.5))
                .set(active, Value::boolean(true))
                .compile(&dialect)
                .unwrap(),
        )
        .unwrap();
    let result = connection
        .execute_query(
            Select::from("items")
                .columns_named(&[id.reference(), name.reference(), score.reference()])
                .where_(id.equals(Value::integer(1)))
                .compile(&dialect)
                .unwrap(),
        )
        .unwrap();
    assert_eq!(
        result.rows,
        vec![vec![
            SqliteValue::Integer(1),
            SqliteValue::Text("first".into()),
            SqliteValue::Real(1.5)
        ]]
    );

    let updated = connection
        .execute_query(
            Update::table("items")
                .set(name.clone(), Value::text("changed"))
                .where_(id.equals(Value::integer(1)))
                .compile(&dialect)
                .unwrap(),
        )
        .unwrap();
    assert_eq!(updated.affected_rows, 1);
    let deleted = connection
        .execute_query(
            Delete::from("items")
                .where_(id.equals(Value::integer(1)))
                .compile(&dialect)
                .unwrap(),
        )
        .unwrap();
    assert_eq!(deleted.affected_rows, 1);
}

#[test]
fn rejects_invalid_identifiers_and_unscoped_writes() {
    let id = Column::<Integer>::new("id");
    let dialect = SqliteDialect;
    assert!(Select::from("").compile(&dialect).is_err());
    assert!(Update::table("items")
        .set(id.clone(), Value::integer(1))
        .compile(&dialect)
        .is_err());
    assert!(Delete::from("items").compile(&dialect).is_err());
    assert!(Select::from("items").limit(-1).compile(&dialect).is_err());
    assert!(Select::from("bad\0name").compile(&dialect).is_err());
}

#[test]
fn executes_compiled_queries_through_the_shared_pool() {
    let path = std::env::temp_dir().join(format!(
        "spectra-r2502-pool-{}.sqlite",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let seed = SqliteConnection::open(&path, std::time::Duration::from_secs(1)).unwrap();
    seed.execute_batch("CREATE TABLE items (id INTEGER PRIMARY KEY, name TEXT NOT NULL, score REAL NOT NULL, active INTEGER NOT NULL);").unwrap();
    let pool = open_pool(
        &path,
        PoolConfig {
            min_size: 1,
            max_size: 2,
            ..PoolConfig::default()
        },
    )
    .unwrap();
    let lease = pool.acquire_blocking().unwrap();
    let id = Column::<Integer>::new("id");
    let name = Column::<Text>::new("name");
    let score = Column::<Real>::new("score");
    let active = Column::<Boolean>::new("active");
    let inserted = lease
        .connection()
        .unwrap()
        .execute_query(
            Insert::into("items")
                .set(id.clone(), Value::integer(10))
                .set(name, Value::text("pooled"))
                .set(score, Value::real(3.0))
                .set(active, Value::boolean(true))
                .compile(&SqliteDialect)
                .unwrap(),
        )
        .unwrap();
    assert_eq!(inserted.affected_rows, 1);
    lease.release().unwrap();
    pool.shutdown().unwrap();
}
