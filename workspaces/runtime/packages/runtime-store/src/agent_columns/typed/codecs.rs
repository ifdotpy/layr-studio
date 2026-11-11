//! Application codecs bind primitive BLOBs through the db-api context.
use studio_db_api::{Codec, CodecContext, CodecError};

use super::AgentIdentity;
use super::evidence::{
    AccountTransferIdEvidence, DeletedAtEvidence, IdleEpisodeFireAtEvidence, IdleEpisodeIdEvidence,
    IsLeadEvidence, NativeFailureHoldEvidence, NativeReleaseResetPendingEvidence,
    ProviderStreamAckEvidence, ProviderStreamGenerationEvidence, RemoteAnchorEvidence,
    RemoteWorkerEvidence, ReviewArchiveNextAtEvidence, WorkspaceOperationEvidence,
};

pub struct AgentIdentityCodec;

pub struct AgentIdCodec;
impl Codec<Option<crate::AgentId>> for AgentIdCodec {
    type Stored = Option<String>;
    type Encoded<'a> = Option<&'a str>;
    fn encode<'a>(
        _: &CodecContext,
        value: &'a Option<crate::AgentId>,
    ) -> Result<Self::Encoded<'a>, CodecError> {
        Ok(value.as_ref().map(AsRef::as_ref))
    }
    fn decode(
        _: &CodecContext,
        value: Option<String>,
    ) -> Result<Option<crate::AgentId>, CodecError> {
        Ok(value.map(Into::into))
    }
}

impl Codec<crate::AgentId> for AgentIdCodec {
    type Stored = String;
    type Encoded<'a> = &'a str;
    fn encode<'a>(_: &CodecContext, value: &'a crate::AgentId) -> Result<&'a str, CodecError> {
        Ok(value.as_ref())
    }
    fn decode(_: &CodecContext, value: String) -> Result<crate::AgentId, CodecError> {
        Ok(value.into())
    }
}

impl Codec<AgentIdentity> for AgentIdentityCodec {
    type Stored = Vec<u8>;
    type Encoded<'a> = &'a [u8];

    fn encode<'a>(_: &CodecContext, value: &'a AgentIdentity) -> Result<&'a [u8], CodecError> {
        Ok(value.as_bytes())
    }

    fn decode(_: &CodecContext, bytes: Vec<u8>) -> Result<AgentIdentity, CodecError> {
        AgentIdentity::from_bytes(&bytes).map_err(|_| CodecError::InvalidQuery)
    }
}

macro_rules! evidence_codecs {
    ($($codec:ident: $value:ident),+ $(,)?) => {$ (
        pub struct $codec;
        impl Codec<$value> for $codec {
            type Stored = Vec<u8>;
            type Encoded<'a> = Vec<u8>;
            fn encode(_: &CodecContext, value: &$value) -> Result<Vec<u8>, CodecError> {
                Ok(value.evidence_bytes())
            }
            fn decode(_: &CodecContext, bytes: Vec<u8>) -> Result<$value, CodecError> {
                $value::from_evidence_bytes(&bytes).map_err(|_| CodecError::InvalidQuery)
            }
        }
    )+};
}

evidence_codecs! {
    DeletedAtCodec: DeletedAtEvidence,
    WorkspaceOperationCodec: WorkspaceOperationEvidence,
    RemoteWorkerCodec: RemoteWorkerEvidence,
    RemoteAnchorCodec: RemoteAnchorEvidence,
    NativeFailureHoldCodec: NativeFailureHoldEvidence,
    ReviewArchiveNextAtCodec: ReviewArchiveNextAtEvidence,
    IdleEpisodeFireAtCodec: IdleEpisodeFireAtEvidence,
    IdleEpisodeIdCodec: IdleEpisodeIdEvidence,
    ProviderStreamGenerationCodec: ProviderStreamGenerationEvidence,
    ProviderStreamAckCodec: ProviderStreamAckEvidence,
    AccountTransferIdCodec: AccountTransferIdEvidence,
    NativeReleaseResetPendingCodec: NativeReleaseResetPendingEvidence,
    IsLeadCodec: IsLeadEvidence,
}
