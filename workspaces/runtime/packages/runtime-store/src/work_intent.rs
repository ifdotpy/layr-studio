use crate::request_results::AdmittedRequest;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PublicRequest {
    pub public_request_id: String,
    pub public_admission: AdmittedRequest,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Submission {
    pub version: i64,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Acceptance {
    pub result_id: String,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Owner<I> {
    pub request_id: String,
    pub owner: String,
    pub revision: String,
    #[serde(flatten)]
    pub id: I,
    #[serde(flatten)]
    pub public: Option<PublicRequest>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Evidence {
    pub layr_project_id: String,
    pub layr_line: String,
    pub layr_state_id: String,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SubmissionResult {
    pub evidence: Evidence,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MergeReceipt {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from_state: Option<String>,
    pub to_state: String,
}
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct AcceptanceResult {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub merge: Option<MergeReceipt>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum Intent<I, R> {
    Pending(Owner<I>),
    NotSent(Owner<I>),
    Unknown(Owner<I>),
    Completed {
        #[serde(flatten)]
        owner: Owner<I>,
        #[serde(flatten)]
        result: R,
    },
    UnknownUnattributed(PublicRequest),
}

#[derive(Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct View {
    /// Historical wire name retained for persisted native Layr receipts.
    #[serde(rename = "vmSubmit", default, skip_serializing_if = "Option::is_none")]
    pub submit: Option<Intent<Submission, SubmissionResult>>,
    /// Historical wire name retained for persisted native Layr receipts.
    #[serde(rename = "vmAccept", default, skip_serializing_if = "Option::is_none")]
    pub accept: Option<Intent<Acceptance, AcceptanceResult>>,
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn historical_native_work_intents_round_trip() {
        let stored = studio_json::json!({
            "vmSubmit": {"status":"completed", "requestId":"evidence", "owner":"worker", "revision":"state", "version":2, "evidence":{"layrProjectId":"project", "layrLine":"worker", "layrStateId":"state"}},
            "vmAccept": {"status":"completed", "requestId":"merge", "owner":"worker", "revision":"state", "resultId":"result", "merge":{"from_state":"old", "to_state":"new"}}
        });
        let view: View = studio_json::from_str(&stored.to_string()).expect("stored native work");
        assert_eq!(
            studio_json::from_str::<studio_json::Value>(
                &studio_json::to_string(&view).expect("serialized work")
            )
            .expect("JSON"),
            stored
        );
    }
    #[test]
    fn accepted_completion_retains_merge_receipt() {
        let stored = r#"{"status":"completed","requestId":"rpc","owner":"a","revision":"rev","resultId":"r","merge":{"from_state":"old","to_state":"new"}}"#;
        let intent: Intent<Acceptance, AcceptanceResult> = studio_json::from_str(stored).unwrap();
        let text = studio_json::to_string(&intent).unwrap();
        assert!(text.contains(r#""merge":{"from_state":"old","to_state":"new"}"#));
    }
    #[test]
    fn recovered_merge_receipt_omits_unknown_previous_state() {
        let receipt = MergeReceipt {
            from_state: None,
            to_state: "resolved".into(),
        };
        let value = studio_json::to_string(&receipt).unwrap();
        assert_eq!(value, r#"{"to_state":"resolved"}"#);
        assert_eq!(
            studio_json::from_str::<MergeReceipt>(&value).unwrap(),
            receipt
        );
    }
    #[test]
    fn accepted_completion_round_trips_without_result_field() {
        let stored = r#"{"status":"completed","requestId":"rpc","owner":"a","revision":"rev","resultId":"r"}"#;
        let intent: Intent<Acceptance, AcceptanceResult> =
            studio_json::from_str(stored).expect("stored completion");
        assert_eq!(studio_json::to_string(&intent).expect("serialized"), stored);
    }
}
