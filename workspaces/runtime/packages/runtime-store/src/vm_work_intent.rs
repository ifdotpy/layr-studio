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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vm_submit: Option<Intent<Submission, SubmissionResult>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vm_accept: Option<Intent<Acceptance, AcceptanceResult>>,
}

#[cfg(test)]
mod tests {
    use super::*;
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
