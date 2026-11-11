//! Required logical fields over the nullable columns of F1 shared tables.
use crate::json_record_codec::{ExactJson, ExactJsonCodec};
use std::marker::PhantomData;
use studio_db_api::{Codec, CodecContext, CodecError, CodecRow, DbValue};

/// A valid legacy TEXT key keeps its identity across rowid changes. NULL keys
/// remain distinct by their original signed rowid, including negative rowids.
pub(crate) fn nullable_text_identity(key: &Option<String>, source_rowid: i64) -> Vec<u8> {
    studio_db_api::migration::encode_key(&(key.clone(), key.is_none().then_some(source_rowid)))
}

// Existing Codec provides a required logical value over a sparse physical column.
pub(crate) struct Required<T>(PhantomData<T>);
impl<T: DbValue> Codec<T> for Required<T> {
    type Stored = Option<T>;
    type Encoded<'a>
        = &'a T
    where
        T: 'a;
    fn encode<'a>(_: &CodecContext, value: &'a T) -> Result<&'a T, CodecError> {
        Ok(value)
    }
    fn decode(_: &CodecContext, value: Option<T>) -> Result<T, CodecError> {
        value.ok_or(CodecError::InvalidQuery)
    }
}

pub(crate) struct RequiredCodec<C, T>(PhantomData<(C, T)>);
impl<C: Codec<T>, T> Codec<T> for RequiredCodec<C, T> {
    type Stored = Option<C::Stored>;
    type Encoded<'a>
        = C::Encoded<'a>
    where
        T: 'a;
    fn encode<'a>(db: &CodecContext, value: &'a T) -> Result<Self::Encoded<'a>, CodecError> {
        C::encode(db, value)
    }
    fn decode(db: &CodecContext, value: Self::Stored) -> Result<T, CodecError> {
        C::decode(db, value.ok_or(CodecError::InvalidQuery)?)
    }
    fn decode_row(db: &CodecContext, row: &CodecRow<'_>, index: usize) -> Result<T, CodecError> {
        if matches!(
            row.get_ref(index)?,
            studio_db_api::rusqlite::types::ValueRef::Null
        ) {
            return Err(CodecError::InvalidQuery);
        }
        C::decode_row(db, row, index)
    }
}

pub(crate) type RequiredJson<T> = RequiredCodec<ExactJsonCodec<T>, ExactJson<T>>;
