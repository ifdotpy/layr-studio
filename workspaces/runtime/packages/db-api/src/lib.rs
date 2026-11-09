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
pub use studio_db_api_derive::{db_delete, db_query, db_update};

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
/// A capability-bound read view; it cannot execute mutations through this API.
/// ```compile_fail
/// use studio_db_api::{Database, DbTable, Read, db_delete};
/// struct Canvas; impl Database for Canvas {}
/// #[derive(DbTable)] #[db(table="typed_rows", database=Canvas)]
/// struct Row { #[db(key)] id: i64 }
/// fn mutate(mut read: Read<'_, Canvas>) { db_delete!(read, Row).unwrap(); }
/// ```
/// ```compile_fail
/// use studio_db_api::{Database, Read};
/// struct Canvas; impl Database for Canvas {}
/// fn leak(read: Read<'_, Canvas>) { read.connection().execute("DELETE FROM rows", []).unwrap(); }
/// ```
pub struct Read<'c, D: Database> {
    connection: &'c Connection,
    database: PhantomData<D>,
}
impl<'c, D: Database> Read<'c, D> {
    #[doc(hidden)]
    pub fn read(&self) -> &Self {
        self
    }
    pub fn from_adapter(connection: &'c Connection, _capability: &D) -> Self {
        Self {
            connection,
            database: PhantomData,
        }
    }
    fn prepare_read(&self, sql: &'static str) -> rusqlite::Result<rusqlite::CachedStatement<'_>> {
        let statement = self.connection.prepare_cached(sql)?;
        if !statement.readonly() {
            return Err(rusqlite::Error::InvalidQuery);
        }
        Ok(statement)
    }
    #[doc(hidden)]
    pub fn parameter<'a, R: Table<Database = D>, C: Column<Row = R>>(
        &self,
        column: C,
        value: &'a C::Value,
    ) -> rusqlite::Result<C::Parameter<'a>> {
        parameter::<R, C>(self.connection, column, value)
    }
    #[doc(hidden)]
    pub fn array_parameter<R: Table<Database = D>, C: Column<Row = R>>(
        &self,
        column: C,
        values: &[C::Value],
    ) -> rusqlite::Result<rusqlite::vtab::array::Array> {
        array_parameter::<R, C>(self.connection, column, values)
    }
    pub fn get<R: Table<Database = D>>(&self, key: &R::Key) -> rusqlite::Result<Option<R>> {
        let mut statement = self.prepare_read(R::GET)?;
        R::bind_key(key, |params| {
            statement
                .query_row(params, |row| R::read(self.connection, row))
                .optional()
        })
    }
    pub fn query<R: Table<Database = D>>(
        &self,
        sql: &'static str,
        params: &[&dyn ToSql],
    ) -> rusqlite::Result<Vec<R>> {
        let mut statement = self.prepare_read(sql)?;
        statement
            .query_map(params, |row| R::read(self.connection, row))?
            .collect()
    }
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
        self.read().get::<R>(key)
    }
    pub fn insert<R: Table<Database = D>>(&mut self, row: &R) -> rusqlite::Result<()> {
        row.validate()?;
        let mut statement = self.transaction.prepare_cached(R::INSERT)?;
        row.bind(self.transaction, |params| {
            statement.execute(params).map(|_| ())
        })
    }
    pub fn insert_rowid<R: Table<Database = D>>(&mut self, row: &R) -> rusqlite::Result<i64> {
        self.insert(row)?;
        Ok(self.transaction.last_insert_rowid())
    }
    pub fn insert_new<R: MutableTable<Database = D>>(&mut self, row: &R) -> rusqlite::Result<bool> {
        row.validate()?;
        let mut statement = self.transaction.prepare_cached(R::INSERT_NEW)?;
        row.bind(self.transaction, |params| {
            statement.execute(params).map(|changed| changed != 0)
        })
    }
    pub fn read(&self) -> Read<'_, D> {
        Read {
            connection: self.transaction,
            database: PhantomData,
        }
    }
    #[doc(hidden)]
    pub fn execute<R: MutableTable<Database = D>>(
        &mut self,
        sql: &'static str,
        params: &[&dyn ToSql],
    ) -> rusqlite::Result<usize> {
        self.transaction.prepare_cached(sql)?.execute(params)
    }
    #[doc(hidden)]
    pub fn update<R: UpdateTable<Database = D>>(
        &mut self,
        sql: &'static str,
        params: &[&dyn ToSql],
    ) -> rusqlite::Result<usize> {
        self.execute::<R>(sql, params)
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
        self.read().query::<R>(sql, params)
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
    fn state_check(connection: &Connection) -> rusqlite::Result<StateCheck> {
        check_rows::<Self>(connection)
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
    const INSERT_NEW: &'static str;
}
/// Checked tables require validated full-row writes.
/// ```compile_fail
/// use studio_db_api::{Database, DbTable, Tx, db_update};
/// struct Canvas; impl Database for Canvas {}
/// #[derive(DbTable)] #[db(table="checked", database=Canvas, check=validate)]
/// struct Row { #[db(key)] id: i64, count: i64 }
/// fn validate(_: &Row) -> std::io::Result<()> { Ok(()) }
/// fn update(mut tx: Tx<'_, Canvas>) { db_update!(tx, Row, set(Row::COUNT = &-1)).unwrap(); }
/// ```
/// All-generated rows support insertion, but not conflict-based mutations.
/// ```compile_fail
/// use studio_db_api::{Database, DbTable, Tx};
/// struct Canvas; impl Database for Canvas {}
/// #[derive(DbTable)] #[db(table="automatic", database=Canvas)]
/// struct Row { #[db(key, generated)] id: i64 }
/// fn upsert(mut tx: Tx<'_, Canvas>) { tx.upsert(&Row { id: 0 }).unwrap(); }
/// ```
/// ```compile_fail
/// use studio_db_api::{Database, DbTable, Tx};
/// struct Canvas; impl Database for Canvas {}
/// #[derive(DbTable)] #[db(table="automatic", database=Canvas)]
/// struct Row { #[db(key, generated)] id: i64 }
/// fn insert_new(mut tx: Tx<'_, Canvas>) { tx.insert_new(&Row { id: 0 }).unwrap(); }
/// ```
pub trait UpdateTable: MutableTable {}
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
    /// Install a descriptor without replacing an existing legacy table or its data.
    pub fn install_if_absent(self, connection: &Connection) -> rusqlite::Result<()> {
        for sql in self.ddl_statements() {
            let keyword = if sql.starts_with("CREATE TABLE ") {
                "TABLE "
            } else {
                "INDEX "
            };
            connection.execute_batch(&sql.replacen(
                keyword,
                &format!("{keyword}IF NOT EXISTS "),
                1,
            ))?;
        }
        Ok(())
    }
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
    const QUOTED_NAME: &'static str;
}
/// Generated fields cannot be assigned by a typed update.
/// ```compile_fail
/// use studio_db_api::{Database, DbTable, Tx, db_update};
/// struct Canvas; impl Database for Canvas {}
/// #[derive(DbTable)] #[db(table="typed_rows", database=Canvas)]
/// struct Row { #[db(key)] id: i64, #[db(generated)] computed: i64 }
/// fn update(mut tx: Tx<'_, Canvas>) {
///     db_update!(tx, Row, set(Row::COMPUTED = &7)).unwrap();
/// }
/// ```
pub trait WritableColumn: Column {}
#[doc(hidden)]
pub const fn column_name<R: Table, C: Column<Row = R>>(_column: C) -> &'static str {
    C::QUOTED_NAME
}
#[doc(hidden)]
pub const fn writable_column_name<R: Table, C: WritableColumn<Row = R>>(
    _column: C,
) -> &'static str {
    C::QUOTED_NAME
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
    pub check: fn(&Connection) -> rusqlite::Result<StateCheck>,
}
#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize)]
pub struct StateCheckFailure {
    pub pointer: String,
    pub keyword: String,
    pub count: u64,
}
impl std::fmt::Display for StateCheckFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.pointer, self.keyword)
    }
}
impl std::error::Error for StateCheckFailure {}
impl StateCheckFailure {
    fn from_decode_error(error: rusqlite::Error) -> Self {
        if let rusqlite::Error::FromSqlConversionFailure(_, _, source)
        | rusqlite::Error::ToSqlConversionFailure(source)
        | rusqlite::Error::UserFunctionError(source) = error
            && let Some(detail) = source.downcast_ref::<Self>()
        {
            return detail.clone();
        }
        Self {
            pointer: String::new(),
            keyword: "storageType".into(),
            count: 1,
        }
    }
}
#[derive(Debug, Default)]
pub struct StateCheck {
    pub row_count: u64,
    pub rejected_count: u64,
    pub failures: Vec<StateCheckFailure>,
}
impl StateCheck {
    pub fn counts(&self) -> (u64, u64) {
        (self.row_count, self.rejected_count)
    }
}
pub fn check<R: Table>(connection: &Connection) -> rusqlite::Result<StateCheck> {
    R::state_check(connection)
}
fn check_rows<R: Table>(connection: &Connection) -> rusqlite::Result<StateCheck> {
    let exists: bool = connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name=?1)",
        [R::NAME],
        |row| row.get(0),
    )?;
    if !exists {
        return Ok(StateCheck::default());
    }
    // SQLite count(*) is a non-negative signed integer.
    let violations = connection
        .prepare_cached(R::STATE_CHECK)?
        .query_row([], |row| row.get::<_, i64>(0))? as u64;
    let mut statement = connection.prepare_cached(R::VALID_ROWS)?;
    let mut rows = statement.query([])?;
    let (mut count, mut rejected) = (violations, violations);
    let mut failures = std::collections::BTreeMap::new();
    if violations > 0 {
        failures.insert((String::new(), "storageType".to_owned()), violations);
    }
    while let Some(row) = rows.next()? {
        count += 1;
        if let Err(error) = R::read(connection, row) {
            rejected += 1;
            let detail = StateCheckFailure::from_decode_error(error);
            *failures
                .entry((detail.pointer, detail.keyword))
                .or_default() += detail.count;
        }
    }
    Ok(StateCheck {
        row_count: count,
        rejected_count: rejected,
        failures: failures
            .into_iter()
            .map(|((pointer, keyword), count)| StateCheckFailure {
                pointer,
                keyword,
                count,
            })
            .collect(),
    })
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
