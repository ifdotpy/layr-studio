use rusqlite::{
    Connection,
    types::{FromSql, ToSql},
};

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
pub trait EnumValue: DbValue {}
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
    type Encoded<'a>: ToSql
    where
        T: 'a;
    fn encode<'a>(connection: &Connection, value: &'a T) -> rusqlite::Result<Self::Encoded<'a>>;
    fn decode(connection: &Connection, value: Self::Stored) -> rusqlite::Result<T>;
}
