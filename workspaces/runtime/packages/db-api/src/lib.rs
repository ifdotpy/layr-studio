//! Static SQLite row contracts. Applications own database markers and codec policy.
pub use rusqlite;
use rusqlite::{
    Connection, OptionalExtension, Transaction,
    types::{FromSql, ToSql},
};
use std::marker::PhantomData;
pub use studio_db_api_derive::{DbEnum, DbTable};

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
pub use studio_db_api_derive::db_query;

/// A marker value is an adapter capability: applications keep its constructor private.
pub trait Database: Sized {}
/// Mutations cannot cross database markers.
/// ```compile_fail
/// use studio_db_api::{Database, DbTable, Tx};
/// struct Canvas; impl Database for Canvas {}
/// struct Analytics; impl Database for Analytics {}
/// #[derive(DbTable)] #[db(table="typed_rows",database=Canvas)]
/// struct Row { #[db(key)] id: i64 }
/// fn wrong(tx: &mut Tx<'_, Analytics>, row: &Row) { tx.insert(row).unwrap(); }
/// ```
pub struct Tx<'c, D: Database> {
    transaction: &'c Transaction<'c>,
    database: PhantomData<D>,
}
impl<'c, D: Database> Tx<'c, D> {
    /// Only a trusted application adapter can supply the privately constructed marker.
    pub fn from_adapter(transaction: &'c Transaction<'c>, _capability: &D) -> Self {
        Self {
            transaction,
            database: PhantomData,
        }
    }
    #[doc(hidden)]
    pub fn connection(&self) -> &Connection {
        self.transaction
    }
    pub fn get<R: Table<Database = D>>(&self, key: &R::Key) -> rusqlite::Result<Option<R>> {
        let mut statement = self.transaction.prepare_cached(R::GET)?;
        R::bind_key(key, |params| {
            statement
                .query_row(params, |row| R::read(self.transaction, row))
                .optional()
        })
    }
    pub fn insert<R: Table<Database = D>>(&mut self, row: &R) -> rusqlite::Result<()> {
        row.validate()?;
        let mut statement = self.transaction.prepare_cached(R::INSERT)?;
        row.bind(self.transaction, |params| {
            statement.execute(params).map(|_| ())
        })
    }
    pub fn upsert<R: MutableTable<Database = D>>(&mut self, row: &R) -> rusqlite::Result<()> {
        row.validate()?;
        let mut statement = self.transaction.prepare_cached(R::UPSERT)?;
        row.bind(self.transaction, |params| {
            statement.execute(params).map(|_| ())
        })
    }
    pub fn delete<R: MutableTable<Database = D>>(
        &mut self,
        key: &R::Key,
    ) -> rusqlite::Result<usize> {
        let mut statement = self.transaction.prepare_cached(R::DELETE)?;
        R::bind_key(key, |params| statement.execute(params))
    }
    /// Static statements emitted by db_query! share the existing connection cache.
    pub fn query<R: Table<Database = D>>(
        &self,
        sql: &'static str,
        params: &[&dyn ToSql],
    ) -> rusqlite::Result<Vec<R>> {
        let mut statement = self.transaction.prepare_cached(sql)?;
        statement
            .query_map(params, |row| R::read(self.transaction, row))?
            .collect()
    }
}

/// SQL storage is independent of wire spelling. Closed enums supply permanent codes.
pub trait DbValue: ToSql + FromSql {
    #[inline]
    fn read(row: &rusqlite::Row<'_>, index: usize) -> rusqlite::Result<Self> {
        row.get(index)
    }
    const SQL_TYPE: &'static str;
    const NULLABLE: bool = false;
    const CODES: &'static str = "";
    const CODE_NAMES: &'static [(i64, &'static str)] = &[];
}
macro_rules! scalar {
    ($($ty:ty => $sql:literal),+ $(,)?) => { $(impl DbValue for $ty { const SQL_TYPE: &'static str = $sql; })+ };
}
scalar!(i64 => "INTEGER", i32 => "INTEGER", u32 => "INTEGER", f64 => "REAL", String => "TEXT", Vec<u8> => "BLOB");
impl DbValue for bool {
    #[inline]
    fn read(row: &rusqlite::Row<'_>, index: usize) -> rusqlite::Result<Self> {
        match row.get::<_, i64>(index)? {
            0 => Ok(false),
            1 => Ok(true),
            code => Err(rusqlite::Error::IntegralValueOutOfRange(index, code)),
        }
    }
    const SQL_TYPE: &'static str = "INTEGER";
    const CODES: &'static str = "0,1";
}
impl<T: DbValue> DbValue for Option<T> {
    #[inline]
    fn read(row: &rusqlite::Row<'_>, index: usize) -> rusqlite::Result<Self> {
        if row.get_ref(index)?.data_type() == rusqlite::types::Type::Null {
            Ok(None)
        } else {
            T::read(row, index).map(Some)
        }
    }
    const SQL_TYPE: &'static str = T::SQL_TYPE;
    const NULLABLE: bool = true;
    const CODES: &'static str = T::CODES;
    const CODE_NAMES: &'static [(i64, &'static str)] = T::CODE_NAMES;
}
/// Explicit opt-in conversion; JSON validation and preservation belong to the application.
pub trait Codec<T> {
    type Stored: DbValue;
    fn encode(connection: &Connection, value: &T) -> rusqlite::Result<Self::Stored>;
    fn decode(connection: &Connection, value: Self::Stored) -> rusqlite::Result<T>;
}
pub trait Table: Sized + 'static {
    type Database: Database;
    type Key;
    const NAME: &'static str;
    const DDL: &'static str;
    const INDEXES: &'static [IndexDescriptor];
    const SELECT: &'static str;
    const STATE_CHECK: &'static str;
    const VALID_ROWS: &'static str;
    const GET: &'static str;
    const INSERT: &'static str;
    const DELETE: &'static str;
    const COLUMNS: &'static [ColumnDescriptor];
    fn read(connection: &Connection, row: &rusqlite::Row<'_>) -> rusqlite::Result<Self>;
    fn bind<T>(
        &self,
        connection: &Connection,
        f: impl FnOnce(&[&dyn ToSql]) -> rusqlite::Result<T>,
    ) -> rusqlite::Result<T>;
    fn bind_key<T>(
        key: &Self::Key,
        f: impl FnOnce(&[&dyn ToSql]) -> rusqlite::Result<T>,
    ) -> rusqlite::Result<T>;
    fn validate(&self) -> rusqlite::Result<()> {
        Ok(())
    }
    fn descriptor() -> Descriptor {
        Descriptor {
            name: Self::NAME,
            ddl: Self::DDL,
            indexes: Self::INDEXES,
            columns: Self::COLUMNS,
        }
    }
}
/// Receipt owners deliberately do not implement this trait.
/// Receipt rows have no generic overwriting upsert.
/// ```compile_fail
/// use studio_db_api::{Database, DbTable, Tx};
/// struct Canvas; impl Database for Canvas {}
/// #[derive(DbTable)] #[db(table="receipts",database=Canvas,receipt)]
/// struct Receipt { #[db(key)] id: i64, reply: String }
/// fn overwrite(tx: &mut Tx<'_, Canvas>, row: &Receipt) { tx.upsert(row).unwrap(); }
/// ```
pub trait MutableTable: Table {
    const UPSERT: &'static str;
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
pub struct ColumnDescriptor {
    pub name: &'static str,
    pub storage: &'static str,
    pub nullable: bool,
    pub private: bool,
    pub codes: &'static [(i64, &'static str)],
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
pub struct IndexDescriptor {
    pub name: &'static str,
    pub sql: &'static str,
}
#[derive(Clone, Copy, Debug, serde::Serialize)]
pub struct CatalogEntry {
    pub create_order: usize,
    pub name: &'static str,
    pub sql: &'static str,
    pub table: &'static str,
    pub r#type: &'static str,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
pub struct Descriptor {
    pub name: &'static str,
    pub ddl: &'static str,
    pub indexes: &'static [IndexDescriptor],
    pub columns: &'static [ColumnDescriptor],
}
impl Descriptor {
    pub fn ddl_statements(self) -> impl Iterator<Item = &'static str> {
        std::iter::once(self.ddl).chain(self.indexes.iter().map(|index| index.sql))
    }
    pub fn catalog_entries(self, create_order: usize) -> impl Iterator<Item = CatalogEntry> {
        std::iter::once(CatalogEntry {
            create_order,
            name: self.name,
            sql: self.ddl,
            table: self.name,
            r#type: "table",
        })
        .chain(
            self.indexes
                .iter()
                .enumerate()
                .map(move |(i, index)| CatalogEntry {
                    create_order: create_order + i + 1,
                    name: index.name,
                    sql: index.sql,
                    table: self.name,
                    r#type: "index",
                }),
        )
    }
    pub fn catalog_json(self, create_order: usize) -> Result<String, studio_json::Error> {
        studio_json::to_string(
            &self
                .catalog_entries(create_order)
                .next()
                .expect("table entry"),
        )
    }
}
/// Report only; the application's migration runner owns execution and transforms.
#[derive(Debug, PartialEq, Eq)]
pub enum DescriptorDiff {
    Added(&'static str),
    Removed(&'static str),
    Changed(&'static str),
    EnumCodesChanged(&'static str, &'static str),
}
pub fn diff(old: &[Descriptor], new: &[Descriptor]) -> Vec<DescriptorDiff> {
    let mut changes = Vec::new();
    for before in old {
        match new.iter().find(|after| after.name == before.name) {
            None => changes.push(DescriptorDiff::Removed(before.name)),
            Some(after) => {
                if before != after {
                    changes.push(DescriptorDiff::Changed(before.name));
                }
                for column in before.columns {
                    if after
                        .columns
                        .iter()
                        .any(|next| next.name == column.name && next.codes != column.codes)
                    {
                        changes.push(DescriptorDiff::EnumCodesChanged(before.name, column.name));
                    }
                }
            }
        }
    }
    for after in new {
        if !old.iter().any(|before| before.name == after.name) {
            changes.push(DescriptorDiff::Added(after.name));
        }
    }
    changes
}

/// Generated unit structs make row, value and SQL name inseparable.
pub trait Column: Copy {
    type Row: Table;
    type Value;
    type Parameter<'a>: ToSql
    where
        Self::Value: 'a;
    fn encode<'a>(
        connection: &Connection,
        value: &'a Self::Value,
    ) -> rusqlite::Result<Self::Parameter<'a>>;
    const NAME: &'static str;
}
#[doc(hidden)]
pub const fn column_name<R: Table, C: Column<Row = R>>(_column: C) -> &'static str {
    C::NAME
}
#[doc(hidden)]
pub fn parameter<'a, R: Table, C: Column<Row = R>>(
    connection: &Connection,
    _column: C,
    value: &'a C::Value,
) -> rusqlite::Result<C::Parameter<'a>> {
    C::encode(connection, value)
}
/// rarray keeps one bound parameter and one static SQL text for any slice length.
/// An empty slice matches no rows. The connection owner loads the rarray module once.
#[doc(hidden)]
pub fn array_parameter<R: Table, C: Column<Row = R>>(
    connection: &Connection,
    _column: C,
    values: &[C::Value],
) -> rusqlite::Result<rusqlite::vtab::array::Array> {
    let values = values
        .iter()
        .map(|value| {
            let encoded = C::encode(connection, value)?;
            let value = encoded.to_sql()?;
            if let rusqlite::types::ToSqlOutput::Owned(value) = value {
                Ok(value)
            } else if let rusqlite::types::ToSqlOutput::Borrowed(value) = value {
                value
                    .try_into()
                    .map_err(|error| rusqlite::Error::ToSqlConversionFailure(Box::new(error)))
            } else {
                Err(rusqlite::Error::InvalidQuery)
            }
        })
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(std::rc::Rc::new(values))
}

pub struct Registration {
    pub descriptor: Descriptor,
    pub check: fn(&Connection) -> rusqlite::Result<(u64, u64)>,
}
pub fn check<R: Table>(connection: &Connection) -> rusqlite::Result<(u64, u64)> {
    let exists: bool = connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name=?1)",
        [R::NAME],
        |row| row.get(0),
    )?;
    if !exists {
        return Ok((0, 0));
    }
    // SQLite count(*) is a non-negative signed integer.
    let violations = connection
        .prepare_cached(R::STATE_CHECK)?
        .query_row([], |row| row.get::<_, i64>(0))? as u64;
    let mut statement = connection.prepare_cached(R::VALID_ROWS)?;
    let mut rows = statement.query([])?;
    let (mut count, mut rejected) = (violations, violations);
    while let Some(row) = rows.next()? {
        count += 1;
        if R::read(connection, row).is_err() {
            rejected += 1;
        }
    }
    Ok((count, rejected))
}
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
