//! Typed, immutable admission of publication storage; original text is never reserialized.

use crate::json_record_codec::ExactJson;
use serde::Deserialize;
use studio_runtime_schema::raw_json::{RawExtensions, RawJson};
pub use studio_runtime_schema::transcript_cache::TranscriptRevisionCacheEvidence as TranscriptCacheValue;

pub type TranscriptCache = ExactJson<TranscriptCacheValue>;
#[derive(Debug, Default)]
pub enum Selector {
    #[default]
    Missing,
    Null,
    Text(String),
    Other(RawJson),
}
#[derive(Debug, Default)]
pub struct AccountSelectors {
    pub id: Selector,
    pub account_id: Selector,
}
#[derive(Debug, Default)]
pub struct PublicationSelectors {
    pub agent: Selector,
    pub root_id: Selector,
    pub project_path: Selector,
    pub home_server_id: Selector,
    pub account_key: Selector,
    pub rate_limit_account_id: Selector,
    pub accounts: Vec<AccountSelectors>,
    pub accounts_value: Selector,
    pub monitor_id: Selector,
    pub rule_id: Selector,
    pub agent_id: Selector,
    pub agent_name: Selector,
    pub review_targets: Selector,
}
impl PublicationSelectors {
    pub fn from_envelope(raw: &str) -> studio_json::Result<Self> {
        let envelope: RawJson = studio_json::from_str(raw)?;
        let fields = object(&envelope)
            .and_then(|map| map.get("value").and_then(raw_object))
            .unwrap_or_default();
        let observation = fields
            .get("observation")
            .and_then(raw_object)
            .unwrap_or_default();
        let data = observation
            .get("data")
            .and_then(raw_object)
            .unwrap_or_default();
        let accounts = fields
            .get("accounts")
            .and_then(|raw| {
                studio_json::from_str::<Vec<RawJson>>(raw.as_str())
                    .ok()
                    .or_else(|| {
                        raw_object(raw).map(|fields| fields.into_values().map(RawJson).collect())
                    })
            })
            .unwrap_or_default()
            .iter()
            .map(|raw| {
                let fields = object(raw).unwrap_or_default();
                AccountSelectors {
                    id: selector(fields.get("id")),
                    account_id: selector(fields.get("accountId")),
                }
            })
            .collect();
        Ok(Self {
            agent: selector(fields.get("agent")),
            root_id: selector(fields.get("rootId")),
            project_path: selector(fields.get("projectPath")),
            home_server_id: selector(fields.get("homeServerId")),
            account_key: selector(fields.get("accountKey")),
            rate_limit_account_id: selector(data.get("accountId")),
            accounts,
            accounts_value: selector(fields.get("accounts")),
            monitor_id: selector(fields.get("monitorId")),
            rule_id: selector(fields.get("ruleId")),
            agent_id: selector(fields.get("agentId")),
            agent_name: selector(fields.get("agentName")),
            review_targets: selector(fields.get("reviewTargets")),
        })
    }
}

fn object(raw: &RawJson) -> Option<RawExtensions> {
    studio_json::from_str(raw.0.as_str()).ok()
}
fn raw_object(raw: &studio_json::OwnedRawJson) -> Option<RawExtensions> {
    studio_json::from_str(raw.as_str()).ok()
}
fn selector(raw: Option<&studio_json::OwnedRawJson>) -> Selector {
    match raw {
        None => Selector::Missing,
        Some(raw) => match studio_json::from_str::<Option<String>>(raw.as_str()) {
            Ok(None) => Selector::Null,
            Ok(Some(text)) => Selector::Text(text),
            Err(_) => Selector::Other(RawJson(raw.clone())),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn publication_payload_cache_preserves_original_bytes_and_extensions() {
        let raw = " \n{\"items\":[{\"id\":\"\\u0061\",\"future\": [1.00,9007199254740993]}],\"truncated\":false,\"cursor\": null}\t";
        let cache = TranscriptCache::from_stored(raw.into()).unwrap();
        assert_eq!(cache.raw(), raw);
        assert!(
            matches!(cache.value(), TranscriptCacheValue::Page(page) if page.items[0].id.as_ref() == "a")
        );
    }

    #[test]
    fn publication_payload_cache_refuses_unsupported_shapes() {
        for raw in [
            "null",
            "[]",
            "{\"items\":[]}",
            "{\"items\":[{}],\"truncated\":false}",
            "{\"items\":[{\"id\":1}],\"truncated\":false}",
            "{\"future\":true}",
        ] {
            assert!(TranscriptCache::from_stored(raw.into()).is_err(), "{raw}");
        }
        assert!(matches!(
            TranscriptCache::from_stored(" {} ".into()).unwrap().value(),
            TranscriptCacheValue::Deleted(_)
        ));
    }

    #[test]
    fn publication_payload_selectors_keep_missing_null_and_wrong_type_distinct() {
        let selectors = PublicationSelectors::from_envelope(r#"{"value":{"agent":null,"rootId":4,"room":null,"reviewTargets":null,"observation":{"data":{"accountId":"account"}},"accounts":[{"id":"x","accountId":false}]}}"#).unwrap();
        assert!(matches!(selectors.agent, Selector::Null));
        assert!(matches!(selectors.root_id, Selector::Other(_)));
        assert!(matches!(selectors.project_path, Selector::Missing));
        assert!(matches!(selectors.review_targets, Selector::Null));
        assert!(
            matches!(selectors.rate_limit_account_id, Selector::Text(ref id) if id == "account")
        );
        assert!(matches!(
            selectors.accounts[0].account_id,
            Selector::Other(_)
        ));
    }
}

/// The generated union owns every DTO. Select its branch before decoding so its
/// broad fallback deserializer does not exhaust a normal thread's stack.
#[derive(Debug)]
pub struct LiveEntity(pub studio_runtime_schema::generated::SyncEntityPayload);
impl<'de> Deserialize<'de> for LiveEntity {
    fn deserialize<D: serde::Deserializer<'de>>(decoder: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        struct Tag {
            collection: String,
        }
        let raw = RawJson::deserialize(decoder)?;
        let tag: Tag = studio_json::from_str(raw.0.as_str()).map_err(serde::de::Error::custom)?;
        decode_live(&tag.collection, raw.0.as_str())
            .ok_or_else(|| serde::de::Error::custom("unknown publication collection"))?
            .map(Self)
            .map_err(serde::de::Error::custom)
    }
}
fn decode_live(
    collection: &str,
    raw: &str,
) -> Option<studio_json::Result<studio_runtime_schema::generated::SyncEntityPayload>> {
    use studio_runtime_schema::generated;
    macro_rules! branch {
        ($payload:ident, $tag:ident) => {
            if collection == generated::$tag::default().as_str() {
                return Some(
                    studio_json::from_str::<generated::$payload>(raw)
                        .map(generated::SyncEntityPayload::$payload),
                );
            }
        };
    }
    branch!(
        SyncAccountCostEntityPayload,
        SyncAccountCostEntityPayloadCollection
    );
    branch!(
        SyncAccountsEntityPayload,
        SyncAccountsEntityPayloadCollection
    );
    branch!(SyncAgentEntityPayload, SyncAgentEntityPayloadCollection);
    branch!(
        SyncAgentDetailEntityPayload,
        SyncAgentDetailEntityPayloadCollection
    );
    branch!(
        SyncAgentSummaryEntityPayload,
        SyncAgentSummaryEntityPayloadCollection
    );
    branch!(
        SyncAgentTailEntityPayload,
        SyncAgentTailEntityPayloadCollection
    );
    branch!(
        SyncAnnotationEntityPayload,
        SyncAnnotationEntityPayloadCollection
    );
    branch!(SyncChatEntityPayload, SyncChatEntityPayloadCollection);
    branch!(
        SyncComplaintEntityPayload,
        SyncComplaintEntityPayloadCollection
    );
    branch!(SyncDesktopEntityPayload, SyncDesktopEntityPayloadCollection);
    branch!(SyncEdgeEntityPayload, SyncEdgeEntityPayloadCollection);
    branch!(SyncEventEntityPayload, SyncEventEntityPayloadCollection);
    branch!(
        SyncMessageReceiptEntityPayload,
        SyncMessageReceiptEntityPayloadCollection
    );
    branch!(SyncModelEntityPayload, SyncModelEntityPayloadCollection);
    branch!(SyncMonitorEntityPayload, SyncMonitorEntityPayloadCollection);
    branch!(
        SyncPeerTeamEntityPayload,
        SyncPeerTeamEntityPayloadCollection
    );
    branch!(SyncPlanEntityPayload, SyncPlanEntityPayloadCollection);
    branch!(SyncProjectEntityPayload, SyncProjectEntityPayloadCollection);
    branch!(
        SyncQuestionHistoryEntityPayload,
        SyncQuestionHistoryEntityPayloadCollection
    );
    branch!(SyncQueueEntityPayload, SyncQueueEntityPayloadCollection);
    branch!(
        SyncRateLimitEntityPayload,
        SyncRateLimitEntityPayloadCollection
    );
    branch!(SyncRequestEntityPayload, SyncRequestEntityPayloadCollection);
    branch!(
        SyncRetainedWorktreeEntityPayload,
        SyncRetainedWorktreeEntityPayloadCollection
    );
    branch!(SyncRoomEntityPayload, SyncRoomEntityPayloadCollection);
    branch!(SyncRuleEntityPayload, SyncRuleEntityPayloadCollection);
    branch!(
        SyncSessionCostEntityPayload,
        SyncSessionCostEntityPayloadCollection
    );
    branch!(
        SyncStudioErrorEntityPayload,
        SyncStudioErrorEntityPayloadCollection
    );
    branch!(SyncTaskEntityPayload, SyncTaskEntityPayloadCollection);
    branch!(
        SyncTaskDetailEntityPayload,
        SyncTaskDetailEntityPayloadCollection
    );
    branch!(
        SyncTerminalEntityPayload,
        SyncTerminalEntityPayloadCollection
    );
    branch!(
        SyncUiPreferencesEntityPayload,
        SyncUiPreferencesEntityPayloadCollection
    );
    branch!(SyncWorkEntityPayload, SyncWorkEntityPayloadCollection);
    branch!(
        SyncWorkDecisionEntityPayload,
        SyncWorkDecisionEntityPayloadCollection
    );
    branch!(
        SyncWorkDetailEntityPayload,
        SyncWorkDetailEntityPayloadCollection
    );
    branch!(
        SyncWorkReleaseEntityPayload,
        SyncWorkReleaseEntityPayloadCollection
    );
    branch!(
        SyncWorkResultEntityPayload,
        SyncWorkResultEntityPayloadCollection
    );
    branch!(
        SyncWorkspaceEntityPayload,
        SyncWorkspaceEntityPayloadCollection
    );
    None
}

/// A missing SQL payload is not JSON null or an empty cached page.
pub enum EntityPayload {
    HashOnly,
    SqlNull,
    TranscriptCache(TranscriptCache),
    Live(Box<ExactJson<LiveEntity>>),
    Tombstone(ExactJson<TombstoneEnvelope>),
    Unknown { collection: String, raw: String },
    Historical { raw: String, reason: String },
}

#[derive(Debug, Deserialize)]
pub struct TombstoneEnvelope {
    pub collection: String,
    pub id: String,
    pub value: RawExtensions,
}
impl EntityPayload {
    pub fn from_stored(
        collection: &str,
        id: &str,
        deleted: bool,
        raw: Option<String>,
    ) -> Result<Self, String> {
        let transcript = crate::publication_collection::transcript_argument(collection).is_some();
        let Some(raw) = raw else {
            return Ok(if transcript {
                Self::HashOnly
            } else {
                Self::SqlNull
            });
        };
        if transcript {
            if id != "@revision" {
                return Err("Only transcript revision rows admit cached payloads".into());
            }
            return TranscriptCache::from_stored(raw)
                .map(Self::TranscriptCache)
                .map_err(|error| error.to_string());
        }
        // The generated union is the existing complete collection owner. Its discriminator
        // decides whether an open collection is known; do not maintain a second name list.
        let known = studio_runtime_schema::sync_entity_branch(collection)?.is_some();
        if !known {
            return Ok(Self::Unknown {
                collection: collection.into(),
                raw,
            });
        }
        let envelope = studio_json::from_str::<TombstoneEnvelope>(&raw);
        if let Ok(envelope) = &envelope
            && (envelope.collection != collection || envelope.id != id)
        {
            return Err("Publication identity mismatch".into());
        }
        if deleted {
            return Ok(match ExactJson::from_stored(raw.clone()) {
                Ok(tombstone) => Self::Tombstone(tombstone),
                Err(error) => Self::Historical {
                    raw,
                    reason: error.to_string(),
                },
            });
        }
        match ExactJson::from_stored(raw.clone()) {
            Ok(live) => Ok(Self::Live(Box::new(live))),
            Err(error) => Ok(Self::Historical {
                raw,
                reason: error.to_string(),
            }),
        }
    }
    pub fn raw(&self) -> Option<&str> {
        match self {
            Self::HashOnly | Self::SqlNull => None,
            Self::TranscriptCache(cache) => Some(cache.raw()),
            Self::Live(live) => Some(live.raw()),
            Self::Tombstone(tombstone) => Some(tombstone.raw()),
            Self::Unknown { raw, .. } | Self::Historical { raw, .. } => Some(raw),
        }
    }
}

#[cfg(test)]
mod entity_tests {
    use super::*;
    #[test]
    fn publication_payload_dispatch_matches_the_schema_owner() {
        let mut mapping: RawExtensions = studio_json::from_str(
            &std::fs::read_to_string(
                std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("../runtime-http/openapi.json"),
            )
            .unwrap(),
        )
        .unwrap();
        for name in [
            "components",
            "schemas",
            "SyncEntityPayload",
            "discriminator",
            "mapping",
        ] {
            mapping = studio_json::from_str(mapping.remove(name).unwrap().as_str()).unwrap();
        }
        for collection in mapping.keys() {
            assert!(decode_live(collection, "{}").is_some(), "{collection}");
        }
        assert!(decode_live("future", "{}").is_none());
    }
    #[test]
    fn publication_payload_entity_preserves_bytes_and_identity() {
        std::thread::spawn(|| {
        let raw = " \n{\"collection\":\"edge\",\"id\":\"e\",\"value\":{\"id\":\"e\",\"kind\":\"chat\"}}\t";
        let entity = EntityPayload::from_stored("edge", "e", false, Some(raw.into())).unwrap();
        assert!(matches!(entity, EntityPayload::Live(_)));
        assert_eq!(entity.raw(), Some(raw));
        assert!(EntityPayload::from_stored("edge", "wrong", false, Some(raw.into())).is_err());
        }).join().unwrap();
    }
    #[test]
    fn publication_payload_storage_alternatives_do_not_collapse() {
        assert!(matches!(
            EntityPayload::from_stored("transcript:a", "item:x", false, None).unwrap(),
            EntityPayload::HashOnly
        ));
        assert!(matches!(
            EntityPayload::from_stored("transcript:a", "@revision", false, Some("{}".into()))
                .unwrap(),
            EntityPayload::TranscriptCache(_)
        ));
        assert!(
            EntityPayload::from_stored("transcript:a", "item:x", false, Some("{}".into())).is_err()
        );
        for raw in ["null", "[]", "broken JSON", "{\"open\":1.00}"] {
            let unknown =
                EntityPayload::from_stored("future", "x", false, Some(raw.into())).unwrap();
            assert!(matches!(unknown, EntityPayload::Unknown { .. }));
            assert_eq!(unknown.raw(), Some(raw));
        }
        let raw = r#"{"collection":"edge","id":"e","value":{"agent":null,"rootId":0}}"#;
        let tombstone = EntityPayload::from_stored("edge", "e", true, Some(raw.into())).unwrap();
        assert!(matches!(tombstone, EntityPayload::Tombstone(_)));
        assert_eq!(tombstone.raw(), Some(raw));
    }
}

#[derive(Debug)]
pub enum DocumentPayload {
    Draft {
        draft: Box<studio_runtime_schema::drafts::DraftVersion>,
        raw: String,
    },
    Historical {
        raw: String,
        reason: String,
    },
    Unknown {
        scope: String,
        raw: String,
    },
}
impl DocumentPayload {
    pub fn from_stored(scope: &str, id: &str, raw: String) -> Result<Self, String> {
        if scope != "drafts" {
            return Ok(Self::Unknown {
                scope: scope.into(),
                raw,
            });
        }
        let mut members = match studio_json::from_str::<RawExtensions>(&raw) {
            Ok(members) => members,
            Err(error) => {
                return Ok(Self::Historical {
                    raw,
                    reason: error.to_string(),
                });
            }
        };
        if let Some(stored_id) = members.get("id") {
            if studio_json::from_str::<String>(stored_id.as_str()).is_ok_and(|stored| stored != id)
            {
                return Err("Draft identity mismatch".into());
            }
        } else {
            members.insert(
                "id".into(),
                studio_json::OwnedRawJson::from_string(
                    studio_json::to_string(id).map_err(|error| error.to_string())?,
                )
                .map_err(|error| error.to_string())?,
            );
        }
        let encoded = studio_json::to_string(&members).map_err(|error| error.to_string())?;
        match studio_json::from_str::<studio_runtime_schema::drafts::DraftVersion>(&encoded) {
            Ok(draft) => match draft.validate() {
                Ok(()) => Ok(Self::Draft {
                    draft: Box::new(draft),
                    raw,
                }),
                Err(reason) => Ok(Self::Historical { raw, reason }),
            },
            Err(error) => Ok(Self::Historical {
                raw,
                reason: error.to_string(),
            }),
        }
    }
    pub fn raw(&self) -> &str {
        match self {
            Self::Draft { raw, .. } | Self::Historical { raw, .. } | Self::Unknown { raw, .. } => {
                raw
            }
        }
    }
}

#[cfg(test)]
mod document_tests {
    use super::*;
    #[test]
    fn publication_payload_draft_uses_owner_defaults_without_rewriting_storage() {
        let raw = " {\"session\":\"s\",\"device\":\"d\",\"text\":\"\\u0061\",\"updated\":1.75,\"extra\": [1.00,9007199254740993]} ";
        let document = DocumentPayload::from_stored("drafts", "d:s", raw.into()).unwrap();
        assert_eq!(document.raw(), raw);
        assert!(
            matches!(document, DocumentPayload::Draft { ref draft, .. } if draft.id == "d:s" && draft.updated == 1 && draft.text == "a")
        );
    }
    #[test]
    fn publication_payload_draft_refuses_identity_mismatch_and_keeps_historical_text() {
        let raw = r#"{"id":"other","session":"s","device":"d","text":""}"#;
        assert!(DocumentPayload::from_stored("drafts", "d:s", raw.into()).is_err());
        for raw in ["not json", "null", "[]", "{\"text\":4}"] {
            let legacy = DocumentPayload::from_stored("drafts", "d:s", raw.into()).unwrap();
            assert!(matches!(legacy, DocumentPayload::Historical { .. }));
            assert_eq!(legacy.raw(), raw);
        }
        let unknown = DocumentPayload::from_stored("future", "x", "raw text".into()).unwrap();
        assert!(matches!(unknown, DocumentPayload::Unknown { .. }));
    }
}
