//! Admission of the historical transcript revision cache, distinct from the live wire DTO.
//! These decode-only views preserve the reader's permissive extension contract.
//! Keep the original text alongside the view; never reconstruct stored bytes from it.
use crate::raw_json::RawExtensions;
use crate::transcript_record::TranscriptItemId;
use serde::Deserialize;

pub struct TranscriptCachedItemEvidence {
    pub id: TranscriptItemId,
}
pub struct TranscriptCachedPageEvidence {
    pub items: Vec<TranscriptCachedItemEvidence>,
    pub truncated: bool,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DeletedTranscriptCacheEvidence {}

/// A supported writer emits either a page or the deleted conversation's empty object.
/// The latter can remain after a later restore, so admission is independent of actor state.
pub enum TranscriptRevisionCacheEvidence {
    Page(TranscriptCachedPageEvidence),
    Deleted(DeletedTranscriptCacheEvidence),
}

// The old reader parsed an object map first: duplicate known members keep the
// last value, including when an earlier occurrence has the wrong type.
fn field<T: serde::de::DeserializeOwned, E: serde::de::Error>(
    members: &mut RawExtensions,
    name: &'static str,
) -> Result<T, E> {
    let raw = members.remove(name).ok_or_else(|| E::missing_field(name))?;
    studio_json::from_str(raw.as_str()).map_err(E::custom)
}
fn page<E: serde::de::Error>(
    mut members: RawExtensions,
) -> Result<TranscriptCachedPageEvidence, E> {
    Ok(TranscriptCachedPageEvidence {
        items: field(&mut members, "items")?,
        truncated: field(&mut members, "truncated")?,
    })
}
impl<'de> Deserialize<'de> for TranscriptCachedItemEvidence {
    fn deserialize<D: serde::Deserializer<'de>>(decoder: D) -> Result<Self, D::Error> {
        let mut members = RawExtensions::deserialize(decoder)?;
        Ok(Self {
            id: field(&mut members, "id")?,
        })
    }
}
impl<'de> Deserialize<'de> for TranscriptCachedPageEvidence {
    fn deserialize<D: serde::Deserializer<'de>>(decoder: D) -> Result<Self, D::Error> {
        page(RawExtensions::deserialize(decoder)?)
    }
}
impl<'de> Deserialize<'de> for TranscriptRevisionCacheEvidence {
    fn deserialize<D: serde::Deserializer<'de>>(decoder: D) -> Result<Self, D::Error> {
        let members = RawExtensions::deserialize(decoder)?;
        if members.is_empty() {
            Ok(Self::Deleted(DeletedTranscriptCacheEvidence {}))
        } else {
            page(members).map(Self::Page)
        }
    }
}

pub fn validate_live_page(raw: &str) -> studio_json::Result<()> {
    studio_json::from_str::<TranscriptCachedPageEvidence>(raw).map(|_| ())
}
pub fn validate_revision_cache(raw: &str) -> studio_json::Result<()> {
    studio_json::from_str::<TranscriptRevisionCacheEvidence>(raw).map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn f1_cache_duplicate_known_members_keep_the_last_reader_value() {
        let page = r#"{"items":[{"id":1,"id":"one"}],"truncated":null,"truncated":false}"#;
        assert!(validate_live_page(page).is_ok());
        assert!(validate_revision_cache(page).is_ok());
    }
    #[test]
    fn f1_cache_evidence_matches_the_live_reader_and_deleted_writer() {
        let page = r#"{"items":[{"id":"one","legacy":[1,null]}],"truncated":false,"agent":{"future":true}}"#;
        assert!(validate_live_page(page).is_ok());
        assert!(validate_revision_cache(page).is_ok());
        assert!(validate_revision_cache("{}").is_ok());
        assert!(validate_live_page("{}").is_err());
        for invalid in [
            r#"{"items":[],"truncated":null}"#,
            r#"{"items":[{}],"truncated":false}"#,
            r#"{"items":[{"id":1}],"truncated":false}"#,
            r#"{"items":{},"truncated":false}"#,
            "null",
            "not json",
        ] {
            assert!(validate_revision_cache(invalid).is_err(), "{invalid}");
        }
    }
}
