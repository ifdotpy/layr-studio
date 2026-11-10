use crate::*;
use rusqlite::{Connection, OptionalExtension, Transaction, types::ToSql};
use std::marker::PhantomData;

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
    pub fn visit<P: QueryParams, R>(
        &self,
        query: &NamedQuery<D, P, R, ReadMode, Many>,
        params: &P,
        visitor: impl FnMut(R) -> rusqlite::Result<()>,
    ) -> rusqlite::Result<()> {
        query.visit(self.connection, params, true, visitor)
    }
    pub fn collect<P: QueryParams, R>(
        &self,
        query: &NamedQuery<D, P, R, ReadMode, Many>,
        params: &P,
    ) -> rusqlite::Result<Vec<R>> {
        let mut rows = Vec::new();
        self.visit(query, params, |row| {
            rows.push(row);
            Ok(())
        })?;
        Ok(rows)
    }
    pub fn named<P: QueryParams, R, C: SingleCardinality>(
        &self,
        query: &NamedQuery<D, P, R, ReadMode, C>,
        params: &P,
    ) -> rusqlite::Result<C::Output<R>> {
        query.run(self.connection, params, true)
    }
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
    pub fn borrowed_parameter<'a, R: Table<Database = D>, C: Column<Row = R> + 'a>(
        &self,
        column: C,
        value: &'a (impl ColumnInput<C> + ?Sized),
    ) -> rusqlite::Result<impl ToSql + 'a> {
        value.encode_input(self.connection, column)
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
        sql: Statement<D>,
        params: &[&dyn ToSql],
    ) -> rusqlite::Result<Vec<R>> {
        let mut statement = self.prepare_read(sql.sql)?;
        statement
            .query_map(params, |row| R::read(self.connection, row))?
            .collect()
    }
    pub fn project<P: Projection>(
        &self,
        sql: Statement<D>,
        params: &[&dyn ToSql],
    ) -> rusqlite::Result<Vec<P>>
    where
        P::Table: Table<Database = D>,
    {
        self.prepare_read(sql.sql)?
            .query_map(params, |row| P::read(self.connection, row))?
            .collect()
    }
    pub fn project_one<P: Projection>(
        &self,
        sql: Statement<D>,
        params: &[&dyn ToSql],
    ) -> rusqlite::Result<Option<P>>
    where
        P::Table: Table<Database = D>,
    {
        self.prepare_read(sql.sql)?
            .query_row(params, |row| P::read(self.connection, row))
            .optional()
    }
    #[doc(hidden)]
    pub fn scalar<R: Table<Database = D>, C: Column<Row = R>>(
        &self,
        _: C,
        sql: Statement<D>,
        params: &[&dyn ToSql],
    ) -> rusqlite::Result<Option<C::Value>> {
        self.prepare_read(sql.sql)?
            .query_row(params, |row| C::decode(self.connection, row, 0))
            .optional()
    }
    #[doc(hidden)]
    pub fn scalars<R: Table<Database = D>, C: Column<Row = R>>(
        &self,
        _: C,
        sql: Statement<D>,
        params: &[&dyn ToSql],
    ) -> rusqlite::Result<Vec<C::Value>> {
        self.prepare_read(sql.sql)?
            .query_map(params, |row| C::decode(self.connection, row, 0))?
            .collect()
    }
    #[doc(hidden)]
    pub fn scalar_raw<R: Table<Database = D>, C: Column<Row = R>>(
        &self,
        _: C,
        sql: Statement<D>,
        params: &[&dyn ToSql],
    ) -> rusqlite::Result<Option<C::Stored>> {
        self.prepare_read(sql.sql)?
            .query_row(params, |row| C::Stored::read(row, 0))
            .optional()
    }
    #[doc(hidden)]
    pub fn exists<R: Table<Database = D>>(
        &self,
        sql: Statement<D>,
        params: &[&dyn ToSql],
    ) -> rusqlite::Result<bool> {
        self.prepare_read(sql.sql)?
            .query_row(params, |_| Ok(()))
            .optional()
            .map(|row| row.is_some())
    }
}
impl<'c, D: Database> Tx<'c, D> {
    pub fn visit<P: QueryParams, R>(
        &mut self,
        query: &NamedQuery<D, P, R, WriteMode, Many>,
        params: &P,
        visitor: impl FnMut(R) -> rusqlite::Result<()>,
    ) -> rusqlite::Result<()> {
        self.with_named_savepoint(|connection| query.visit(connection, params, false, visitor))
    }
    pub fn collect<P: QueryParams, R>(
        &mut self,
        query: &NamedQuery<D, P, R, WriteMode, Many>,
        params: &P,
    ) -> rusqlite::Result<Vec<R>> {
        let mut rows = Vec::new();
        self.visit(query, params, |row| {
            rows.push(row);
            Ok(())
        })?;
        Ok(rows)
    }
    /// A failed write decoder, visitor, cardinality check or terminal step rolls
    /// back this operation; the caller still owns the surrounding transaction.
    pub fn named<P: QueryParams, R, C: SingleCardinality>(
        &mut self,
        query: &NamedQuery<D, P, R, WriteMode, C>,
        params: &P,
    ) -> rusqlite::Result<C::Output<R>> {
        self.with_named_savepoint(|connection| query.run(connection, params, false))
    }
    fn with_named_savepoint<T>(
        &mut self,
        operation: impl FnOnce(&Connection) -> rusqlite::Result<T>,
    ) -> rusqlite::Result<T> {
        self.transaction
            .execute_batch("SAVEPOINT db_api_named_query")?;
        match operation(self.transaction) {
            Ok(value) => {
                self.transaction
                    .execute_batch("RELEASE db_api_named_query")?;
                Ok(value)
            }
            Err(error) => {
                self.transaction
                    .execute_batch("ROLLBACK TO db_api_named_query; RELEASE db_api_named_query")?;
                Err(error)
            }
        }
    }
    pub fn partition<P: PartitionTable<Database = D>>(&mut self) -> Partition<'_, 'c, D, P> {
        Partition {
            tx: self,
            row: PhantomData,
        }
    }
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
    pub fn insert<R: InsertTable<Database = D>>(&mut self, row: &R) -> rusqlite::Result<()> {
        row.validate()?;
        let mut statement = self.transaction.prepare_cached(R::INSERT)?;
        row.bind(self.transaction, |params| {
            statement.execute(params).map(|_| ())
        })
    }
    pub fn insert_rowid<R: InsertTable<Database = D>>(&mut self, row: &R) -> rusqlite::Result<i64> {
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
        sql: Statement<D>,
        params: &[&dyn ToSql],
    ) -> rusqlite::Result<usize> {
        self.transaction.prepare_cached(sql.sql)?.execute(params)
    }
    #[doc(hidden)]
    pub fn update<R: UpdateTable<Database = D>>(
        &mut self,
        sql: Statement<D>,
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
    /// Validate a complete row and replace it only when its key already exists.
    pub fn update_existing<R: MutableTable<Database = D>>(
        &mut self,
        row: &R,
    ) -> rusqlite::Result<bool> {
        row.validate()?;
        row.bind_update(self.transaction, |params| {
            self.transaction
                .prepare_cached(R::UPDATE_EXISTING)?
                .execute(params)
                .map(|n| n != 0)
        })
    }
    /// Compare the stored revision while applying the validated new row.
    pub fn update_existing_at<R: RevisionTable<Database = D>>(
        &mut self,
        row: &R,
        expected: &R::Revision,
    ) -> rusqlite::Result<bool> {
        row.validate()?;
        row.bind_update(self.transaction, |params| {
            self.transaction
                .prepare_cached(R::UPDATE_AT_REVISION)?
                .execute(rusqlite::params_from_iter(
                    params
                        .iter()
                        .copied()
                        .chain(std::iter::once(expected as &dyn ToSql)),
                ))
                .map(|n| n != 0)
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
        sql: Statement<D>,
        params: &[&dyn ToSql],
    ) -> rusqlite::Result<Vec<R>> {
        self.read().query::<R>(sql, params)
    }
}
pub struct Partition<'t, 'c, D: Database, P> {
    tx: &'t mut Tx<'c, D>,
    row: PhantomData<P>,
}
impl<D: Database, P: PartitionTable<Database = D>> Partition<'_, '_, D, P> {
    pub fn get(&self, key: &P::Key) -> rusqlite::Result<Option<P>> {
        self.tx.get::<P>(key)
    }
    pub fn list(&self) -> rusqlite::Result<Vec<P>> {
        self.tx.read().collect(&P::SELECT_QUERY, &())
    }
}
impl<D: Database, P: PartitionTable<Database = D> + MutableTable> Partition<'_, '_, D, P> {
    pub fn put(&mut self, row: &P) -> rusqlite::Result<()> {
        self.tx.upsert(row)
    }
    pub fn delete(&mut self, key: &P::Key) -> rusqlite::Result<usize> {
        self.tx.delete::<P>(key)
    }
}

/// Generated unit structs make row, value and SQL name inseparable.
pub trait Column: Copy {
    type Row: Table;
    type Value;
    type Stored: DbValue;
    type Parameter<'a>: ToSql
    where
        Self::Value: 'a;
    fn encode<'a>(
        connection: &Connection,
        value: &'a Self::Value,
    ) -> rusqlite::Result<Self::Parameter<'a>>;
    const NAME: &'static str;
    const QUOTED_NAME: &'static str;
    fn decode(
        connection: &Connection,
        row: &rusqlite::Row<'_>,
        index: usize,
    ) -> rusqlite::Result<Self::Value>;
}
/// Generated bindings admit borrowed native strings without bypassing custom codecs.
pub trait ColumnInput<C: Column> {
    type Parameter<'a>: ToSql
    where
        Self: 'a;
    fn encode_input<'a>(
        &'a self,
        connection: &Connection,
        column: C,
    ) -> rusqlite::Result<Self::Parameter<'a>>;
}
/// Only columns and decoded types from the named owner can enter a projection.
/// ```compile_fail
/// use studio_db_api::{Database, DbTable, DbProjection};
/// struct Canvas; impl Database for Canvas {}
/// #[derive(DbTable)] #[db(table="rows_table",database=Canvas)]
/// struct Row { #[db(key)] id: i64, text: String }
/// #[derive(DbProjection)] #[db(of=Row)]
/// struct Wrong { text: i64 }
/// ```
pub trait Projection: Sized {
    type Table: Table;
    const COLUMNS: &'static [ColumnDescriptor];
    const SELECT: &'static str;
    fn read(connection: &Connection, row: &rusqlite::Row<'_>) -> rusqlite::Result<Self>;
}
#[doc(hidden)]
pub fn projection_value<C: Column>(
    connection: &Connection,
    row: &rusqlite::Row<'_>,
    index: usize,
    _: C,
) -> rusqlite::Result<C::Value> {
    C::decode(connection, row, index)
}
#[doc(hidden)]
pub fn projection_raw<C: Column>(
    row: &rusqlite::Row<'_>,
    index: usize,
    _: C,
) -> rusqlite::Result<C::Stored> {
    C::Stored::read(row, index)
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
