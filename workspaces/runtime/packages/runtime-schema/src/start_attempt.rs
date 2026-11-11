//! Typed fields shared by stored execution and recovery attempts.
use serde::{Deserialize, Serialize};

/// Identity of the retained supervisor that owns a provider connection.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SupervisorIdentity {
    pub state_dir: String,
    pub handle: String,
    pub generation: u64,
}

/// Durable transport identity; response contents alone never authorize mutation.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ProviderTicket {
    pub operation: String,
    pub agent: String,
    pub epoch: u64,
    pub account: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thread: Option<String>,
    pub connection: String,
    pub generation: u64,
    pub request: String,
}

/// A provider request; legacy agent snapshots can omit its root request ID.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct NativeRequest<R = String, P = studio_json::Value> {
    pub root_request_id: R,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub admitted_request: Option<crate::persistence::AdmittedRequest>,
    pub ticket: ProviderTicket,
    pub method: String,
    pub params: P,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ActionIdentity {
    pub account_key: String,
    pub epoch: i64,
    pub thread_id: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ModelSettingsAttempt {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub account_key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub connection_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub epoch: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub method: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub operation: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<crate::raw_json::RawJson>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub settings: Option<crate::raw_json::RawJson>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub request_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub root_request_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scope: Option<crate::provider_call_contract::ConnectionScope>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<crate::persistence::NativeSettingsStatus>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thread_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider_request: Option<crate::persistence::AdmittedRequest>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ClaudeInputEvent {
    pub id: String,
    pub kind: String,
    pub created: f64,
    pub assets: Vec<crate::raw_json::RawJson>,
    pub text_hash: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ClaudeInputSource {
    #[serde(
        default,
        skip_serializing_if = "crate::queue_command::Field::is_missing"
    )]
    #[schemars(with = "Option<String>")]
    pub cwd: crate::queue_command::Field<String>,
    #[serde(
        default,
        skip_serializing_if = "crate::queue_command::Field::is_missing"
    )]
    #[schemars(with = "Option<String>")]
    pub root_id: crate::queue_command::Field<String>,
    #[serde(
        default,
        skip_serializing_if = "crate::queue_command::Field::is_missing"
    )]
    #[schemars(with = "Option<String>")]
    pub parent_id: crate::queue_command::Field<String>,
    #[serde(
        default,
        skip_serializing_if = "crate::queue_command::Field::is_missing"
    )]
    #[schemars(with = "Option<String>")]
    pub role: crate::queue_command::Field<String>,
    #[serde(
        default,
        skip_serializing_if = "crate::queue_command::Field::is_missing"
    )]
    #[schemars(with = "Option<bool>")]
    pub is_lead: crate::queue_command::Field<bool>,
    #[serde(
        default,
        skip_serializing_if = "crate::queue_command::Field::is_missing"
    )]
    #[schemars(with = "Option<String>")]
    pub model: crate::queue_command::Field<String>,
    #[serde(
        default,
        skip_serializing_if = "crate::queue_command::Field::is_missing"
    )]
    #[schemars(with = "Option<String>")]
    pub effort: crate::queue_command::Field<String>,
    #[serde(
        default,
        skip_serializing_if = "crate::queue_command::Field::is_missing"
    )]
    #[schemars(with = "Option<String>")]
    pub native_effort: crate::queue_command::Field<String>,
    #[serde(
        default,
        skip_serializing_if = "crate::queue_command::Field::is_missing"
    )]
    #[schemars(with = "Option<bool>")]
    pub fast_mode: crate::queue_command::Field<bool>,
    #[serde(
        default,
        skip_serializing_if = "crate::queue_command::Field::is_missing"
    )]
    #[schemars(with = "Option<crate::claude_permissions::PermissionMode>")]
    pub permission_mode: crate::queue_command::Field<crate::claude_permissions::PermissionMode>,
    #[serde(
        default,
        skip_serializing_if = "crate::queue_command::Field::is_missing"
    )]
    #[schemars(with = "Option<bool>")]
    pub yolo_mode: crate::queue_command::Field<bool>,
    #[serde(
        default,
        skip_serializing_if = "crate::queue_command::Field::is_missing"
    )]
    #[schemars(with = "Option<u64>")]
    pub compactions: crate::queue_command::Field<u64>,
    #[serde(
        default,
        skip_serializing_if = "crate::queue_command::Field::is_missing"
    )]
    #[schemars(with = "Option<bool>")]
    pub image_workspace: crate::queue_command::Field<bool>,
    #[serde(
        default,
        skip_serializing_if = "crate::queue_command::Field::is_missing"
    )]
    #[schemars(with = "Option<bool>")]
    pub image_workspace_ready: crate::queue_command::Field<bool>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ClaudeInputRequest {
    pub client_user_message_id: String,
    pub configuration: crate::raw_json::RawJson,
    pub input: Vec<crate::raw_json::RawJson>,
    pub source: ClaudeInputSource,
    pub events: Vec<ClaudeInputEvent>,
    pub text_hash: String,
    pub transcript_item_id: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum VmPreparationPhase {
    Submitted,
    NotSent,
    Unknown,
    Failed,
    Completed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ProjectImportPhase {
    Planned,
    Uploading,
    Imported,
    NotSent,
    Unknown,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectImport {
    pub project_id: String,
    pub source: String,
    pub request_id: String,
    pub operation: String,
    pub upload_id: String,
    pub root: String,
    pub phase: ProjectImportPhase,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub archive_bytes: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub archive_sha256: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct VmWorkspacePreparation {
    pub request_id: String,
    pub method: String,
    pub params: crate::raw_json::RawJson,
    pub phase: VmPreparationPhase,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project_import: Option<ProjectImport>,
}

/// Target of a native review request.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(tag = "type", deny_unknown_fields)]
pub enum ReviewTarget {
    #[serde(rename = "uncommittedChanges")]
    UncommittedChanges,
    #[serde(rename = "baseBranch")]
    BaseBranch { branch: String },
    #[serde(rename = "commit")]
    Commit { sha: String, title: Option<String> },
    #[serde(rename = "custom")]
    Custom { instructions: String },
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StartAttemptView {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub epoch: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub account_key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thread_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub turn_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub observed_turn_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub events: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub submitted: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub active_at_reservation: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created: Option<f64>,
    #[serde(default, deserialize_with = "present_action")]
    #[schemars(with = "StartAttemptAction")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub action: Option<StartAttemptAction>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub execution_outcome: Option<crate::persistence::ExecutionStatus>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub request_ticket: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub connection_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub connection_generation: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub native_operation_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub root_request_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub interrupt: Option<Box<StartInterruptView>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub action_identity: Option<Box<ActionIdentity>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub action_request_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub capacity_retry_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub claude_retry_of: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub claude_input_request: Option<Box<ClaudeInputRequest>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model_settings: Option<Box<ModelSettingsAttempt>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub native_request: Option<Box<NativeRequest<Option<String>, crate::raw_json::RawJson>>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider_request: Option<crate::persistence::AdmittedRequest>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider_turn: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prepare_error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub not_submitted_reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response_error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retired_events: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub review_target: Option<Box<ReviewTarget>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub settings_fixed: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub supervisor_identity: Option<Box<SupervisorIdentity>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vm_workspace_preparation: Option<Box<VmWorkspacePreparation>>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "sqlite", derive(studio_db_api::DbEnum))]
#[repr(i64)]
pub enum StartAttemptAction {
    Compact = 0,
    Review = 1,
    Capacity = 2,
    Safety = 3,
}

fn present_action<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<StartAttemptAction>, D::Error> {
    StartAttemptAction::deserialize(deserializer).map(Some)
}

#[derive(Default)]
pub struct StartAttemptFields {
    pub attempt: StartAttemptView,
    seen: std::collections::BTreeSet<String>,
}

impl<'de> crate::raw_object::RawObjectFields<'de> for StartAttemptFields {
    type Output = StartAttemptView;
    fn field<M: serde::de::MapAccess<'de>>(
        &mut self,
        name: &str,
        map: &mut M,
    ) -> Result<bool, M::Error> {
        macro_rules! field {
            ($wire:literal, $field:ident) => {
                if name == $wire {
                    if !self.seen.insert(name.to_owned()) {
                        return Err(serde::de::Error::duplicate_field($wire));
                    }
                    self.attempt.$field = map.next_value()?;
                    return Ok(true);
                }
            };
        }
        field!("id", id);
        field!("epoch", epoch);
        field!("accountKey", account_key);
        field!("threadId", thread_id);
        field!("turnId", turn_id);
        field!("observedTurnId", observed_turn_id);
        field!("events", events);
        field!("submitted", submitted);
        field!("activeAtReservation", active_at_reservation);
        field!("created", created);
        if name == "action" {
            if !self.seen.insert(name.to_owned()) {
                return Err(serde::de::Error::duplicate_field("action"));
            }
            self.attempt.action = Some(map.next_value()?);
            return Ok(true);
        }
        field!("executionOutcome", execution_outcome);
        field!("requestTicket", request_ticket);
        field!("connectionId", connection_id);
        field!("connectionGeneration", connection_generation);
        field!("nativeOperationId", native_operation_id);
        field!("rootRequestId", root_request_id);
        field!("interrupt", interrupt);
        field!("actionIdentity", action_identity);
        field!("actionRequestId", action_request_id);
        field!("capacityRetryId", capacity_retry_id);
        field!("claudeRetryOf", claude_retry_of);
        field!("claudeInputRequest", claude_input_request);
        field!("modelSettings", model_settings);
        field!("nativeRequest", native_request);
        field!("providerRequest", provider_request);
        field!("providerTurn", provider_turn);
        field!("prepareError", prepare_error);
        field!("notSubmittedReason", not_submitted_reason);
        field!("responseError", response_error);
        field!("retiredEvents", retired_events);
        field!("reviewTarget", review_target);
        field!("settingsFixed", settings_fixed);
        field!("supervisorIdentity", supervisor_identity);
        field!("vmWorkspacePreparation", vm_workspace_preparation);
        Ok(false)
    }
    fn finish<E: serde::de::Error>(
        self,
        extra: std::collections::BTreeMap<String, studio_json::OwnedRawJson>,
    ) -> Result<Self::Output, E> {
        if let Some((name, _)) = extra.into_iter().next() {
            return Err(E::custom(format!("unknown start attempt field {name}")));
        }
        Ok(self.attempt)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum InterruptOutcome {
    Unknown,
    Unsent,
    Accepted,
    Rejected,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StartInterruptView {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub request_ticket: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thread_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub turn_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub generation: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub outcome: Option<InterruptOutcome>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response_error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub root_request_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub admission: Option<crate::persistence::AdmittedRequest>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub terminals_admission: Option<crate::persistence::AdmittedRequest>,
}

#[cfg(test)]
mod tests {
    use super::{NativeRequest, StartAttemptView};
    use serde::Deserialize;

    #[cfg(feature = "sqlite")]
    #[test]
    fn start_attempt_action_codes_preserve_wire_and_refuse_other_storage() {
        use super::StartAttemptAction;
        use rusqlite::types::{FromSql, ToSql, ToSqlOutput, Value, ValueRef};
        for (action, wire, code) in [
            (StartAttemptAction::Compact, "compact", 0),
            (StartAttemptAction::Review, "review", 1),
            (StartAttemptAction::Capacity, "capacity", 2),
            (StartAttemptAction::Safety, "safety", 3),
        ] {
            assert_eq!(
                studio_json::to_string(&action).unwrap(),
                format!("\"{wire}\"")
            );
            assert_eq!(
                action.to_sql().unwrap(),
                ToSqlOutput::Owned(Value::Integer(code))
            );
            assert_eq!(
                StartAttemptAction::column_result(ValueRef::Integer(code)).unwrap(),
                action
            );
            assert!(StartAttemptAction::column_result(ValueRef::Text(wire.as_bytes())).is_err());
        }
        assert!(StartAttemptAction::column_result(ValueRef::Integer(4)).is_err());
        assert!(StartAttemptAction::column_result(ValueRef::Null).is_err());
    }

    #[test]
    fn project_import_written_by_current_version_is_readable_by_base_version() {
        // Frozen from the base version: the persisted phase is required.
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase", deny_unknown_fields)]
        struct BaseProjectImport {
            project_id: String,
            source: String,
            request_id: String,
            operation: String,
            upload_id: String,
            root: String,
            phase: BasePhase,
            archive_bytes: Option<u64>,
            archive_sha256: Option<String>,
        }
        #[derive(Deserialize, PartialEq, Debug)]
        #[serde(rename_all = "snake_case")]
        enum BasePhase {
            Planned,
            Uploading,
            Imported,
            NotSent,
            Unknown,
        }
        for (phase, expected) in [
            (super::ProjectImportPhase::Planned, BasePhase::Planned),
            (super::ProjectImportPhase::Uploading, BasePhase::Uploading),
            (super::ProjectImportPhase::Imported, BasePhase::Imported),
            (super::ProjectImportPhase::NotSent, BasePhase::NotSent),
            (super::ProjectImportPhase::Unknown, BasePhase::Unknown),
        ] {
            let current = super::ProjectImport {
                project_id: "p".into(),
                source: "s".into(),
                request_id: "r".into(),
                operation: "o".into(),
                upload_id: "u".into(),
                root: "/".into(),
                phase,
                archive_bytes: None,
                archive_sha256: None,
            };
            let serialized = studio_json::to_vec(&current).unwrap();
            let old: BaseProjectImport = studio_json::from_slice(&serialized).unwrap();
            assert_eq!(old.phase, expected);
            assert_eq!(
                (
                    old.project_id,
                    old.source,
                    old.request_id,
                    old.operation,
                    old.upload_id,
                    old.root
                ),
                (
                    "p".into(),
                    "s".into(),
                    "r".into(),
                    "o".into(),
                    "u".into(),
                    "/".into()
                )
            );
            assert!(old.archive_bytes.is_none() && old.archive_sha256.is_none());
        }
    }

    #[test]
    fn direct_and_value_attempt_decodes_accept_provider_payload() {
        let raw = r#"{"id":"attempt","nativeRequest":{"ticket":{"operation":"op","agent":"a","epoch":1,"account":"acct","thread":"thread","connection":"c","generation":1,"request":"r"},"method":"review/start","params":{"z":  1,"a":2}}}"#;
        let direct: StartAttemptView = studio_json::from_str(raw).unwrap();
        assert_eq!(
            direct.native_request.unwrap().params.0.as_str(),
            r#"{"z":  1,"a":2}"#
        );
        // Existing in-memory callers have already lost the source whitespace.
        let value: studio_json::Value = studio_json::from_str(raw).unwrap();
        let projected = StartAttemptView::deserialize(value).unwrap();
        assert_eq!(
            projected.native_request.unwrap().params.0.as_str(),
            r#"{"a":2,"z":1}"#
        );
        // The live request type still requires the root identity.
        let native = r#"{"ticket":{"operation":"op","agent":"a","epoch":1,"account":"acct","thread":"thread","connection":"c","generation":1,"request":"r"},"method":"review/start","params":{}}"#;
        assert!(studio_json::from_str::<NativeRequest>(native).is_err());
    }

    #[test]
    fn attempt_rejects_unknown_duplicate_and_null_action() {
        assert!(studio_json::from_str::<StartAttemptView>(r#"{"future":1}"#).is_err());
        assert!(studio_json::from_str::<StartAttemptView>(r#"{"id":null,"id":null}"#).is_err());
        assert!(studio_json::from_str::<StartAttemptView>(r#"{"action":null}"#).is_err());
        assert!(studio_json::from_str::<StartAttemptView>(r#"{"action":"review"}"#).is_ok());
    }
}
