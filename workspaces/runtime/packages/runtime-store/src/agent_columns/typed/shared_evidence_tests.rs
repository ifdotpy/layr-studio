//! Preservation witnesses use the actual reachable payload owners, before conversion.
const FUTURE: &str = r#"{ "n":1.00e+3, "dup":1, "dup":2 }"#;

fn preserves<T: serde::de::DeserializeOwned + serde::Serialize>(raw: &str, occurrences: usize) {
    let payload: T = studio_json::from_str(raw).expect("accepted existing payload");
    let written = studio_json::to_string(&payload).expect("unchanged wire owner");
    assert_eq!(
        written.matches(FUTURE).count(),
        occurrences,
        "exact future number/member tokens: {written}"
    );
}

#[test]
fn flat_agent_draft_preserves_exact_raw_extensions_without_a_stored_wrapper() {
    preserves::<super::record::AgentDraft>(
        &format!(r#"{{"text":"draft","prefixText":"prefix","assets":[],"future":{FUTURE}}}"#),
        1,
    );
}

#[test]
fn shared_continuation_extensions_preserve_exact_tokens() {
    preserves::<studio_runtime_schema::continuation_snapshot::UsageContinuation>(
        &format!(
            r#"{{"id":"x","accountKey":"x","threadId":"x","epoch":0,"turnId":"x","status":"scheduled","failedAt":0,"future":{FUTURE}}}"#
        ),
        1,
    );
}

#[test]
fn shared_recovery_error_and_extensions_preserve_exact_tokens() {
    preserves::<studio_runtime_schema::recovery_record::RecoveryFields>(
        &format!(r#"{{"error":{FUTURE},"future":{FUTURE}}}"#),
        2,
    );
}

#[test]
fn shared_recovery_supervisor_extensions_preserve_exact_tokens() {
    preserves::<studio_runtime_schema::recovery_record::RecoverySupervisorIdentity>(
        &format!(r#"{{"future":{FUTURE}}}"#),
        1,
    );
}

#[test]
fn shared_transfer_history_result_preserves_exact_tokens() {
    preserves::<crate::transfer_record::TransferAccountHistoryEntry>(
        &format!(r#"{{"accountKey":"a","result":{FUTURE}}}"#),
        1,
    );
}

#[test]
fn shared_worker_defaults_extensions_preserve_exact_tokens() {
    preserves::<crate::transfer_record::TransferWorkerDefaults>(
        &format!(r#"{{"future":{FUTURE}}}"#),
        1,
    );
}

#[test]
fn shared_transfer_summary_original_preserves_exact_tokens() {
    preserves::<crate::transfer_record::UnreadableTransferSummary>(
        &format!(r#"{{"kind":"unreadable","reason":"legacy","original":{FUTURE}}}"#),
        1,
    );
}

#[test]
fn shared_provider_error_extensions_preserve_exact_tokens() {
    preserves::<studio_runtime_schema::provider_errors::NativeProviderError>(
        &format!(r#"{{"message":"error","future":{FUTURE}}}"#),
        1,
    );
}

#[test]
fn shared_codex_error_detail_extensions_preserve_exact_tokens() {
    preserves::<studio_runtime_schema::provider_errors::CodexErrorDetail>(
        &format!(r#"{{"future":{FUTURE}}}"#),
        1,
    );
}

#[test]
fn shared_codex_error_other_preserves_exact_tokens() {
    preserves::<studio_runtime_schema::provider_errors::CodexErrorPayload>(
        &format!("[{FUTURE}]"),
        1,
    );
}
