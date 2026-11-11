//! Fixed descriptor-owned statements. Only generated declarations construct tokens.
use crate::*;
use rusqlite::hooks::{AuthAction, AuthContext, Authorization};
use rusqlite::{Connection, Statement as SqlStatement, types::ToSql};
use std::{
    marker::PhantomData,
    sync::{Arc, Mutex},
};

/// A fixed statement emitted by the query macros; it has no safe SQL constructor.
pub struct Statement<D: Database> {
    pub(crate) sql: &'static str,
    database: PhantomData<D>,
}
impl<D: Database> Copy for Statement<D> {}
impl<D: Database> Clone for Statement<D> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<D: Database> Statement<D> {
    const fn internal(sql: &'static str) -> Self {
        Self {
            sql,
            database: PhantomData,
        }
    }
    /// Generated-code boundary
    /// Only generated declarations may call this. SQL must be fixed, descriptor-bound,
    /// and scoped to D. Application callers must never supply SQL here.
    #[doc(hidden)]
    pub const fn __generated_declare(sql: &'static str) -> Self {
        Self {
            sql,
            database: PhantomData,
        }
    }
}
/// The generated argument owner binds each field with its declared column codec.
#[doc(hidden)]
pub trait QueryParams {
    const COUNT: usize;
    fn bind<T>(
        &self,
        connection: &Connection,
        f: impl FnOnce(&[&dyn ToSql]) -> rusqlite::Result<T>,
    ) -> rusqlite::Result<T>;
}
mod sealed {
    pub trait Sealed {}
}
pub struct ReadMode;
pub struct WriteMode;
pub struct One;
pub struct Optional;
pub struct Many;
pub struct NoRows;
impl sealed::Sealed for One {}
impl sealed::Sealed for Optional {}
impl sealed::Sealed for Many {}
impl sealed::Sealed for NoRows {}
impl QueryParams for () {
    const COUNT: usize = 0;
    fn bind<T>(
        &self,
        _: &Connection,
        f: impl FnOnce(&[&dyn ToSql]) -> rusqlite::Result<T>,
    ) -> rusqlite::Result<T> {
        f(&[])
    }
}
/// The non-streaming cardinalities admitted by named().
pub trait SingleCardinality: Cardinality {}
impl SingleCardinality for One {}
impl SingleCardinality for Optional {}
impl SingleCardinality for NoRows {}
/// Cardinality is independent of read/write mode. Every execution drains the statement.
pub trait Cardinality: sealed::Sealed {
    type Output<R>;
    fn collect<R>(
        statement: &mut SqlStatement<'_>,
        params: &[&dyn ToSql],
        decode: impl Fn(&rusqlite::Row<'_>) -> rusqlite::Result<R>,
    ) -> rusqlite::Result<Self::Output<R>>;
}
fn single<R>(
    statement: &mut SqlStatement<'_>,
    params: &[&dyn ToSql],
    decode: impl Fn(&rusqlite::Row<'_>) -> rusqlite::Result<R>,
) -> rusqlite::Result<Option<R>> {
    let mut rows = statement.query(params)?;
    let first = rows.next()?.map(decode).transpose()?;
    if rows.next()?.is_some() {
        return Err(rusqlite::Error::InvalidQuery);
    }
    Ok(first)
}
impl Cardinality for One {
    type Output<R> = R;
    fn collect<R>(
        s: &mut SqlStatement<'_>,
        p: &[&dyn ToSql],
        d: impl Fn(&rusqlite::Row<'_>) -> rusqlite::Result<R>,
    ) -> rusqlite::Result<R> {
        single(s, p, d)?.ok_or(rusqlite::Error::QueryReturnedNoRows)
    }
}
impl Cardinality for Optional {
    type Output<R> = Option<R>;
    fn collect<R>(
        s: &mut SqlStatement<'_>,
        p: &[&dyn ToSql],
        d: impl Fn(&rusqlite::Row<'_>) -> rusqlite::Result<R>,
    ) -> rusqlite::Result<Option<R>> {
        single(s, p, d)
    }
}
impl Cardinality for Many {
    type Output<R> = ();
    fn collect<R>(
        s: &mut SqlStatement<'_>,
        p: &[&dyn ToSql],
        d: impl Fn(&rusqlite::Row<'_>) -> rusqlite::Result<R>,
    ) -> rusqlite::Result<()> {
        for row in s.query_map(p, d)? {
            row?;
        }
        Ok(())
    }
}
impl Cardinality for NoRows {
    type Output<R> = usize;
    fn collect<R>(
        s: &mut SqlStatement<'_>,
        p: &[&dyn ToSql],
        _: impl Fn(&rusqlite::Row<'_>) -> rusqlite::Result<R>,
    ) -> rusqlite::Result<usize> {
        s.execute(p)
    }
}
#[derive(Debug)]
pub enum QueryValidationError {
    NonMainSchema { schema: String },
    Sqlite(rusqlite::Error),
}
impl From<rusqlite::Error> for QueryValidationError {
    fn from(error: rusqlite::Error) -> Self {
        Self::Sqlite(error)
    }
}
impl std::fmt::Display for QueryValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NonMainSchema { schema } => write!(
                f,
                "Named query validation requires only the main database; found schema {schema}"
            ),
            Self::Sqlite(error) => write!(f, "{error}"),
        }
    }
}
impl std::error::Error for QueryValidationError {}
/// A sealed token associates a fixed statement with its database, arguments,
/// decoder, mode and cardinality. Its fields cannot be fabricated by callers.
pub struct NamedQuery<D: Database, P, R, M, C> {
    statement: Statement<D>,
    decode: fn(&Connection, &rusqlite::Row<'_>) -> rusqlite::Result<R>,
    columns: &'static [ColumnDescriptor],
    tables: &'static [&'static str],
    marker: PhantomData<(P, M, C)>,
}
impl<D: Database, P: QueryParams, R, M, C: Cardinality> NamedQuery<D, P, R, M, C> {
    /// Generated-code boundary
    /// Generated DbQuery declarations only: all identifiers and codecs must be
    /// descriptor-owned, every reference must belong to D, and mode/cardinality
    /// must describe the fixed single statement. Validate against each schema.
    #[doc(hidden)]
    pub const fn __generated_declare(
        sql: &'static str,
        decode: fn(&Connection, &rusqlite::Row<'_>) -> rusqlite::Result<R>,
        columns: &'static [ColumnDescriptor],
        tables: &'static [&'static str],
    ) -> Self {
        Self {
            statement: Statement::__generated_declare(sql),
            decode,
            columns,
            tables,
            marker: PhantomData,
        }
    }
    /// Fixed SQL for schema diagnostics and query-plan inspection.
    pub fn sql(&self) -> &'static str {
        self.statement.sql
    }

    /// Prepare without executing. Fixtures must additionally exercise expression
    /// storage classes, NULLs and codecs; preparation cannot prove expression types.
    pub fn validate(
        &self,
        connection: &Connection,
        read_only: bool,
    ) -> Result<(), QueryValidationError> {
        let mut non_main = None;
        connection.pragma_query(None, "database_list", |row| {
            let schema: String = row.get(1)?;
            if schema != "main" {
                non_main = Some(schema);
            }
            Ok(())
        })?;
        if let Some(schema) = non_main {
            return Err(QueryValidationError::NonMainSchema { schema });
        }
        // Main-only validation is required: SQLite omits the database on empty-column reads.
        // Preserve every reported access for the final ownership assertion; deny
        // forbidden operations during preparation, before SQLite can apply pragmas.
        let accesses = Arc::new(Mutex::new(Vec::new()));
        let recorded = accesses.clone();
        let tables = self.tables;
        connection.authorizer(Some(move |context: AuthContext<'_>| {
            #[expect(
                clippy::wildcard_enum_match_arm,
                reason = "authorizer denies every action not explicitly allowed"
            )]
            let allowed = match context.action {
                AuthAction::Read {
                    table_name,
                    column_name,
                } => {
                    (context.database_name == Some("main")
                        || (context.database_name.is_none() && column_name.is_empty()))
                        && tables.contains(&table_name)
                        && !table_name.to_ascii_lowercase().starts_with("pragma_")
                }
                AuthAction::Insert { table_name }
                | AuthAction::Update {
                    table_name,
                    column_name: _,
                }
                | AuthAction::Delete { table_name } => {
                    context.database_name == Some("main")
                        && tables.contains(&table_name)
                        && !table_name.to_ascii_lowercase().starts_with("pragma_")
                }
                AuthAction::Function { function_name } => {
                    !function_name.to_ascii_lowercase().starts_with("pragma_")
                }
                AuthAction::Select | AuthAction::Recursive => true,
                _ => false,
            };
            let Ok(mut log) = recorded.lock() else {
                return Authorization::Deny;
            };
            log.push((format!("{context:?}"), allowed));
            if allowed {
                Authorization::Allow
            } else {
                Authorization::Deny
            }
        }))?;
        let prepared = connection.prepare(self.statement.sql);
        connection.authorizer(None::<fn(AuthContext<'_>) -> Authorization>)?;
        if let Some((access, _)) = accesses
            .lock()
            .map_err(|_| rusqlite::Error::InvalidQuery)?
            .iter()
            .find(|(_, allowed)| !*allowed)
        {
            return Err(rusqlite::Error::SqliteFailure(
                rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_AUTH),
                Some(format!("Undeclared query access: {access}")),
            )
            .into());
        }
        let statement = prepared?;
        if statement.readonly() != read_only
            || statement.column_count() != self.columns.len()
            || statement.parameter_count() != P::COUNT
        {
            return Err(rusqlite::Error::InvalidQuery.into());
        }
        for (i, column) in self.columns.iter().enumerate() {
            if statement.column_name(i)? != column.name {
                return Err(rusqlite::Error::InvalidQuery.into());
            }
        }
        for i in 1..=P::COUNT {
            if statement.parameter_name(i) != Some(format!("?{i}").as_str()) {
                return Err(rusqlite::Error::InvalidQuery.into());
            }
        }
        Ok(())
    }
    pub(crate) fn run(
        &self,
        connection: &Connection,
        params: &P,
        read_only: bool,
    ) -> rusqlite::Result<C::Output<R>> {
        let mut statement = connection.prepare_cached(self.statement.sql)?;
        if read_only && !statement.readonly() {
            return Err(rusqlite::Error::InvalidQuery);
        }
        params.bind(connection, |params| {
            C::collect(&mut statement, params, |row| (self.decode)(connection, row))
        })
    }
}
impl<D: Database, T: Table<Database = D>> NamedQuery<D, (), T, ReadMode, Many> {
    pub const fn all_rows() -> Self {
        Self {
            statement: Statement::internal(T::SELECT),
            decode: T::read,
            columns: T::COLUMNS,
            tables: &[T::NAME],
            marker: PhantomData,
        }
    }
}
impl<D: Database, P: QueryParams, R, M> NamedQuery<D, P, R, M, Many> {
    pub(crate) fn visit(
        &self,
        connection: &Connection,
        params: &P,
        read_only: bool,
        mut visitor: impl FnMut(R) -> rusqlite::Result<()>,
    ) -> rusqlite::Result<()> {
        let mut statement = connection.prepare_cached(self.statement.sql)?;
        if read_only && !statement.readonly() {
            return Err(rusqlite::Error::InvalidQuery);
        }
        params.bind(connection, |params| {
            let mut rows = statement.query(params)?;
            while let Some(row) = rows.next()? {
                visitor((self.decode)(connection, row)?)?;
            }
            Ok(())
        })
    }
}
#[doc(hidden)]
pub const fn query_table<D: Database, T: Table<Database = D>>() -> &'static str {
    assert!(
        T::PARTITION.is_empty(),
        "named queries require a physical descriptor; partition queries are not supported"
    );
    T::NAME
}
#[doc(hidden)]
pub const fn query_column<D: Database, C: Column>(_: C) -> &'static str
where
    C::Row: Table<Database = D>,
{
    assert!(
        <C::Row as Table>::PARTITION.is_empty(),
        "named queries do not admit partition descriptors"
    );
    C::QUOTED_NAME
}
#[doc(hidden)]
pub const fn query_column_descriptor<C: Column>(_: C) -> ColumnDescriptor {
    ColumnDescriptor {
        name: C::NAME,
        storage: C::Stored::SQL_TYPE,
        nullable: C::Stored::NULLABLE,
        private: false,
        codes: C::Stored::CODE_NAMES,
    }
}

/// SQLite hidden row identity, owned by the table marker. WITHOUT ROWID tables
/// reject it when prepared; application descriptors use it only for legacy ordering.
pub struct Rowid<T>(PhantomData<T>);
impl<T> Copy for Rowid<T> {}
impl<T> Clone for Rowid<T> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<T: Table> Rowid<T> {
    pub const COLUMN: Self = Self(PhantomData);
}
impl<T: Table> crate::ColumnInput<Rowid<T>> for i64 {
    type Parameter<'a> = &'a i64;
    fn encode_input<'a>(
        &'a self,
        connection: &Connection,
        column: Rowid<T>,
    ) -> rusqlite::Result<Self::Parameter<'a>> {
        let _ = column;
        Rowid::<T>::encode(connection, self)
    }
}
impl<T: Table> Column for Rowid<T> {
    type Row = T;
    type Value = i64;
    type Stored = i64;
    type Parameter<'a> = &'a i64;
    const NAME: &'static str = "rowid";
    const QUOTED_NAME: &'static str = "\"rowid\"";
    fn encode<'a>(_: &Connection, value: &'a i64) -> rusqlite::Result<&'a i64> {
        Ok(value)
    }
    fn decode(_: &Connection, row: &rusqlite::Row<'_>, index: usize) -> rusqlite::Result<i64> {
        i64::read(row, index)
    }
}

/// Token boundaries are checked by the Rust compiler.
/// ```compile_fail
/// use studio_db_api::{Database, Read, Statement, DbTable};
/// struct D; impl Database for D {}
/// #[derive(DbTable)] #[db(table="rows_table",database=D)] struct Row { #[db(key)] id:i64 }
/// fn raw(read: Read<'_,D>) { read.query::<Row>("SELECT id FROM rows_table", &[]); }
/// ```
/// ```compile_fail
/// use studio_db_api::{Database, NamedQuery, QueryParams, Read, ReadMode, One};
/// struct A; impl Database for A {} struct B; impl Database for B {}
/// fn wrong<P:QueryParams>(read: Read<'_,A>, q: &NamedQuery<B,P,i64,ReadMode,One>, p:&P) { read.named(q,p); }
/// ```
/// ```compile_fail
/// use studio_db_api::{Database, NamedQuery, QueryParams, Read, WriteMode, One};
/// struct D; impl Database for D {}
/// fn write<P:QueryParams>(read: Read<'_,D>, q: &NamedQuery<D,P,i64,WriteMode,One>, p:&P) { read.named(q,p); }
/// ```
/// ```compile_fail
/// use studio_db_api::{Database, NamedQuery, QueryParams, Tx, WriteMode, One};
/// struct D; impl Database for D {}
/// fn write<P:QueryParams>(tx: &Tx<'_,D>, q: &NamedQuery<D,P,i64,WriteMode,One>, p:&P) { tx.named(q,p); }
/// ```
/// ```compile_fail
/// use studio_db_api::{Database, Statement};
/// struct D; impl Database for D {}
/// fn fabricate() { let _: Statement<D> = Statement { sql: "DELETE FROM rows_table", database: std::marker::PhantomData }; }
/// ```
/// ```compile_fail
/// use studio_db_api::{Database, DbQuery, DbTable};
/// struct D; impl Database for D {}
/// #[derive(DbTable)] #[db(table="rows_table",database=D)] struct Row { #[db(key)] id:i64 }
/// #[derive(DbQuery)] #[db_query(database=D,mode=read,cardinality=one,result=String,decode=Row::ID,
/// tables(rows=Row),columns(id=Row::ID),sql="SELECT {id} AS {id} FROM {rows}")]
/// struct WrongResult;
/// ```
/// ```compile_fail
/// use studio_db_api::{Database, DbQuery, DbTable};
/// struct D; impl Database for D {}
/// #[derive(DbTable)] #[db(table="rows_table",database=D)] struct Row { #[db(key)] id:i64 }
/// #[derive(DbQuery)] #[db_query(database=D,mode=read,cardinality=one,result=i64,decode=Row::ID,
/// tables(rows=Row),columns(id=Row::ID),sql="SELECT {id} AS {id} FROM {rows} WHERE {id}=?1")]
/// struct WrongParam { #[db(column=Row::ID)] id:String }
/// ```
#[doc(hidden)]
pub struct QueryBoundaryWitness;
