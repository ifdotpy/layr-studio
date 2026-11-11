//! Exact held-selector evidence. Each selector has its own concrete Rust type.
//! The two-byte purpose/class header is an evidence format, never a query/key format.

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum EvidenceDecodeError {
    #[error("held selector evidence belongs to another selector")]
    WrongSelector,
    #[error("invalid held selector evidence class or length")]
    InvalidShape,
    #[error("held selector text is not UTF-8")]
    InvalidText,
}

macro_rules! selectors {
    ($($name:ident = $purpose:literal),+ $(,)?) => {$ (
        #[derive(Clone, Debug, PartialEq)]
        pub enum $name {
            Null,
            Integer(i64),
            Real(f64),
            Text(Box<str>),
        }

        impl $name {
            pub fn evidence_bytes(&self) -> Vec<u8> {
                let mut bytes = vec![$purpose];
                match self {
                    Self::Null => bytes.push(0),
                    Self::Integer(value) => { bytes.push(1); bytes.extend(value.to_le_bytes()); }
                    Self::Real(value) => { bytes.push(2); bytes.extend(value.to_bits().to_le_bytes()); }
                    Self::Text(value) => { bytes.push(3); bytes.extend(value.as_bytes()); }
                }
                bytes
            }

            pub fn from_evidence_bytes(bytes: &[u8]) -> Result<Self, EvidenceDecodeError> {
                let Some((&purpose, body)) = bytes.split_first() else {
                    return Err(EvidenceDecodeError::InvalidShape);
                };
                if purpose != $purpose { return Err(EvidenceDecodeError::WrongSelector); }
                let Some((&class, payload)) = body.split_first() else {
                    return Err(EvidenceDecodeError::InvalidShape);
                };
                match class {
                    0 if payload.is_empty() => Ok(Self::Null),
                    1 => payload.try_into().map(i64::from_le_bytes).map(Self::Integer)
                        .map_err(|_| EvidenceDecodeError::InvalidShape),
                    2 => payload.try_into().map(u64::from_le_bytes).map(f64::from_bits).map(Self::Real)
                        .map_err(|_| EvidenceDecodeError::InvalidShape),
                    3 => std::str::from_utf8(payload).map(|text| Self::Text(text.into()))
                        .map_err(|_| EvidenceDecodeError::InvalidText),
                    _ => Err(EvidenceDecodeError::InvalidShape),
                }
            }
        }
    )+};
}

// Permanent purpose tags. Names retain the source selector's meaning and unit.
selectors! {
    DeletedAtEvidence = 0,
    WorkspaceOperationEvidence = 1,
    RemoteWorkerEvidence = 2,
    RemoteAnchorEvidence = 3,
    NativeFailureHoldEvidence = 4,
    ReviewArchiveNextAtEvidence = 5,
    IdleEpisodeFireAtEvidence = 6,
    IdleEpisodeIdEvidence = 7,
    ProviderStreamGenerationEvidence = 8,
    ProviderStreamAckEvidence = 9,
    AccountTransferIdEvidence = 10,
    NativeReleaseResetPendingEvidence = 11,
    IsLeadEvidence = 12,
}
