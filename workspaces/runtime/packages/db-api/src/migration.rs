//! Lossless copy plans and durable, typed SQLite key cursors.
use std::fmt::Write;

use rusqlite::{Connection, OptionalExtension, params_from_iter, types::Value};
use sha2::{Digest, Sha256};

use crate::{MutableTable, Table, Tx};

/// Maximum stored cell bytes decoded into one copy page. One larger row still
/// advances alone; numeric cells count as eight bytes and NULL as zero.
pub const COPY_PAGE_MAX_BYTES: usize = 2 * 1024 * 1024;

/// Stored cell payload bytes in a source row, shared by paging and migration
/// measurements. This borrows cells and never decodes a typed payload.
pub fn copy_row_bytes(row: &rusqlite::Row<'_>) -> rusqlite::Result<usize> {
    let mut row_bytes = 0usize;
    for column in 0..row.as_ref().column_count() {
        let size = match row.get_ref(column)? {
            rusqlite::types::ValueRef::Null => 0,
            rusqlite::types::ValueRef::Integer(_) | rusqlite::types::ValueRef::Real(_) => 8,
            rusqlite::types::ValueRef::Text(value) | rusqlite::types::ValueRef::Blob(value) => {
                value.len()
            }
        };
        row_bytes = row_bytes.saturating_add(size);
    }
    Ok(row_bytes)
}

/// A key component keeps its SQLite storage class. Floating-point keys are
/// deliberately unsupported; nullable legacy identities must include a tie key.
pub trait KeyPart: Sized {
    const INTEGER: bool = false;
    fn value(&self) -> Value;
    fn from_value(value: Value) -> rusqlite::Result<Self>;
}
macro_rules! key_part {
    ($ty:ty, $variant:ident, $integer:literal) => {
        impl KeyPart for $ty {
            const INTEGER: bool = $integer;
            fn value(&self) -> Value {
                Value::$variant(self.clone())
            }
            fn from_value(value: Value) -> rusqlite::Result<Self> {
                match value {
                    Value::$variant(value) => Ok(value),
                    _ => Err(rusqlite::Error::InvalidQuery),
                }
            }
        }
    };
}
key_part!(i64, Integer, true);
key_part!(String, Text, false);
key_part!(Vec<u8>, Blob, false);
impl<T: KeyPart> KeyPart for Option<T> {
    fn value(&self) -> Value {
        self.as_ref().map_or(Value::Null, KeyPart::value)
    }
    fn from_value(value: Value) -> rusqlite::Result<Self> {
        if value == Value::Null {
            Ok(None)
        } else {
            T::from_value(value).map(Some)
        }
    }
}

/// A scalar or tuple cursor, bound as its original SQLite components.
pub trait MigrationKey: Sized + Send + Sync + 'static {
    const INTEGER: bool = false;
    fn values(&self) -> Vec<Value>;
    fn from_values(values: Vec<Value>) -> rusqlite::Result<Self>;
}
impl<T: KeyPart + Send + Sync + 'static> MigrationKey for T {
    const INTEGER: bool = T::INTEGER;
    fn values(&self) -> Vec<Value> {
        vec![self.value()]
    }
    fn from_values(mut values: Vec<Value>) -> rusqlite::Result<Self> {
        if values.len() != 1 {
            return Err(rusqlite::Error::InvalidQuery);
        }
        T::from_value(values.remove(0))
    }
}
macro_rules! tuple_key {
    ($($name:ident),+) => {
        impl<$($name: KeyPart + Send + Sync + 'static),+> MigrationKey for ($($name,)+) {
            #[allow(non_snake_case)]
            fn values(&self) -> Vec<Value> { let ($($name,)+) = self; vec![$($name.value()),+] }
            fn from_values(values: Vec<Value>) -> rusqlite::Result<Self> {
                let mut values = values.into_iter();
                let key = ($($name::from_value(values.next().ok_or(rusqlite::Error::InvalidQuery)?)?,)+);
                if values.next().is_some() { return Err(rusqlite::Error::InvalidQuery); }
                Ok(key)
            }
        }
    };
}
tuple_key!(A, B);
tuple_key!(A, B, C);
tuple_key!(A, B, C, D);
impl<A: KeyPart + Send + Sync + 'static> MigrationKey for (A,) {
    const INTEGER: bool = A::INTEGER;
    fn values(&self) -> Vec<Value> {
        vec![self.0.value()]
    }
    fn from_values(values: Vec<Value>) -> rusqlite::Result<Self> {
        <A as MigrationKey>::from_values(values).map(|value| (value,))
    }
}

/// The online runner reads both historical integer keys and framed composites.
/// INTEGER affinity is retained in the legacy non-STRICT dirty table; SQLite
/// stores composite BLOBs without coercion.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DirtyKey {
    Integer(i64),
    Composite(Vec<u8>),
}
impl rusqlite::types::ToSql for DirtyKey {
    fn to_sql(&self) -> rusqlite::Result<rusqlite::types::ToSqlOutput<'_>> {
        match self {
            Self::Integer(value) => value.to_sql(),
            Self::Composite(bytes) => bytes.to_sql(),
        }
    }
}
impl rusqlite::types::FromSql for DirtyKey {
    fn column_result(value: rusqlite::types::ValueRef<'_>) -> rusqlite::types::FromSqlResult<Self> {
        match value {
            rusqlite::types::ValueRef::Integer(value) => Ok(Self::Integer(value)),
            rusqlite::types::ValueRef::Blob(value) => Ok(Self::Composite(value.to_vec())),
            rusqlite::types::ValueRef::Null
            | rusqlite::types::ValueRef::Real(_)
            | rusqlite::types::ValueRef::Text(_) => Err(rusqlite::types::FromSqlError::InvalidType),
        }
    }
}
// A dirty key can itself be part of the (migration, key) identity.
impl KeyPart for DirtyKey {
    fn value(&self) -> Value {
        match self {
            Self::Integer(value) => Value::Integer(*value),
            Self::Composite(bytes) => Value::Blob(bytes.clone()),
        }
    }
    fn from_value(value: Value) -> rusqlite::Result<Self> {
        match value {
            Value::Integer(value) => Ok(Self::Integer(value)),
            Value::Blob(bytes) => Ok(Self::Composite(bytes)),
            Value::Null | Value::Real(_) | Value::Text(_) => Err(rusqlite::Error::InvalidQuery),
        }
    }
}
impl crate::DbValue for DirtyKey {
    const SQL_TYPE: &'static str = "INTEGER";
}
impl DirtyKey {
    pub fn decode<K: MigrationKey>(&self) -> rusqlite::Result<K> {
        match self {
            Self::Integer(value) => K::from_values(vec![Value::Integer(*value)]),
            Self::Composite(bytes) => decode_key(bytes),
        }
    }
}

/// Framing shared by Rust and change triggers: storage tag, sixteen hex length
/// digits (bytes), then hex bytes. INTEGER uses its exact decimal representation.
pub fn encode_key<K: MigrationKey>(key: &K) -> Vec<u8> {
    let mut encoded = String::new();
    for value in key.values() {
        let (tag, bytes) = match value {
            Value::Null => ('n', Vec::new()),
            Value::Integer(value) => ('i', value.to_string().into_bytes()),
            Value::Text(value) => ('t', value.into_bytes()),
            Value::Blob(value) => ('b', value),
            Value::Real(_) => unreachable!("MigrationKey does not support REAL components"),
        };
        write!(encoded, "{tag}{:016X}", bytes.len()).expect("String write");
        for byte in bytes {
            write!(encoded, "{byte:02X}").expect("String write");
        }
    }
    encoded.into_bytes()
}

pub fn decode_key<K: MigrationKey>(bytes: &[u8]) -> rusqlite::Result<K> {
    let invalid = || rusqlite::Error::InvalidQuery;
    let mut encoded = std::str::from_utf8(bytes).map_err(|_| invalid())?;
    if !encoded.is_ascii() {
        return Err(invalid());
    }
    let mut values = Vec::new();
    while !encoded.is_empty() {
        let header = encoded.get(..17).ok_or_else(invalid)?;
        let length = usize::from_str_radix(&header[1..], 16).map_err(|_| invalid())?;
        let end = 17usize
            .checked_add(length.checked_mul(2).ok_or_else(invalid)?)
            .ok_or_else(invalid)?;
        let hex = encoded.get(17..end).ok_or_else(invalid)?;
        let bytes = (0..hex.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).map_err(|_| invalid()))
            .collect::<rusqlite::Result<Vec<_>>>()?;
        let value = match header.as_bytes()[0] {
            b'n' if bytes.is_empty() => Value::Null,
            b'i' => Value::Integer(
                std::str::from_utf8(&bytes)
                    .map_err(|_| invalid())?
                    .parse()
                    .map_err(|_| invalid())?,
            ),
            b't' => Value::Text(String::from_utf8(bytes).map_err(|_| invalid())?),
            b'b' => Value::Blob(bytes),
            _ => return Err(invalid()),
        };
        values.push(value);
        encoded = &encoded[end..];
    }
    K::from_values(values)
}

/// Quoted identifiers only; callers declare columns, never SQL expressions.
pub fn identifier(name: &str) -> String {
    format!("\"{}\"", name.replace('"', "\"\""))
}

/// Same framing as `encode_key`, produced entirely by built-in SQLite functions
/// so every writer can execute the persistent triggers without a connection hook.
pub fn key_sql(columns: &[&str], alias: &str) -> String {
    let expressions = columns
        .iter()
        .map(|column| format!("{}.{}", identifier(alias), identifier(column)))
        .collect::<Vec<_>>();
    key_expression_sql(&expressions.iter().map(String::as_str).collect::<Vec<_>>())
}

/// Same key framing for owner-generated SQL expressions, including OLD/NEW cells.
/// Expressions are trusted schema source, never runtime input.
pub fn key_expression_sql(expressions: &[&str]) -> String {
    let parts = expressions.iter().map(|value| {
        format!("(CASE typeof({value}) WHEN 'null' THEN 'n' WHEN 'integer' THEN 'i' WHEN 'text' THEN 't' WHEN 'blob' THEN 'b' ELSE RAISE(ABORT,'unsupported migration key storage') END || printf('%016X',coalesce(length(CAST({value} AS BLOB)),0)) || hex(CAST({value} AS BLOB)))")
    }).collect::<Vec<_>>().join(" || ");
    format!("CAST({parts} AS BLOB)")
}

/// Complete identity, multiplicity, ordering and scalar/byte evidence of one row.
/// TEXT and BLOB differ even when their bytes match. JSON is never normalized.
#[derive(Clone, Debug)]
pub struct Evidence {
    pub identity: Vec<Value>,
    pub ordering: Vec<Value>,
    pub values: Vec<Value>,
}
impl Evidence {
    pub fn digest(&self) -> [u8; 32] {
        let mut hash = Sha256::new();
        for group in [&self.identity, &self.ordering, &self.values] {
            hash.update((group.len() as u64).to_le_bytes());
            for cell in group {
                let (tag, bytes) = match cell {
                    Value::Null => (0, Vec::new()),
                    Value::Integer(value) => (1, value.to_le_bytes().to_vec()),
                    Value::Real(value) => (2, value.to_bits().to_le_bytes().to_vec()),
                    Value::Text(value) => (3, value.as_bytes().to_vec()),
                    Value::Blob(value) => (4, value.clone()),
                };
                hash.update([tag]);
                hash.update((bytes.len() as u64).to_le_bytes());
                hash.update(bytes);
            }
        }
        hash.finalize().into()
    }
}

/// Compare-and-set copies can skip a changed source; every successful write is
/// read back and checked in the same transaction before it contributes evidence.
pub fn copy_verified<E>(
    write: impl FnOnce() -> Result<bool, E>,
    verify_read_back: impl FnOnce() -> Result<bool, E>,
    mismatch: impl FnOnce() -> E,
) -> Result<bool, E> {
    if !write()? {
        return Ok(false);
    }
    if !verify_read_back()? {
        return Err(mismatch());
    }
    Ok(true)
}

/// Retire a descriptor-owned migration source. SQLite removes its attached
/// indexes and triggers in the same transaction. Replace cross-table producer
/// references before this call; no arbitrary DDL is accepted.
pub fn retire_source<T: Table>(tx: &mut Tx<'_, T::Database>) -> rusqlite::Result<()> {
    tx.connection()
        .execute_batch(&format!("DROP TABLE {}", identifier(T::NAME)))
}

/// A dated migration is one plan declaration plus its pure transform/evidence
/// functions. The caller owns the state lock and transaction. `finish` switches
/// durable writer authority and retires the source, after verification.
/// `target_key` must be a pure, injective function of the complete source key.
/// Only one-to-one copies are supported; splits and merges need explicit adapters.
/// The target must start empty and have no independent writers while copying.
pub struct CopyPlan<L: Table, T: MutableTable<Database = L::Database>> {
    pub name: &'static str,
    pub source_keys: &'static [&'static str],
    pub source_key: fn(&L) -> L::Key,
    pub target_key: fn(&L::Key) -> T::Key,
    pub transform: fn(L) -> rusqlite::Result<T>,
    pub source_evidence: fn(&L) -> rusqlite::Result<Evidence>,
    pub target_evidence: fn(&T) -> rusqlite::Result<Evidence>,
    /// Select exactly this plan's target rows, including a partition predicate.
    pub target_select: crate::NamedQuery<L::Database, (), T, crate::ReadMode, crate::Many>,
    pub finish: fn(&mut Tx<'_, L::Database>) -> rusqlite::Result<()>,
}

impl<L: Table, T: MutableTable<Database = L::Database>> CopyPlan<L, T>
where
    L::Key: MigrationKey,
{
    /// A bounded existence probe; the online runner records this admission
    /// durably with its initial counters before installing dirty triggers.
    pub fn require_empty(&self, db: &Connection) -> rusqlite::Result<()> {
        let occupied: bool = db.query_row(
            &format!(
                "SELECT EXISTS(SELECT 1 FROM ({}))",
                self.target_select.sql()
            ),
            [],
            |row| row.get(0),
        )?;
        if occupied {
            return Err(rusqlite::Error::InvalidQuery);
        }
        Ok(())
    }
    pub fn read_after(
        &self,
        db: &Connection,
        after: Option<&L::Key>,
        limit: usize,
    ) -> rusqlite::Result<Vec<(L::Key, L)>> {
        let columns = self
            .source_keys
            .iter()
            .map(|name| identifier(name))
            .collect::<Vec<_>>()
            .join(",");
        let mut values = after.map(MigrationKey::values).unwrap_or_default();
        let predicate = cursor_predicate(self.source_keys, &values, after.is_some());
        values.push(Value::Integer(limit.try_into().unwrap_or(i64::MAX)));
        let sql = format!(
            "{}{predicate} ORDER BY {columns} LIMIT ?{}",
            L::SELECT,
            values.len()
        );
        let mut statement = db.prepare_cached(&sql)?;
        let mut cursor = statement.query(params_from_iter(values))?;
        let mut page = Vec::new();
        let mut bytes = 0usize;
        while let Some(row) = cursor.next()? {
            // Inspect borrowed cells before the typed decoder allocates strings,
            // blobs or nested records for another row of the page.
            let row_bytes = copy_row_bytes(row)?;
            if !page.is_empty() && bytes.saturating_add(row_bytes) > COPY_PAGE_MAX_BYTES {
                break;
            }
            let row = L::read(db, row)?;
            bytes = bytes.saturating_add(row_bytes);
            page.push(((self.source_key)(&row), row));
            if bytes >= COPY_PAGE_MAX_BYTES {
                break;
            }
        }
        Ok(page)
    }
    /// Page logical target keys without decoding payloads. The selector and key
    /// columns both come from the typed target owner.
    pub fn target_keys_after(
        &self,
        db: &Connection,
        after: Option<&T::Key>,
        limit: usize,
    ) -> rusqlite::Result<Vec<T::Key>>
    where
        T::Key: MigrationKey,
    {
        let keys = T::KEY_COLUMNS;
        if keys.is_empty()
            || keys
                .iter()
                .any(|name| !T::COLUMNS.iter().any(|column| column.name == *name))
        {
            return Err(rusqlite::Error::InvalidQuery);
        }
        let columns = keys
            .iter()
            .map(|name| identifier(name))
            .collect::<Vec<_>>()
            .join(",");
        let mut values = after.map(MigrationKey::values).unwrap_or_default();
        if after.is_some() && values.len() != keys.len() {
            return Err(rusqlite::Error::InvalidQuery);
        }
        let predicate = cursor_predicate(keys, &values, after.is_some());
        values.push(Value::Integer(limit.try_into().unwrap_or(i64::MAX)));
        let sql = format!(
            "SELECT {columns} FROM ({}){predicate} ORDER BY {columns} LIMIT ?{}",
            self.target_select.sql(),
            values.len()
        );
        let mut statement = db.prepare_cached(&sql)?;
        let mut cursor = statement.query(params_from_iter(values))?;
        let mut page = Vec::new();
        let mut bytes = 0usize;
        while let Some(row) = cursor.next()? {
            let row_bytes = copy_row_bytes(row)?;
            if !page.is_empty() && bytes.saturating_add(row_bytes) > COPY_PAGE_MAX_BYTES {
                break;
            }
            let values = (0..keys.len())
                .map(|index| row.get(index))
                .collect::<rusqlite::Result<Vec<Value>>>()?;
            page.push(T::Key::from_values(values)?);
            bytes = bytes.saturating_add(row_bytes);
            if bytes >= COPY_PAGE_MAX_BYTES {
                break;
            }
        }
        Ok(page)
    }
    pub fn read_one(&self, db: &Connection, key: &L::Key) -> rusqlite::Result<Option<L>> {
        L::bind_key(key, |params| {
            db.prepare_cached(L::GET)?
                .query_row(params, |row| L::read(db, row))
                .optional()
        })
    }
    pub fn verify(&self, tx: &Tx<'_, L::Database>) -> rusqlite::Result<()> {
        let mut expected = tx
            .read()
            .collect(&L::SELECT_QUERY, &())?
            .iter()
            .map(|row| (self.source_evidence)(row).map(|e| e.digest()))
            .collect::<rusqlite::Result<Vec<_>>>()?;
        let mut actual = tx
            .read()
            .collect(&self.target_select, &())?
            .iter()
            .map(|row| (self.target_evidence)(row).map(|e| e.digest()))
            .collect::<rusqlite::Result<Vec<_>>>()?;
        expected.sort_unstable();
        actual.sort_unstable();
        if expected != actual {
            return Err(rusqlite::Error::InvalidQuery);
        }
        Ok(())
    }
    /// Small-table path: create, copy, verify, switch and retire in the caller's
    /// transaction. An error must roll back that transaction.
    pub fn copy_tx(&self, tx: &mut Tx<'_, L::Database>) -> rusqlite::Result<()> {
        T::descriptor().install_if_absent(tx.connection())?;
        self.require_empty(tx.connection())?;
        for row in tx.read().collect(&L::SELECT_QUERY, &())? {
            let expected = (self.source_evidence)(&row)?;
            let key = (self.target_key)(&(self.source_key)(&row));
            let out = (self.transform)(row)?;
            // The outer transaction is the sole writer for both closures.
            copy_verified(
                || {
                    out.validate()?;
                    out.bind(tx.connection(), |params| {
                        tx.connection()
                            .prepare_cached(T::INSERT)?
                            .execute(params)
                            .map(|_| true)
                    })
                },
                || {
                    let row = tx
                        .get::<T>(&key)?
                        .ok_or(rusqlite::Error::QueryReturnedNoRows)?;
                    Ok((self.target_evidence)(&row)?.digest() == expected.digest())
                },
                || rusqlite::Error::InvalidQuery,
            )?;
        }
        self.verify(tx)?;
        (self.finish)(tx)
    }
}

/// A reviewable starting point for one dated migration; historical transforms
/// and the authority/retirement operation remain explicit application functions.
pub fn migration_skeleton(id: &str, legacy: &str, target: &str) -> String {
    format!(
        "// Migration {id}\nfn plan() -> CopyPlan<{legacy}, {target}> {{\n    CopyPlan {{\n        name: \"{}\",\n        source_keys: &[],\n        source_key, target_key, transform, source_evidence, target_evidence,\n        target_select: {target}::SELECT_QUERY, finish: switch_and_retire,\n    }}\n}}\n",
        id.replace('-', "_")
    )
}

fn cursor_predicate(keys: &[&str], values: &[Value], has_cursor: bool) -> String {
    let columns = keys
        .iter()
        .map(|name| identifier(name))
        .collect::<Vec<_>>()
        .join(",");
    if has_cursor && values.iter().all(|value| !matches!(value, Value::Null)) {
        // A non-NULL cursor permits an index range seek. SQLite compares
        // tuples left-to-right, so later prefixes still admit NULL suffixes.
        let parameters = (1..=values.len())
            .map(|index| format!("?{index}"))
            .collect::<Vec<_>>()
            .join(",");
        format!(" WHERE ({columns})>({parameters})")
    } else if has_cursor {
        // SQLite's ASC ordering puts NULL first. `IS` compares nullable
        // components without losing rows whose legacy identity has NULLs.
        let mut prefix = Vec::new();
        let mut terms = Vec::new();
        for (i, column) in keys.iter().enumerate() {
            let column = identifier(column);
            let parameter = format!("?{}", i + 1);
            let greater =
                format!("({column}>{parameter} OR ({parameter} IS NULL AND {column} IS NOT NULL))");
            let mut term = prefix.clone();
            term.push(greater);
            terms.push(format!("({})", term.join(" AND ")));
            prefix.push(format!("{column} IS {parameter}"));
        }
        format!(" WHERE ({})", terms.join(" OR "))
    } else {
        String::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn key_framing_preserves_types_boundaries_nulls_and_arbitrary_bytes() {
        let key = (Some("a\0:✓".to_owned()), i64::MIN, vec![0, 255, 1]);
        assert_eq!(
            decode_key::<(Option<String>, i64, Vec<u8>)>(&encode_key(&key)).unwrap(),
            key
        );
        assert_eq!(
            decode_key::<(Option<String>, String)>(&encode_key(&(None::<String>, String::new())))
                .unwrap(),
            (None, String::new())
        );
        assert_ne!(
            encode_key(&("a".to_owned(), "bc".to_owned())),
            encode_key(&("ab".to_owned(), "c".to_owned()))
        );
        assert_ne!(encode_key(&"1".to_owned()), encode_key(&1i64));
        assert_ne!(encode_key(&Vec::<u8>::new()), encode_key(&String::new()));
        for malformed in [
            b"".as_slice(),
            b"tFFFFFFFFFFFFFFFF",
            b"t0000000000000001GG",
            "✓0000000000000000".as_bytes(),
            b"n000000000000000100",
        ] {
            assert!(decode_key::<String>(malformed).is_err());
        }
    }

    #[test]
    fn durable_direct_key_expressions_preserve_insert_delete_and_rename_bytes() {
        let db = Connection::open_in_memory().unwrap();
        db.execute_batch("CREATE TABLE source(id TEXT PRIMARY KEY, record TEXT); CREATE TABLE compared(old_key BLOB,new_key BLOB)").unwrap();
        let old = key_sql(&["label", "id"], "source_key");
        let row = |alias: &str| {
            let id = format!("{alias}.id");
            let new = key_expression_sql(&["'runtime_tasks'", &id]);
            format!(
                "INSERT INTO compared SELECT {old},{new} FROM (SELECT 'runtime_tasks' AS label,{id} AS id) AS source_key;"
            )
        };
        db.execute_batch(&format!("CREATE TRIGGER source_insert AFTER INSERT ON source BEGIN {} END; CREATE TRIGGER source_update AFTER UPDATE ON source BEGIN {} {} END; CREATE TRIGGER source_delete AFTER DELETE ON source BEGIN {} END;", row("NEW"), row("OLD"), row("NEW"), row("OLD"))).unwrap();
        let before = "old:\0✓";
        let after = "renamed:'✓";
        let record = "{\"opaque\": [ 1, null ], \"future\":true}";
        db.execute("INSERT INTO source VALUES(?1,?2)", (before, record))
            .unwrap();
        db.execute("UPDATE source SET id=?1 WHERE id=?2", (after, before))
            .unwrap();
        assert_eq!(
            db.query_row("SELECT record FROM source", [], |r| r.get::<_, String>(0))
                .unwrap(),
            record
        );
        db.execute("DELETE FROM source WHERE id=?1", [after])
            .unwrap();
        let rows = db
            .prepare("SELECT old_key,new_key FROM compared ORDER BY rowid")
            .unwrap()
            .query_map([], |r| {
                Ok((r.get::<_, Vec<u8>>(0)?, r.get::<_, Vec<u8>>(1)?))
            })
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        assert_eq!(rows.len(), 4);
        for ((old, new), id) in rows.into_iter().zip([before, before, after, after]) {
            assert_eq!(old, new);
            assert_eq!(
                new,
                encode_key(&("runtime_tasks".to_owned(), id.to_owned()))
            );
        }
    }

    #[test]
    fn sql_triggers_and_rust_share_exact_key_framing() {
        let db = Connection::open_in_memory().unwrap();
        db.execute_batch("CREATE TABLE source(a,b,c); CREATE TABLE changed(k BLOB NOT NULL)")
            .unwrap();
        db.execute_batch(&format!("CREATE TRIGGER tracked AFTER INSERT ON source BEGIN INSERT INTO changed VALUES({}); END",key_sql(&["a","b","c"],"NEW"))).unwrap();
        for key in [
            (Some("a\0✓".to_owned()), i64::MIN, vec![0, 255]),
            (None, 0, vec![]),
            (Some(String::new()), -1, vec![1]),
        ] {
            db.execute(
                "INSERT INTO source VALUES(?1,?2,?3)",
                rusqlite::params![key.0, key.1, key.2],
            )
            .unwrap();
            let encoded: Vec<u8> = db
                .query_row(
                    "SELECT k FROM changed ORDER BY rowid DESC LIMIT 1",
                    [],
                    |row| row.get(0),
                )
                .unwrap();
            assert_eq!(encoded, encode_key(&key));
            assert_eq!(
                decode_key::<(Option<String>, i64, Vec<u8>)>(&encoded).unwrap(),
                key
            );
        }
        assert!(
            db.execute("INSERT INTO source VALUES(0.5,1,x'')", [])
                .is_err()
        );
        assert_eq!(
            db.query_row("SELECT count(*) FROM source", [], |row| row
                .get::<_, i64>(0))
                .unwrap(),
            3
        );
    }

    #[test]
    fn f2_nullable_target_and_old_new_cursor_frames_match_sql() {
        type TargetKey = (Option<String>, Option<i64>, Option<i64>);
        type Cursor = (Option<String>, i64);
        let db = Connection::open_in_memory().unwrap();
        db.execute_batch("CREATE TABLE target(id,component,tie); CREATE TABLE source(id); CREATE TABLE changed(k BLOB NOT NULL)").unwrap();
        db.execute_batch(&format!("CREATE TRIGGER target_insert AFTER INSERT ON target BEGIN INSERT INTO changed VALUES({}); END", key_sql(&["id", "component", "tie"], "NEW"))).unwrap();
        for (key, expected) in [
            (
                (Some("a".into()), None, None),
                "t000000000000000161n0000000000000000n0000000000000000",
            ),
            (
                (Some("".into()), None, None),
                "t0000000000000000n0000000000000000n0000000000000000",
            ),
            (
                (None, Some(0), Some(-1)),
                "n0000000000000000i000000000000000130i00000000000000022D31",
            ),
            (
                (None, Some(0), Some(-2)),
                "n0000000000000000i000000000000000130i00000000000000022D32",
            ),
            (
                (None, Some(1), Some(-1)),
                "n0000000000000000i000000000000000131i00000000000000022D31",
            ),
        ] {
            let key: TargetKey = key;
            db.execute(
                "INSERT INTO target VALUES(?1,?2,?3)",
                rusqlite::params![key.0, key.1, key.2],
            )
            .unwrap();
            let actual: Vec<u8> = db
                .query_row(
                    "SELECT k FROM changed ORDER BY rowid DESC LIMIT 1",
                    [],
                    |row| row.get(0),
                )
                .unwrap();
            assert_eq!(actual, expected.as_bytes());
            assert_eq!(actual, encode_key(&key));
            assert_eq!(decode_key::<TargetKey>(&actual).unwrap(), key);
        }
        db.execute_batch("DELETE FROM changed; INSERT INTO source(rowid,id) VALUES(-1,NULL)")
            .unwrap();
        db.execute_batch(&format!("CREATE TRIGGER source_update AFTER UPDATE ON source BEGIN INSERT INTO changed VALUES({}); INSERT INTO changed VALUES({}); END", key_sql(&["id","rowid"], "OLD"), key_sql(&["id","rowid"], "NEW"))).unwrap();
        for (old, new) in [
            ((None, -1), (Some("a".into()), -1)),
            ((Some("a".into()), -1), (Some("b".into()), -1)),
            ((Some("b".into()), -1), (None, -2)),
        ] {
            let (old, new): (Cursor, Cursor) = (old, new);
            db.execute("DELETE FROM changed", []).unwrap();
            db.execute(
                "UPDATE source SET id=?1,rowid=?2",
                rusqlite::params![new.0, new.1],
            )
            .unwrap();
            let frames = db
                .prepare("SELECT k FROM changed ORDER BY rowid")
                .unwrap()
                .query_map([], |row| row.get::<_, Vec<u8>>(0))
                .unwrap()
                .collect::<rusqlite::Result<Vec<_>>>()
                .unwrap();
            assert_eq!(frames, vec![encode_key(&old), encode_key(&new)]);
            assert_eq!(decode_key::<Cursor>(&frames[0]).unwrap(), old);
            assert_eq!(decode_key::<Cursor>(&frames[1]).unwrap(), new);
        }
    }

    #[test]
    fn evidence_frames_storage_class_length_ordering_and_multiplicity() {
        let make = |values| Evidence {
            identity: vec![Value::Integer(1)],
            ordering: vec![Value::Integer(-3)],
            values,
        };
        assert_ne!(
            make(vec![Value::Text("x".into())]).digest(),
            make(vec![Value::Blob(b"x".to_vec())]).digest()
        );
        assert_ne!(
            make(vec![Value::Text("a".into()), Value::Text("bc".into())]).digest(),
            make(vec![Value::Text("ab".into()), Value::Text("c".into())]).digest()
        );
        assert_ne!(
            make(vec![Value::Real(-0.0)]).digest(),
            make(vec![Value::Real(0.0)]).digest()
        );
        assert_ne!(
            make(vec![Value::Text("{\"a\":1}".into())]).digest(),
            make(vec![Value::Text("{ \"a\":1.0 }".into())]).digest()
        );
        let one = make(vec![Value::Null]).digest();
        assert_ne!(vec![one], vec![one, one]);
        let mut moved = make(vec![Value::Null]);
        moved.ordering = vec![Value::Integer(0)];
        assert_ne!(one, moved.digest());
    }

    #[test]
    fn f1_dirty_key_component_preserves_integer_blob_and_nested_cursor() {
        let inner = encode_key(&("x".to_owned(),));
        for key in [
            DirtyKey::Integer(-7),
            DirtyKey::Integer(7),
            DirtyKey::Composite(vec![]),
            DirtyKey::Composite(vec![0x37]),
            DirtyKey::Composite(inner),
        ] {
            let original = ("m".to_owned(), key);
            let bytes = encode_key(&original);
            assert_eq!(decode_key::<(String, DirtyKey)>(&bytes).unwrap(), original);
        }
        assert_eq!(
            encode_key(&("m".to_owned(), DirtyKey::Integer(7))),
            b"t00000000000000016Di000000000000000137"
        );
        assert_eq!(
            encode_key(&("m".to_owned(), DirtyKey::Composite(vec![0x37]))),
            b"t00000000000000016Db000000000000000137"
        );
    }

    #[test]
    fn f1_dirty_key_trigger_and_adapter_component_bytes_match() {
        let db = Connection::open_in_memory().unwrap();
        db.execute_batch("CREATE TABLE source(migration TEXT NOT NULL,key); CREATE TABLE changed(k BLOB NOT NULL)").unwrap();
        db.execute_batch(&format!("CREATE TRIGGER tracked AFTER INSERT ON source BEGIN INSERT INTO changed VALUES({}); END",key_sql(&["migration","key"],"NEW"))).unwrap();
        for key in [
            DirtyKey::Integer(i64::MIN),
            DirtyKey::Integer(0),
            DirtyKey::Integer(i64::MAX),
            DirtyKey::Composite(vec![]),
            DirtyKey::Composite(vec![0, 0xFF]),
            DirtyKey::Composite(encode_key(&("x".to_owned(),))),
        ] {
            db.execute(
                "INSERT INTO source(migration,key) VALUES(?1,?2)",
                rusqlite::params!["m", &key],
            )
            .unwrap();
            let stored: Vec<u8> = db
                .query_row(
                    "SELECT k FROM changed ORDER BY rowid DESC LIMIT 1",
                    [],
                    |row| row.get(0),
                )
                .unwrap();
            assert_eq!(stored, encode_key(&("m".to_owned(), key)));
        }
        for value in [Value::Null, Value::Real(7.0), Value::Text("7".into())] {
            assert!(<DirtyKey as KeyPart>::from_value(value).is_err());
        }
    }

    #[test]
    fn dated_skeleton_names_one_copy_plan_and_explicit_authority_switch() {
        let source = migration_skeleton("2026-11-09-metadata", "LegacySettings", "Metadata");
        assert!(source.contains("CopyPlan<LegacySettings, Metadata>"));
        assert!(source.contains("2026_11_09_metadata"));
        assert!(source.contains("finish: switch_and_retire"));
    }
}
