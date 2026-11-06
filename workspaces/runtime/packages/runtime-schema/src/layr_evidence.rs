//! Wire fields shared by native Layr evidence replies.
use serde::Serialize;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EvidenceResponse {
    pub state_id: String,
    pub project_id: Option<String>,
    pub line: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_evidence_projection_bytes() {
        let response = EvidenceResponse {
            state_id: "state".into(),
            project_id: Some("project".into()),
            line: Some("line".into()),
        };
        let response = studio_json::to_value(response).unwrap();
        assert_eq!(
            studio_json::to_vec(&response).unwrap(),
            br#"{"line":"line","projectId":"project","stateId":"state"}"#
        );
    }
}
