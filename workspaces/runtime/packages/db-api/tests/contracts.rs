use studio_db_api::rusqlite::Connection;
use studio_db_api::{Database, DbEnum, DbTable, DbValue, Table, Tx, db_query, db_schema};
struct Canvas;
impl Database for Canvas {}

#[test]
fn read_capability_refuses_delete_returning_before_execution() {
    let db = Connection::open_in_memory().expect("db");
    db.execute_batch(
        "CREATE TABLE text_keys(id TEXT PRIMARY KEY); INSERT INTO text_keys VALUES('kept')",
    )
    .expect("fixture");
    let read = studio_db_api::Read::from_adapter(&db, &Canvas);
    assert!(
        read.query::<TextKey>("DELETE FROM text_keys RETURNING id", &[])
            .is_err()
    );
    assert!(
        read.get::<TextKey>(&("kept".into(),))
            .expect("read")
            .is_some()
    );
}

#[derive(DbTable)]
#[db(table="keyword_bool", database=Canvas, legacy_ddl="CREATE TABLE keyword_bool(id INTEGER PRIMARY KEY, \"select\" INTEGER NOT NULL)")]
struct KeywordBool {
    #[db(key)]
    id: i64,
    #[db(column = "select")]
    flag: bool,
}
#[derive(DbTable)]
#[db(table="automatic", database=Canvas)]
struct Automatic {
    #[db(key, generated)]
    id: i64,
}
#[test]
fn all_generated_rows_still_allow_default_inserts() {
    let mut db = Connection::open_in_memory().expect("db");
    db.execute_batch(Automatic::DDL).expect("DDL");
    let transaction = db.transaction().expect("tx");
    let mut tx = Tx::from_adapter(&transaction, &Canvas);
    assert_eq!(
        tx.insert_rowid(&Automatic { id: 0 })
            .expect("default insert"),
        1
    );
    assert_eq!(tx.get::<Automatic>(&(1,)).expect("get").expect("row").id, 1);
}
#[test]
fn state_constraints_quote_legacy_keyword_columns() {
    let db = Connection::open_in_memory().expect("db");
    db.execute_batch(KeywordBool::DDL).expect("legacy DDL");
    db.execute_batch("INSERT INTO keyword_bool VALUES(1,1),(2,7)")
        .expect("legacy rows");
    assert_eq!(
        studio_db_api::check::<KeywordBool>(&db)
            .expect("quoted constraints")
            .counts(),
        (2, 1)
    );
}

#[derive(DbTable)]
#[db(table = "legacy", database = Canvas,
    legacy_ddl = "CREATE TABLE legacy (id TEXT PRIMARY KEY, end INTEGER NOT NULL, doubled INTEGER GENERATED ALWAYS AS (end * 2) VIRTUAL)",
    legacy_index = "CREATE UNIQUE INDEX legacy_endId ON legacy(end)")]
struct Legacy {
    #[db(key)]
    id: String,
    end: i64,
    #[db(generated)]
    doubled: i64,
}

#[derive(DbTable)]
#[db(table = "numbered", database = Canvas,
    legacy_ddl = "CREATE TABLE numbered (\n seq INTEGER PRIMARY KEY AUTOINCREMENT, text TEXT NOT NULL)")]
struct Numbered {
    #[db(key, generated)]
    seq: i64,
    text: String,
}

#[test]
fn legacy_ddl_generated_fields_read_capability_and_typed_mutations() {
    use studio_db_api::{Read, db_delete, db_update};
    let mut db = Connection::open_in_memory().expect("db");
    for descriptor in [Legacy::descriptor(), Numbered::descriptor()] {
        for ddl in descriptor.ddl_statements() {
            db.execute_batch(ddl).expect("legacy DDL");
        }
        let actual: String = db
            .query_row(
                "SELECT sql FROM sqlite_master WHERE name=?1",
                [descriptor.name],
                |r| r.get(0),
            )
            .expect("catalog");
        assert_eq!(actual, descriptor.ddl);
    }
    let transaction = db.transaction().expect("tx");
    let mut tx = Tx::from_adapter(&transaction, &Canvas);
    tx.insert(&Legacy {
        id: "one".into(),
        end: 3,
        doubled: 99,
    })
    .expect("generated field omitted");
    let id = tx
        .insert_rowid(&Numbered {
            seq: 99,
            text: "auto".into(),
        })
        .expect("automatic key");
    assert_eq!(id, 1);
    assert!(
        tx.insert_new(&Legacy {
            id: "other".into(),
            end: 3,
            doubled: 0
        })
        .is_err(),
        "other unique conflicts must not be ignored"
    );
    assert!(
        !tx.insert_new(&Legacy {
            id: "one".into(),
            end: 30,
            doubled: 0
        })
        .expect("key conflict only")
    );
    assert_eq!(
        tx.get::<Numbered>(&(id,)).expect("get").expect("row").seq,
        1
    );
    assert_eq!(
        db_update!(
            tx,
            Legacy,
            set(Legacy::END = &4),
            filter(Legacy::ID.eq(&"one".into()))
        )
        .expect("update"),
        1
    );
    let row = tx
        .get::<Legacy>(&("one".into(),))
        .expect("get")
        .expect("row");
    assert_eq!((row.end, row.doubled), (4, 8));
    tx.upsert(&Legacy {
        id: "one".into(),
        end: 5,
        doubled: 99,
    })
    .expect("upsert omits generated field");
    assert_eq!(
        db_delete!(tx, Legacy, filter(Legacy::END.lt(&0))).expect("no match"),
        0
    );
    assert_eq!(
        db_delete!(tx, Legacy, filter(Legacy::END.eq(&5))).expect("delete"),
        1
    );
    transaction.commit().expect("commit");
    let read = Read::from_adapter(&db, &Canvas);
    assert_eq!(
        read.get::<Numbered>(&(1,))
            .expect("read get")
            .expect("row")
            .text,
        "auto"
    );
    assert_eq!(
        db_query!(read, Numbered, filter(Numbered::SEQ.eq(&1)))
            .expect("read query")
            .len(),
        1
    );
    db.execute("INSERT INTO legacy(id,end) VALUES(NULL,2)", [])
        .expect("legacy nullable key");
    assert_eq!(
        studio_db_api::check::<Legacy>(&db)
            .expect("legacy check")
            .counts(),
        (1, 1)
    );
}

#[derive(DbTable)]
#[db(table = "semantic", database = Canvas, state_check = semantic_check, legacy_ddl = "CREATE TABLE semantic(id PRIMARY KEY)")]
struct Semantic {
    #[db(key)]
    id: i64,
}
fn semantic_check(
    connection: &Connection,
) -> studio_db_api::rusqlite::Result<studio_db_api::StateCheck> {
    let (row_count, rejected_count) = connection.query_row(
        "SELECT count(*), count(*) FILTER (WHERE id=2) FROM semantic",
        [],
        |row| Ok((row.get::<_, i64>(0)? as u64, row.get::<_, i64>(1)? as u64)),
    )?;
    Ok(studio_db_api::StateCheck {
        row_count,
        rejected_count,
        failures: vec![studio_db_api::StateCheckFailure {
            pointer: "/id".into(),
            keyword: "semantic".into(),
            count: 1,
        }],
    })
}
#[test]
fn state_check_hook_preserves_details_without_changing_writes() {
    let mut db = Connection::open_in_memory().expect("db");
    Semantic::descriptor()
        .install_if_absent(&db)
        .expect("install");
    Semantic::descriptor()
        .install_if_absent(&db)
        .expect("idempotent");
    let transaction = db.transaction().expect("tx");
    let mut tx = Tx::from_adapter(&transaction, &Canvas);
    tx.insert(&Semantic { id: 2 })
        .expect("write admits existing contract");
    tx.upsert(&Semantic { id: 2 })
        .expect("upsert admits existing contract");
    assert!(tx.get::<Semantic>(&(2,)).expect("read").is_some());
    transaction
        .execute("INSERT INTO semantic VALUES('historical')", [])
        .expect("legacy row accepted by the existing checker");
    let report = studio_db_api::check::<Semantic>(&transaction).expect("state check");
    assert_eq!(report.counts(), (2, 1));
    assert_eq!(
        report.failures,
        vec![studio_db_api::StateCheckFailure {
            pointer: "/id".into(),
            keyword: "semantic".into(),
            count: 1
        }]
    );
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, DbEnum)]
#[repr(i64)]
enum Status {
    Ready = 3,
    Ended = 19,
}
#[derive(Debug, PartialEq, DbTable)]
#[db(table = "witness", database = Canvas, strict, without_rowid)]
#[db(index(name = "witness_active", columns(state), unique, predicate = state.ne(Status::Ended)))]
struct Witness {
    #[db(key)]
    owner: String,
    #[db(key)]
    ordinal: i64,
    state: Status,
    detail: Option<String>,
}
#[test]
fn integer_enum_composite_key_predicate_and_crud() {
    let mut db = Connection::open_in_memory().expect("db");
    studio_db_api::rusqlite::vtab::array::load_module(&db).expect("array module");
    for registration in db_schema![Canvas; Witness] {
        db.execute_batch(registration.descriptor.ddl).expect("DDL");
        for sql in registration.descriptor.indexes {
            db.execute_batch(sql.sql).expect("index");
        }
    }
    let catalog = Witness::descriptor().catalog_entries(1).collect::<Vec<_>>();
    assert_eq!(catalog.len(), 2);
    assert_eq!(catalog[1].name, "witness_active");
    assert_eq!(catalog[1].create_order, 2);
    let transaction = db.transaction().expect("transaction");
    let mut tx = Tx::from_adapter(&transaction, &Canvas);
    let row = Witness {
        owner: "a".into(),
        ordinal: 1,
        state: Status::Ready,
        detail: None,
    };
    tx.insert(&row).expect("insert");
    assert_eq!(tx.get::<Witness>(&("a".into(), 1)).expect("get"), Some(row));
    assert_eq!(Status::CODE_NAMES, &[(3, "Ready"), (19, "Ended")]);
    assert!(
        transaction
            .execute("INSERT INTO witness VALUES('b',1,42,NULL)", [])
            .is_err()
    );
    assert!(
        transaction
            .execute("INSERT INTO witness VALUES('b',1,3,NULL)", [])
            .is_err(),
        "predicate unique index"
    );
    let changed = Witness {
        owner: "a".into(),
        ordinal: 1,
        state: Status::Ended,
        detail: Some("done".into()),
    };
    tx.upsert(&changed).expect("upsert");
    tx.insert(&Witness {
        owner: "b".into(),
        ordinal: 2,
        state: Status::Ended,
        detail: None,
    })
    .expect("predicate excludes ended rows");
    let status = Status::Ended;
    assert_eq!(
        db_query!(
            tx,
            Witness,
            limit(1),
            order_by(Witness::ORDINAL.desc, Witness::OWNER.asc),
            filter(Witness::STATE.eq(&status), Witness::DETAIL.is_not_null())
        )
        .expect("select"),
        vec![changed]
    );
    let empty = [];
    assert_eq!(
        db_query!(tx, Witness, limit(-1_i64))
            .expect("negative limit is unbounded")
            .len(),
        2
    );
    assert!(
        db_query!(tx, Witness, filter(Witness::ORDINAL.in_(&empty)))
            .expect("empty IN")
            .is_empty()
    );
    let many = vec![2; 1000];
    assert_eq!(
        db_query!(tx, Witness, filter(Witness::ORDINAL.in_(&many)))
            .expect("one array binding")
            .len(),
        1
    );
    assert_eq!(tx.delete::<Witness>(&("a".into(), 1)).expect("delete"), 1);
    assert_eq!(std::mem::size_of_val(&Witness::STATE), 0);
}
#[test]
fn descriptor_diff_flags_permanent_code_changes() {
    let before = Witness::descriptor();
    let after = studio_db_api::Descriptor {
        columns: &[studio_db_api::ColumnDescriptor {
            name: "state",
            storage: "INTEGER",
            nullable: false,
            private: false,
            codes: &[(4, "Ready")],
        }],
        ..before
    };
    assert!(studio_db_api::diff(&[before], &[after]).contains(
        &studio_db_api::DescriptorDiff::EnumCodesChanged("witness", "state")
    ));
}

struct Count(i64);
struct CountCodec;
impl studio_db_api::Codec<Count> for CountCodec {
    type Stored = i64;
    fn encode(_: &Connection, value: &Count) -> studio_db_api::rusqlite::Result<i64> {
        Ok(value.0)
    }
    fn decode(_: &Connection, value: i64) -> studio_db_api::rusqlite::Result<Count> {
        if value < 0 {
            return Err(studio_db_api::rusqlite::Error::FromSqlConversionFailure(
                2,
                studio_db_api::rusqlite::types::Type::Integer,
                Box::new(studio_db_api::StateCheckFailure {
                    pointer: "/count".into(),
                    keyword: "minimum".into(),
                    count: 1,
                }),
            ));
        }
        Ok(Count(value))
    }
}
#[derive(DbTable)]
#[db(table="parents",database=Canvas)]
struct Parent {
    #[db(key, column = "parent_id")]
    id: i64,
}
#[derive(DbTable)]
#[db(table="children",database=Canvas)]
struct Child {
    #[db(key)]
    id: i64,
    #[db(fk = Parent::id)]
    parent: i64,
    #[db(codec = CountCodec, private)]
    count: Count,
}
#[test]
fn renamed_foreign_key_custom_codec_and_private_metadata() {
    let mut db = Connection::open_in_memory().expect("db");
    studio_db_api::rusqlite::vtab::array::load_module(&db).expect("array module");
    db.execute_batch("PRAGMA foreign_keys=ON").expect("FK");
    for registration in db_schema![Canvas; Parent, Child] {
        db.execute_batch(registration.descriptor.ddl).expect("DDL");
    }
    let transaction = db.transaction().expect("tx");
    let mut tx = Tx::from_adapter(&transaction, &Canvas);
    assert!(
        tx.insert(&Child {
            id: 1,
            parent: 9,
            count: Count(3)
        })
        .is_err()
    );
    tx.insert(&Parent { id: 9 }).expect("parent");
    tx.insert(&Child {
        id: 1,
        parent: 9,
        count: Count(3),
    })
    .expect("child");
    assert_eq!(
        tx.get::<Child>(&(1,)).expect("read").expect("row").count.0,
        3
    );
    assert!(Child::COLUMNS[2].private);
    assert_eq!(
        studio_db_api::check::<Child>(&transaction)
            .expect("check")
            .counts(),
        (1, 0)
    );
}

#[derive(DbTable)]
#[db(table="text_keys",database=Canvas)]
struct TextKey {
    #[db(key)]
    id: String,
}
#[test]
fn non_integer_primary_keys_reject_null() {
    let db = Connection::open_in_memory().expect("db");
    db.execute_batch(TextKey::DDL).expect("DDL");
    assert!(
        db.execute("INSERT INTO text_keys VALUES(NULL)", [])
            .is_err()
    );
    let not_null: i64 = db
        .query_row(
            "SELECT \"notnull\" FROM pragma_table_info('text_keys') WHERE name='id'",
            [],
            |r| r.get(0),
        )
        .expect("physical constraint");
    assert_eq!(not_null, 1);
}

#[test]
fn codec_columns_use_the_same_encoder_for_filters_arrays_and_order() {
    let mut db = Connection::open_in_memory().expect("db");
    studio_db_api::rusqlite::vtab::array::load_module(&db).expect("array");
    db.execute_batch(Parent::DDL).expect("parent");
    db.execute_batch(Child::DDL).expect("child");
    let transaction = db.transaction().expect("transaction");
    let mut tx = Tx::from_adapter(&transaction, &Canvas);
    tx.insert(&Parent { id: 1 }).expect("parent");
    tx.insert(&Child {
        id: 1,
        parent: 1,
        count: Count(30),
    })
    .expect("first");
    tx.insert(&Child {
        id: 2,
        parent: 1,
        count: Count(10),
    })
    .expect("second");
    let same = Count(30);
    let rows = db_query!(
        tx,
        Child,
        filter(Child::COUNT.eq(&same)),
        order_by(Child::COUNT.asc)
    )
    .expect("codec eq");
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].id, 1);
    let values = [Count(30), Count(10)];
    let rows = db_query!(
        tx,
        Child,
        filter(Child::COUNT.in_(&values), Child::COUNT.ne(&Count(99))),
        order_by(Child::COUNT.asc)
    )
    .expect("codec array and order");
    assert_eq!(rows[0].id, 2);
    assert_eq!(rows[1].id, 1);
    use studio_db_api::{db_delete, db_update};
    assert_eq!(
        db_update!(
            tx,
            Child,
            set(Child::COUNT = &Count(40)),
            filter(Child::COUNT.in_(&values))
        )
        .expect("codec update"),
        2
    );
    assert_eq!(
        db_delete!(tx, Child, filter(Child::COUNT.eq(&Count(40)))).expect("codec delete"),
        2
    );
    transaction
        .execute("INSERT INTO children VALUES(1,1,-1),(2,1,-2)", [])
        .expect("invalid codec rows");
    let report = studio_db_api::check::<Child>(&transaction).expect("decode diagnostics");
    assert_eq!(report.counts(), (2, 2));
    assert_eq!(
        report.failures,
        vec![studio_db_api::StateCheckFailure {
            pointer: "/count".into(),
            keyword: "minimum".into(),
            count: 2
        }]
    );
}
#[test]
fn nullable_comparisons_follow_sql_null_semantics() {
    let mut db = Connection::open_in_memory().expect("db");
    db.execute_batch(Witness::DDL).expect("DDL");
    let transaction = db.transaction().expect("tx");
    let mut tx = Tx::from_adapter(&transaction, &Canvas);
    tx.insert(&Witness {
        owner: "null".into(),
        ordinal: 1,
        state: Status::Ready,
        detail: None,
    })
    .expect("null");
    tx.insert(&Witness {
        owner: "text".into(),
        ordinal: 2,
        state: Status::Ready,
        detail: Some("x".into()),
    })
    .expect("text");
    assert!(
        db_query!(tx, Witness, filter(Witness::DETAIL.eq(&None)))
            .expect("eq NULL")
            .is_empty()
    );
    assert!(
        db_query!(tx, Witness, filter(Witness::DETAIL.ne(&None)))
            .expect("ne NULL")
            .is_empty()
    );
    assert_eq!(
        db_query!(tx, Witness, filter(Witness::DETAIL.is_null())).expect("IS NULL")[0].owner,
        "null"
    );
    assert_eq!(
        db_query!(tx, Witness, filter(Witness::DETAIL.is_not_null())).expect("IS NOT NULL")[0]
            .owner,
        "text"
    );
}

#[derive(DbTable)]
#[db(table="declared_constraints",database=Canvas)]
struct CheckedRow {
    #[db(key, check = "id=1")]
    id: i64,
    flag: bool,
}
#[test]
fn state_check_enforces_declared_constraints_and_strict_bool_without_double_counting() {
    let mut db = Connection::open_in_memory().expect("db");
    db.execute_batch(CheckedRow::DDL).expect("DDL");
    db.execute_batch(
        "PRAGMA ignore_check_constraints=ON; INSERT INTO declared_constraints VALUES(2,7)",
    )
    .expect("corrupted fixture");
    assert_eq!(
        studio_db_api::check::<CheckedRow>(&db)
            .expect("check")
            .counts(),
        (1, 1)
    );
    let transaction = db.transaction().expect("tx");
    assert!(
        Tx::from_adapter(&transaction, &Canvas)
            .get::<CheckedRow>(&(2,))
            .is_err()
    );
    let unchecked = Connection::open_in_memory().expect("unchecked schema");
    unchecked.execute_batch("CREATE TABLE declared_constraints(id INTEGER,flag INTEGER); INSERT INTO declared_constraints VALUES(2,0),(1,1),(NULL,1)").expect("missing physical constraints");
    assert_eq!(
        studio_db_api::check::<CheckedRow>(&unchecked)
            .expect("declared checks")
            .counts(),
        (3, 2)
    );
}

#[test]
fn bool_and_optional_bool_admit_only_integer_zero_and_one() {
    let db = Connection::open_in_memory().expect("db");
    assert!(
        !db.query_row("SELECT 0", [], |row| <bool as DbValue>::read(row, 0))
            .expect("false")
    );
    assert!(
        db.query_row("SELECT 1", [], |row| <bool as DbValue>::read(row, 0))
            .expect("true")
    );
    for sql in [
        "SELECT 7",
        "SELECT -1",
        "SELECT 1.0",
        "SELECT '1'",
        "SELECT NULL",
        "SELECT x'01'",
    ] {
        assert!(
            db.query_row(sql, [], |row| <bool as DbValue>::read(row, 0))
                .is_err(),
            "{sql}"
        );
    }
    assert_eq!(
        db.query_row("SELECT NULL", [], |row| <Option<bool> as DbValue>::read(
            row, 0
        ))
        .expect("optional NULL"),
        None
    );
    assert!(
        db.query_row("SELECT 7", [], |row| <Option<bool> as DbValue>::read(
            row, 0
        ))
        .is_err()
    );
}

#[derive(DbTable)]
#[db(table="nullable_checks",database=Canvas)]
struct NullableCheckRow {
    #[db(key)]
    id: i64,
    #[db(check = "value=1")]
    value: Option<i64>,
}
#[test]
fn state_check_preserves_sqlite_null_check_semantics() {
    let db = Connection::open_in_memory().expect("db");
    db.execute_batch(NullableCheckRow::DDL).expect("DDL");
    db.execute_batch("PRAGMA ignore_check_constraints=ON; INSERT INTO nullable_checks VALUES(1,1),(2,2),(3,NULL)").expect("fixture");
    assert_eq!(
        studio_db_api::check::<NullableCheckRow>(&db)
            .expect("all three rows checked")
            .counts(),
        (3, 1)
    );
}
