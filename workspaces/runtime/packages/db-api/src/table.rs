use crate::*;
use rusqlite::{Connection, types::ToSql};

pub trait Table: Sized + 'static {
    type Database: Database;
    type Key;
    const SELECT_QUERY: NamedQuery<Self::Database, (), Self, ReadMode, Many> =
        NamedQuery::all_rows();
    const NAME: &'static str;
    const DDL: &'static str;
    const INDEXES: &'static [IndexDescriptor];
    const SELECT: &'static str;
    const PARTITION: &'static str = "";
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
/// Ordinary and per-kind rows can be inserted; physical partition owners cannot.
/// ```compile_fail
/// use studio_db_api::{Database, DbTable, DbEnum, Tx};
/// struct Canvas; impl Database for Canvas {}
/// #[derive(DbEnum)] enum Kind { A = 1, B = 2 }
/// #[derive(DbTable)] #[db(table="entities",database=Canvas,partitioned_by=Kind)]
/// struct Entities { #[db(key)] kind: Kind, #[db(key)] id: i64, payload: String }
/// fn mismatch(tx: &mut Tx<'_, Canvas>, row: &Entities) { tx.insert(row).unwrap(); }
/// ```
pub trait InsertTable: Table {}
/// Receipt owners deliberately do not implement this trait.
/// Receipt rows have no generic overwriting upsert.
/// ```compile_fail
/// use studio_db_api::{Database, DbTable, Tx};
/// struct Canvas; impl Database for Canvas {}
/// #[derive(DbTable)] #[db(table="receipts",database=Canvas,receipt)]
/// struct Receipt { #[db(key)] id: i64, reply: String }
/// fn overwrite(tx: &mut Tx<'_, Canvas>, row: &Receipt) { tx.upsert(row).unwrap(); }
/// ```
pub trait MutableTable: InsertTable {
    const UPSERT: &'static str;
    const INSERT_NEW: &'static str;
    const UPDATE_EXISTING: &'static str;
    fn bind_update<T>(
        &self,
        connection: &Connection,
        f: impl FnOnce(&[&dyn ToSql]) -> rusqlite::Result<T>,
    ) -> rusqlite::Result<T>;
}
pub trait RevisionTable: MutableTable {
    type Revision: DbValue;
    const UPDATE_AT_REVISION: &'static str;
}
/// A physical table owns the permanent enum and includes its kind in the key.
/// ```compile_fail
/// use studio_db_api::{Database, DbTable, DbEnum, DbPartition};
/// struct Canvas; impl Database for Canvas {}
/// #[derive(DbEnum)] enum Kind { A = 1 }
/// #[derive(DbTable)] #[db(table="entities",database=Canvas,partitioned_by=Kind)]
/// struct Entities { #[db(key)] kind: Kind, #[db(key)] id: i64, payload: String }
/// #[derive(DbPartition)] #[db(of=Entities,kind=Kind::A)]
/// struct WrongKey { #[db(key)] id: String, payload: String }
/// ```
/// ```compile_fail
/// use studio_db_api::{Database, DbTable, DbEnum, DbPartition};
/// struct Canvas; impl Database for Canvas {}
/// #[derive(DbEnum)] enum Kind { A = 1 }
/// #[derive(DbTable)] #[db(table="entities",database=Canvas,partitioned_by=Kind)]
/// struct Entities { #[db(key)] kind: Kind, #[db(key)] id: i64, payload: String }
/// #[derive(DbPartition)] #[db(of=Entities,kind=Kind::A)]
/// struct WrongStorage { #[db(key)] id: i64, payload: i64 }
/// ```
/// Partition row fields cannot expose the physical discriminator, even with an alias.
/// ```compile_fail
/// use studio_db_api::{Database, DbEnum, DbTable, DbPartition};
/// struct Canvas; impl Database for Canvas {}
/// #[derive(DbEnum)] enum Kind { Plan = 4, Task = 9 }
/// #[derive(DbTable)] #[db(table="entities",database=Canvas,partitioned_by=Kind)]
/// struct Physical { #[db(key)] kind: Kind, #[db(key)] id: i64, payload: i64 }
/// #[derive(DbPartition)] #[db(of=Physical,kind=Kind::Plan)]
/// struct Plan { #[db(key)] id: i64, kind: Kind, payload: i64 }
/// ```
/// ```compile_fail
/// use studio_db_api::{Database, DbEnum, DbTable, DbPartition};
/// struct Canvas; impl Database for Canvas {}
/// #[derive(DbEnum)] enum Kind { Plan = 4, Task = 9 }
/// #[derive(DbTable)] #[db(table="entities",database=Canvas,partitioned_by=Kind)]
/// struct Physical { #[db(key)] kind: Kind, #[db(key)] id: i64, shadow: Kind }
/// #[derive(DbPartition)] #[db(of=Physical,kind=Kind::Plan)]
/// struct Plan { #[db(key)] id: i64, #[db(column="kind")] shadow: Kind }
/// ```
/// Raw legacy indexes cannot bypass a partition's kind predicate.
/// ```compile_fail
/// use studio_db_api::{Database, DbEnum, DbTable, DbPartition};
/// struct Canvas; impl Database for Canvas {}
/// #[derive(DbEnum)] enum Kind { Plan = 4, Task = 9 }
/// #[derive(DbTable)] #[db(table="entities",database=Canvas,partitioned_by=Kind)]
/// struct Physical { #[db(key)] kind: Kind, #[db(key)] id: i64, payload: i64 }
/// #[derive(DbPartition)] #[db(of=Physical,kind=Kind::Plan,
///     legacy_index="CREATE UNIQUE INDEX unsafe_index ON entities(payload)")]
/// struct Plan { #[db(key)] id: i64, payload: i64 }
/// ```
pub trait PartitionedTable: Table {
    type Kind: EnumValue;
    type PartitionKey;
    const KIND_COLUMN: &'static str;
}
pub trait PartitionTable: Table {
    type Physical: PartitionedTable<Database = Self::Database>;
    const LIST: &'static str;
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

/// Compare declared SQL identifiers while checking generated partition contracts.
#[doc(hidden)]
pub const fn identifiers_equal(left: &str, right: &str) -> bool {
    let left = left.as_bytes();
    let right = right.as_bytes();
    if left.len() != right.len() {
        return false;
    }
    let mut i = 0;
    while i < left.len() {
        if left[i] != right[i] {
            return false;
        }
        i += 1;
    }
    true
}
