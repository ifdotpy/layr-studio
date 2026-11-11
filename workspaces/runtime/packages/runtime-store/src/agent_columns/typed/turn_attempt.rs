//! One exact attempt payload with a typed normal branch and named legacy evidence.
use studio_db_api::{Codec, CodecContext, CodecError};
use studio_runtime_schema::raw_json::RawJson;
use studio_runtime_schema::start_attempt::StartAttemptView;

/// Accepted source attempts can be broader than the current normal request schema.
/// This owner preserves those exact tokens; business selectors are separate typed columns.
#[derive(Clone, Debug, PartialEq)]
pub struct LegacyAttemptEvidence {
    raw: crate::json_record_codec::ExactJson<RawJson>,
}

impl LegacyAttemptEvidence {
    pub fn raw(&self) -> &str {
        self.raw.raw()
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum StoredTurnAttempt {
    Normal(Box<crate::json_record_codec::ExactJson<StartAttemptView>>),
    Legacy(LegacyAttemptEvidence),
}

impl StoredTurnAttempt {
    pub fn from_stored(raw: String) -> Result<Self, CodecError> {
        match crate::json_record_codec::ExactJson::from_stored(raw.clone()) {
            Ok(value) => Ok(Self::Normal(Box::new(value))),
            Err(_) => crate::json_record_codec::ExactJson::from_stored(raw)
                .map(|raw| Self::Legacy(LegacyAttemptEvidence { raw }))
                .map_err(|_| CodecError::InvalidQuery),
        }
    }

    pub fn raw(&self) -> &str {
        match self {
            Self::Normal(value) => value.raw(),
            Self::Legacy(value) => value.raw(),
        }
    }

    pub fn normal(&self) -> Option<&StartAttemptView> {
        match self {
            Self::Normal(value) => Some(value.value()),
            Self::Legacy(_) => None,
        }
    }
}

pub struct TurnAttemptCodec;
impl Codec<Option<StoredTurnAttempt>> for TurnAttemptCodec {
    type Stored = Option<String>;
    type Encoded<'a> = Option<&'a str>;
    fn encode<'a>(
        _: &CodecContext,
        value: &'a Option<StoredTurnAttempt>,
    ) -> Result<Self::Encoded<'a>, CodecError> {
        Ok(value.as_ref().map(StoredTurnAttempt::raw))
    }
    fn decode(
        _: &CodecContext,
        value: Self::Stored,
    ) -> Result<Option<StoredTurnAttempt>, CodecError> {
        value.map(StoredTurnAttempt::from_stored).transpose()
    }
}
