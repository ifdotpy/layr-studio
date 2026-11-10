use studio_db_api::rusqlite::{Connection, Error};
use studio_db_api::{Database, DbQuery, DbTable, Read, Table, Tx, db_queries};
struct Canvas;
impl Database for Canvas {}
#[derive(DbTable)]
#[db(table="named_rows",database=Canvas)]
struct Row {
    #[db(key)]
    id: i64,
    count: i64,
    enabled: bool,
}

#[db_queries]
mod queries {
    use super::*;
    #[derive(DbQuery)]
    #[db_query(database=Canvas,mode=read,cardinality=one,result=i64,decode=Row::COUNT,
        tables(rows=Row),columns(count=Row::COUNT),sql="SELECT {count} AS {count} FROM {rows}")]
    pub struct One;
    #[derive(DbQuery)]
    #[db_query(database=Canvas,mode=read,cardinality=optional,result=i64,decode=Row::COUNT,
        tables(rows=Row),columns(count=Row::COUNT),sql="SELECT {count} AS {count} FROM {rows}")]
    pub struct Optional;
    #[derive(DbQuery)]
    #[db_query(database=Canvas,mode=read,cardinality=many,result=bool,decode=Row::ENABLED,
        tables(rows=Row),columns(enabled=Row::ENABLED),sql="SELECT {enabled} AS {enabled} FROM {rows}")]
    pub struct Flags;
    #[derive(DbQuery)]
    #[db_query(database=Canvas,mode=write,cardinality=one,result=i64,decode=Row::COUNT,
        tables(rows=Row),columns(count=Row::COUNT,id=Row::ID),
        sql="UPDATE {rows} SET {count}=?1 WHERE {id}>=?2 RETURNING {count} AS {count}")]
    pub struct Set {
        #[db(column=Row::COUNT)]
        pub count: i64,
        #[db(column=Row::ID)]
        pub from: i64,
    }
    #[derive(DbQuery)]
    #[db_query(database=Canvas,mode=write,cardinality=many,result=bool,decode=Row::ENABLED,
        tables(rows=Row),columns(enabled=Row::ENABLED,count=Row::COUNT,id=Row::ID),
        sql="UPDATE {rows} SET {count}=?1 RETURNING CASE WHEN {id}=1 THEN {enabled} ELSE 2 END AS {enabled}")]
    pub struct BadFlags {
        #[db(column=Row::COUNT)]
        pub code: i64,
    }
    #[derive(DbQuery)]
    #[db_query(database=Canvas,mode=write,cardinality=none,result=(),
        tables(rows=Row),columns(id=Row::ID),sql="DELETE FROM {rows} WHERE {id}=?1")]
    pub struct Delete {
        #[db(column=Row::ID)]
        pub id: i64,
    }
}
fn fixture() -> Connection {
    let db = Connection::open_in_memory().expect("in-memory named-query fixture");
    Row::descriptor()
        .install_if_absent(&db)
        .expect("named-query table");
    db
}
#[test]
fn named_queries_validate_cardinality_codecs_and_returning_rollback() {
    let mut db = fixture();
    queries::validate_queries(&db).unwrap();
    let read = Read::from_adapter(&db, &Canvas);
    assert!(matches!(
        read.named(&queries::One::QUERY, &queries::One),
        Err(Error::QueryReturnedNoRows)
    ));
    assert_eq!(
        read.named(&queries::Optional::QUERY, &queries::Optional)
            .unwrap(),
        None
    );
    let tx = db.transaction().unwrap();
    let mut adapter = Tx::from_adapter(&tx, &Canvas);
    adapter
        .insert(&Row {
            id: 1,
            count: 7,
            enabled: true,
        })
        .unwrap();
    assert_eq!(
        adapter
            .read()
            .named(&queries::One::QUERY, &queries::One)
            .unwrap(),
        7
    );
    adapter
        .insert(&Row {
            id: 2,
            count: 8,
            enabled: false,
        })
        .unwrap();
    assert!(
        adapter
            .read()
            .named(&queries::One::QUERY, &queries::One)
            .is_err()
    );
    assert!(
        adapter
            .read()
            .named(&queries::Optional::QUERY, &queries::Optional)
            .is_err()
    );
    // Multiple RETURNING rows violate One; the entire statement is rolled back.
    assert!(
        adapter
            .named(&queries::Set::QUERY, &queries::Set { count: 99, from: 1 })
            .is_err()
    );
    assert_eq!(adapter.get::<Row>(&(1,)).unwrap().unwrap().count, 7);
    assert_eq!(adapter.get::<Row>(&(2,)).unwrap().unwrap().count, 8);
    assert_eq!(
        adapter
            .named(&queries::Set::QUERY, &queries::Set { count: 9, from: 2 })
            .unwrap(),
        9
    );
    // A decode failure must roll back even rows SQLite updated before yielding.
    assert!(
        adapter
            .collect(&queries::BadFlags::QUERY, &queries::BadFlags { code: 2 })
            .is_err()
    );
    assert!(
        adapter
            .visit(
                &queries::BadFlags::QUERY,
                &queries::BadFlags { code: 3 },
                |_| Err(Error::InvalidQuery)
            )
            .is_err()
    );
    assert_eq!(adapter.get::<Row>(&(1,)).unwrap().unwrap().count, 7);
    assert_eq!(adapter.get::<Row>(&(2,)).unwrap().unwrap().count, 9);
    assert_eq!(
        adapter
            .read()
            .collect(&queries::Flags::QUERY, &queries::Flags)
            .unwrap(),
        vec![true, false]
    );
    assert_eq!(
        adapter
            .named(&queries::Delete::QUERY, &queries::Delete { id: 1 })
            .unwrap(),
        1
    );
    tx.commit().unwrap();
}

#[test]
fn named_queries_reuse_the_connection_statement_cache() {
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };
    use studio_db_api::rusqlite::hooks::{AuthContext, Authorization};
    let db = fixture();
    let count = Arc::new(AtomicUsize::new(0));
    let observed = count.clone();
    db.authorizer(Some(move |_: AuthContext<'_>| {
        observed.fetch_add(1, Ordering::Relaxed);
        Authorization::Allow
    }))
    .unwrap();
    let read = Read::from_adapter(&db, &Canvas);
    read.named(&queries::Optional::QUERY, &queries::Optional)
        .unwrap();
    let first = count.load(Ordering::Relaxed);
    assert!(first > 0, "first query is prepared");
    read.named(&queries::Optional::QUERY, &queries::Optional)
        .unwrap();
    assert_eq!(
        count.load(Ordering::Relaxed),
        first,
        "cached statement is not prepared again"
    );
}

#[derive(DbQuery)]
#[db_query(database=Canvas,mode=read,cardinality=one,result=i64,decode=Row::COUNT,
    columns(count=Row::COUNT),sql="SELECT NULL AS {count}")]
struct NullCount;
#[derive(DbQuery)]
#[db_query(database=Canvas,mode=read,cardinality=one,result=i64,decode=Row::COUNT,
    columns(count=Row::COUNT),sql="SELECT ?2 AS {count}")]
struct Hole {
    #[db(column=Row::COUNT)]
    count: i64,
}
#[derive(DbQuery)]
#[db_query(database=Canvas,mode=read,cardinality=one,result=i64,decode=Row::COUNT,
    columns(count=Row::COUNT),tables(rows=Row),sql="SELECT {count} AS {count},{count} AS {count} FROM {rows}")]
struct WrongWidth;
#[test]
fn named_queries_reject_null_decodes_and_invalid_statement_shapes() {
    let db = fixture();
    NullCount::validate_query(&db).unwrap();
    assert!(
        Read::from_adapter(&db, &Canvas)
            .named(&NullCount::QUERY, &NullCount)
            .is_err()
    );
    assert!(Hole::validate_query(&db).is_err());
    assert!(WrongWidth::validate_query(&db).is_err());
}

#[derive(studio_db_api::DbProjection)]
#[db(of=Row)]
struct IdentityAndCount {
    id: i64,
    count: i64,
}
#[derive(DbQuery)]
#[db_query(database=Canvas,mode=read,cardinality=many,result=IdentityAndCount,projection=IdentityAndCount,
    tables(rows=Row),columns(id=Row::ID,count=Row::COUNT),
    sql="SELECT {count} AS {count},{id} AS {id} FROM {rows}")]
struct Swapped;
#[test]
fn named_queries_reject_swapped_result_mapping() {
    let db = fixture();
    assert!(Swapped::validate_query(&db).is_err());
    let mut db = fixture();
    let tx = db.transaction().unwrap();
    let mut adapter = Tx::from_adapter(&tx, &Canvas);
    adapter
        .insert(&Row {
            id: 1,
            count: 2,
            enabled: true,
        })
        .unwrap();
    let mut observed = None;
    adapter
        .read()
        .visit(&Swapped::QUERY, &Swapped, |row| {
            observed = Some((row.id, row.count));
            Ok(())
        })
        .unwrap();
    assert_eq!(
        observed,
        Some((2, 1)),
        "the validation error prevents silently exchanging two INTEGER fields"
    );
}

#[derive(DbTable)]
#[db(table="hidden",database=Canvas)]
struct Hidden {
    #[db(key)]
    id: i64,
    count: i64,
}
#[derive(DbQuery)]
#[db_query(database=Canvas,mode=read,cardinality=one,result=i64,decode=Row::COUNT,
    columns(count=Row::COUNT),sql="SELECT {count} AS {count} FROM 'hidden'")]
struct QuotedHiddenRead;
#[derive(DbQuery)]
#[db_query(database=Canvas,mode=write,cardinality=none,result=(),sql="DELETE FROM 'hidden'")]
struct QuotedHiddenDelete;
#[derive(DbQuery)]
#[db_query(database=Canvas,mode=read,cardinality=many,result=i64,decode=Row::COUNT,
    tables(rows=Row),columns(count=Row::COUNT),sql="SELECT {count} AS {count} FROM {rows}, pragma_table_info('hidden')")]
struct CommaPragma;
fn hidden_fixture() -> Connection {
    let db = fixture();
    Hidden::descriptor()
        .install_if_absent(&db)
        .expect("undeclared-table witness");
    db
}
#[test]
fn named_queries_reject_single_quoted_hidden_read() {
    assert!(QuotedHiddenRead::validate_query(&hidden_fixture()).is_err());
}
#[test]
fn named_queries_reject_single_quoted_hidden_delete() {
    assert!(QuotedHiddenDelete::validate_query(&hidden_fixture()).is_err());
}
#[test]
fn named_queries_reject_comma_join_pragma_table_function() {
    assert!(CommaPragma::validate_query(&hidden_fixture()).is_err());
}

#[derive(DbQuery)]
#[db_query(database=Canvas,mode=read,cardinality=one,result=i64,decode=Row::COUNT,
    tables(rows=Row),columns(count=Row::COUNT),sql="SELECT COUNT(*) AS {count} FROM {rows}")]
struct RowCount;
#[derive(DbQuery)]
#[db_query(database=Canvas,mode=read,cardinality=one,result=i64,decode=Row::COUNT,
    columns(count=Row::COUNT),sql="SELECT COUNT(*) AS {count} FROM 'hidden'")]
struct HiddenCount;
#[test]
fn named_query_count_allows_only_declared_empty_column_reads() {
    let db = hidden_fixture();
    RowCount::validate_query(&db).unwrap();
    assert_eq!(
        Read::from_adapter(&db, &Canvas)
            .named(&RowCount::QUERY, &RowCount)
            .unwrap(),
        0
    );
    assert!(HiddenCount::validate_query(&db).is_err());
    RowCount::validate_query(&db).unwrap();
}
#[test]
fn named_query_validation_refuses_attached_and_temp_schemas_with_typed_error() {
    let db = fixture();
    db.execute_batch("ATTACH DATABASE ':memory:' AS secondary")
        .unwrap();
    assert!(
        matches!(RowCount::validate_query(&db),Err(studio_db_api::QueryValidationError::NonMainSchema{schema}) if schema=="secondary")
    );
    let db = fixture();
    db.execute_batch("CREATE TEMP TABLE validation_temp(id INTEGER)")
        .unwrap();
    assert!(
        matches!(RowCount::validate_query(&db),Err(studio_db_api::QueryValidationError::NonMainSchema{schema}) if schema=="temp")
    );
}
