//! Lossless copy plans and durable, typed SQLite key cursors.
use std::fmt::Write;

use rusqlite::{Connection, OptionalExtension, params_from_iter, types::Value};
use sha2::{Digest, Sha256};

use crate::{MutableTable, Table, Tx};

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
    let parts = columns.iter().map(|column| {
        let value = format!("{}.{}", identifier(alias), identifier(column));
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
    pub target_select: &'static str,
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
            &format!("SELECT EXISTS(SELECT 1 FROM ({}))", self.target_select),
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
        let predicate = if after.is_some() {
            // SQLite's ASC ordering puts NULL first. `IS` compares nullable
            // components without losing rows whose legacy identity has NULLs.
            let mut prefix = Vec::new();
            let mut terms = Vec::new();
            for (i, column) in self.source_keys.iter().enumerate() {
                let column = identifier(column);
                let parameter = format!("?{}", i + 1);
                let greater = format!(
                    "({column}>{parameter} OR ({parameter} IS NULL AND {column} IS NOT NULL))"
                );
                let mut term = prefix.clone();
                term.push(greater);
                terms.push(format!("({})", term.join(" AND ")));
                prefix.push(format!("{column} IS {parameter}"));
            }
            format!(" WHERE ({})", terms.join(" OR "))
        } else {
            String::new()
        };
        values.push(Value::Integer(limit.try_into().unwrap_or(i64::MAX)));
        let sql = format!(
            "{}{predicate} ORDER BY {columns} LIMIT ?{}",
            L::SELECT,
            values.len()
        );
        db.prepare_cached(&sql)?
            .query_map(params_from_iter(values), |row| {
                let row = L::read(db, row)?;
                Ok(((self.source_key)(&row), row))
            })?
            .collect()
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
            .query::<L>(L::SELECT, &[])?
            .iter()
            .map(|row| (self.source_evidence)(row).map(|e| e.digest()))
            .collect::<rusqlite::Result<Vec<_>>>()?;
        let mut actual = tx
            .query::<T>(self.target_select, &[])?
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
        for row in tx.query::<L>(L::SELECT, &[])? {
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
        "// Migration {id}\nfn plan() -> CopyPlan<{legacy}, {target}> {{\n    CopyPlan {{\n        name: \"{}\",\n        source_keys: &[],\n        source_key, target_key, transform, source_evidence, target_evidence,\n        target_select: {target}::SELECT, finish: switch_and_retire,\n    }}\n}}\n",
        id.replace('-', "_")
    )
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
    fn dated_skeleton_names_one_copy_plan_and_explicit_authority_switch() {
        let source = migration_skeleton("2026-11-09-metadata", "LegacySettings", "Metadata");
        assert!(source.contains("CopyPlan<LegacySettings, Metadata>"));
        assert!(source.contains("2026_11_09_metadata"));
        assert!(source.contains("finish: switch_and_retire"));
    }
}
