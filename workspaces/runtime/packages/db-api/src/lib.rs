//! Static SQLite row contracts. Applications own database markers and codec policy.
pub mod catalog;
pub use rusqlite;
mod codec;
pub mod migration;
mod query;
mod table;
pub use codec::*;
pub use query::*;
pub use studio_db_api_derive::{DbEnum, DbPartition, DbProjection, DbTable};
pub use table::*;

/// Generated SQL rejects unknown columns and parameter types.
/// ```compile_fail
/// use studio_db_api::{Database, DbTable, Tx, db_query};
/// struct Canvas; impl Database for Canvas {}
/// #[derive(DbTable)] #[db(table="typed_rows",database=Canvas)]
/// struct Row { #[db(key)] id: i64 }
/// fn query(tx: &Tx<'_, Canvas>) { db_query!(tx, Row, filter(Row::MISSING.eq(&1))).unwrap(); }
/// ```
/// ```compile_fail
/// use studio_db_api::{Database, DbTable, Tx, db_query};
/// struct Canvas; impl Database for Canvas {}
/// #[derive(DbTable)] #[db(table="typed_rows",database=Canvas)]
/// struct Row { #[db(key)] id: i64 }
/// fn query(tx: &Tx<'_, Canvas>) { db_query!(tx, Row, filter(Row::ID.eq(&String::from("bad")))).unwrap(); }
/// ```
/// A different row's column cannot enter this query, including its ordering.
/// ```compile_fail
/// use studio_db_api::{Database, DbTable, Tx, db_query};
/// struct Canvas; impl Database for Canvas {}
/// #[derive(DbTable)] #[db(table="first_rows",database=Canvas)]
/// struct First { #[db(key)] id: i64 }
/// #[derive(DbTable)] #[db(table="second_rows",database=Canvas)]
/// struct Second { #[db(key)] id: i64 }
/// fn query(tx: &Tx<'_, Canvas>) { db_query!(tx, First, order_by(Second::ID.asc)).unwrap(); }
/// ```
/// Identifiers and nullable keys fail at the declared field/table span.
/// ```compile_fail
/// use studio_db_api::{Database, DbTable};
/// struct Canvas; impl Database for Canvas {}
/// #[derive(DbTable)] #[db(table="select",database=Canvas)]
/// struct Bad { #[db(key)] id: i64 }
/// ```
/// ```compile_fail
/// use studio_db_api::{Database, DbTable};
/// struct Canvas; impl Database for Canvas {}
/// #[derive(DbTable)] #[db(table="bad_keys",database=Canvas)]
/// struct Bad { #[db(key)] id: Option<i64> }
/// ```
/// Type aliases cannot hide a nullable key, including a composite key.
/// ```compile_fail
/// use studio_db_api::{Database, DbTable};
/// struct Canvas; impl Database for Canvas {}
/// type NullableId = Option<i64>;
/// #[derive(DbTable)] #[db(table="aliased_keys",database=Canvas)]
/// struct Bad { #[db(key)] id: NullableId, #[db(key)] other: String }
/// ```
pub use studio_db_api_derive::{db_delete, db_query, db_update};

#[macro_export]
macro_rules! db_schema {
    ($database:ty; $($row:ty),* $(,)?) => {{
        fn register<R: $crate::Table<Database = $database>>() -> $crate::Registration {
            $crate::Registration { descriptor: R::descriptor(), check: $crate::check::<R> }
        }
        [$(register::<$row>()),*]
    }};
}
/// Const concatenation supports type-owned SQL fragments without runtime DDL assembly.
pub const fn sql_len(parts: &[&str]) -> usize {
    let mut n = 0;
    let mut i = 0;
    while i < parts.len() {
        n += parts[i].len();
        i += 1;
    }
    n
}
pub const fn sql_bytes<const N: usize>(parts: &[&str]) -> [u8; N] {
    let mut out = [0; N];
    let mut i = 0;
    let mut at = 0;
    while i < parts.len() {
        let bytes = parts[i].as_bytes();
        let mut j = 0;
        while j < bytes.len() {
            out[at] = bytes[j];
            at += 1;
            j += 1;
        }
        i += 1;
    }
    out
}
#[macro_export]
macro_rules! static_sql {
    ($($part:expr),* $(,)?) => {{
        const PARTS: &[&str] = &[$($part),*];
        const BYTES: [u8; $crate::sql_len(PARTS)] = $crate::sql_bytes(PARTS);
        const SQL: &str = match ::std::str::from_utf8(&BYTES) { Ok(sql) => sql, Err(_) => panic!("SQL fragments must be UTF-8") };
        SQL
    }};
}
