//! Persisted cleanup intent and effect tags.
pub use crate::records::{AgentArchive, CleanedWorktree};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use studio_json::{Number, OwnedRawJson};
use studio_runtime_schema::queue_command::Field;

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct WorktreeIdentity {
    pub id: String,
    pub cwd: String,
    pub root: String,
    pub repository: String,
    pub relative: String,
    pub head: String,
    pub branch: Option<String>,
    pub lifecycle_identity: Option<(Option<i64>, Option<String>, Option<f64>)>,
    pub epoch: Option<i64>,
    pub deleted_at: Option<f64>,
    pub archive: Option<AgentArchive>,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CleanupAction {
    Delete,
    Archive,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum CleanupEffect<I = WorktreeIdentity> {
    ArchiveImage,
    RemoveImage,
    SaveNativeTurn,
    ReleaseNativeLine,
    SaveVmTurn { turn_id: String },
    ReleaseVmLine { method: String },
    SaveWorktreeRef { identity: I },
    RemoveWorktree { identity: I },
}

// Decode the tag without serde's internally-tagged buffering, which cannot retain raw JSON.
#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
enum CleanupEffectKind {
    ArchiveImage,
    RemoveImage,
    SaveNativeTurn,
    ReleaseNativeLine,
    SaveVmTurn,
    ReleaseVmLine,
    SaveWorktreeRef,
    RemoveWorktree,
}

impl<'de, I: serde::de::DeserializeOwned> Deserialize<'de> for CleanupEffect<I> {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        struct Fields {
            kind: CleanupEffectKind,
            #[serde(default)]
            turn_id: Field<OwnedRawJson>,
            // Required only for release_vm_line; other effect kinds omit it.
            #[serde(default, rename = "method")]
            vm_method: Field<OwnedRawJson>,
            #[serde(default)]
            identity: Field<OwnedRawJson>,
        }
        fn read<T: serde::de::DeserializeOwned, E: serde::de::Error>(
            field: Field<OwnedRawJson>,
            name: &'static str,
        ) -> Result<T, E> {
            let raw = match field {
                Field::Missing => return Err(E::missing_field(name)),
                Field::Null => return studio_json::from_str("null").map_err(E::custom),
                Field::Value(raw) => raw,
            };
            studio_json::from_str(raw.as_str()).map_err(E::custom)
        }
        let fields = Fields::deserialize(deserializer)?;
        Ok(match fields.kind {
            CleanupEffectKind::ArchiveImage => Self::ArchiveImage,
            CleanupEffectKind::RemoveImage => Self::RemoveImage,
            CleanupEffectKind::SaveNativeTurn => Self::SaveNativeTurn,
            CleanupEffectKind::ReleaseNativeLine => Self::ReleaseNativeLine,
            CleanupEffectKind::SaveVmTurn => Self::SaveVmTurn {
                turn_id: read(fields.turn_id, "turn_id")?,
            },
            CleanupEffectKind::ReleaseVmLine => Self::ReleaseVmLine {
                method: read(fields.vm_method, "method")?,
            },
            CleanupEffectKind::SaveWorktreeRef => Self::SaveWorktreeRef {
                identity: read(fields.identity, "identity")?,
            },
            CleanupEffectKind::RemoveWorktree => Self::RemoveWorktree {
                identity: read(fields.identity, "identity")?,
            },
        })
    }
}

studio_runtime_schema::raw_json_payload! { flat
/// Older retained intents can contain only the path available at the failure.
#[derive(Clone, Debug, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PartialWorktreeIdentity {
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub root: Field<String>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub repository: Field<String>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub relative: Field<String>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub head: Field<String>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub branch: Field<String>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub lifecycle_identity: Field<(Option<i64>, Option<String>, Option<f64>)>,
    // Preserve unknown fields in retained identities from newer versions.
    @extensions pub extra,
}
}

impl From<WorktreeIdentity> for PartialWorktreeIdentity {
    fn from(value: WorktreeIdentity) -> Self {
        fn nullable<T>(value: Option<T>) -> Field<T> {
            value.map_or(Field::Null, Field::Value)
        }
        Self {
            root: Field::Value(value.root),
            repository: Field::Value(value.repository),
            relative: Field::Value(value.relative),
            head: Field::Value(value.head),
            branch: nullable(value.branch),
            lifecycle_identity: nullable(value.lifecycle_identity),
            extra: BTreeMap::new(),
        }
    }
}

studio_runtime_schema::raw_json_payload! { flat
#[derive(Clone, Debug, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LifecycleIdentity {
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub id: Field<String>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub epoch: Field<i64>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub cwd: Field<String>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub deleted_at: Field<Number>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub archive_at: Field<Number>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub archive_epoch: Field<u64>,
    // Preserve identity extensions while forwarding a stored intent.
    @extensions pub extra,
}
}

studio_runtime_schema::raw_json_payload! { flat
#[derive(Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct StoredCleanupIntent {
    pub action: CleanupAction,
    pub phase: CleanupPhase,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    // These legacy values are forwarded unchanged; retry attempts are decoded only when used.
    pub operation_id: Field<OwnedRawJson>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub request_id: Field<String>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub attempt: Field<OwnedRawJson>,
    // The cleanup runner decodes the effect once; raw storage preserves extensions.
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub effect: Field<OwnedRawJson>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    // Preserve exact numeric spelling and unknown fields until a comparison needs the identity.
    pub identity: Field<OwnedRawJson>,
    // Provider receipts are forwarded raw and decoded by the effect consumer.
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub receipt: Field<OwnedRawJson>,
    // Preserve unknown fields from newer stored intent versions.
    @extensions pub extra,
}
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CleanupPhase {
    Queued,
    Written,
    NotSent,
    Pending,
    Unknown,
}
