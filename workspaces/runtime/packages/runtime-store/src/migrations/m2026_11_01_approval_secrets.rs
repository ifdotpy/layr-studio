//! Redact secret question answers copied by the earlier approvals migration.
use crate::{StoreError, StoreResult, SyncDocumentsRepository, approvals};
use rusqlite::{OptionalExtension, Transaction};
use std::collections::BTreeSet;
use studio_runtime_schema::approvals::{Answer, ApprovalState, QuestionAnswer, Request};

pub(super) fn apply(tx: &Transaction<'_>) -> StoreResult<()> {
    if !crate::schema::table_exists(tx, "approvals")? {
        return Ok(());
    }
    let legacy_exists = crate::schema::table_exists(tx, "runtime_requests")?;
    let sync_exists = crate::schema::table_exists(tx, "sync_entities")?;
    for mut approval in approvals::all(tx)? {
        let mut secrets: BTreeSet<String> = match &approval.request {
            Request::Question { questions } | Request::AgentQuestion { questions } => questions
                .iter()
                .filter(|q| q.is_secret)
                .map(|q| q.id.clone())
                .collect(),
            Request::Exec { .. }
            | Request::Patch { .. }
            | Request::Permissions { .. }
            | Request::Elicitation { .. }
            | Request::ToolUse { .. }
            | Request::MonitorStart { .. }
            | Request::BrowserOpen(_) => continue,
        };
        if legacy_exists {
            let raw: Option<String> = tx
                .query_row(
                    "SELECT record FROM runtime_requests WHERE id=?1",
                    [approval.id.as_str()],
                    |row| row.get(0),
                )
                .optional()?;
            if let Some(raw) = raw {
                match super::m2026_10_30_approvals::legacy_secret_ids(approval.id.as_str(), &raw) {
                    Ok(ids) => secrets.extend(ids),
                    // One legacy row the strict reader cannot place (for example, no createdAt)
                    // keeps only the secret flags of its approval. It never fails the migration.
                    Err(error) => eprintln!(
                        "approval secret redaction used the approval's own secret flags: {error}"
                    ),
                }
            }
        }
        let answer = match &mut approval.state {
            ApprovalState::Answering { answer, .. }
            | ApprovalState::Answered { answer, .. }
            | ApprovalState::Unknown { answer, .. } => answer,
            ApprovalState::Pending | ApprovalState::Closed { .. } => continue,
        };
        let Answer::Answers { answers } = &mut *answer else {
            continue;
        };
        let mut changed = false;
        for answer in answers {
            if secrets.contains(&answer.id) && answer.answer != [QuestionAnswer::REDACTED] {
                answer.digest = Some(crate::sha256_hex(answer.answer.join("\u{0}").as_bytes()));
                answer.answer = vec![QuestionAnswer::REDACTED.to_owned()];
                changed = true;
            }
        }
        if !changed {
            continue;
        }
        tx.execute(
            "UPDATE approvals SET answer=?2 WHERE id=?1",
            (approval.id.as_str(), serde_json::to_string(answer)?),
        )?;
        if sync_exists {
            for collection in ["request", "questionHistory"] {
                let raw: Option<String> = tx.query_row("SELECT payload FROM sync_entities WHERE collection=?1 AND id=?2 AND deleted=0", (collection, approval.id.as_str()), |row| row.get(0)).optional()?;
                if let Some(raw) = raw {
                    let mut envelope: serde_json::Value = serde_json::from_str(&raw)?;
                    let value = envelope
                        .get_mut("value")
                        .and_then(serde_json::Value::as_object_mut)
                        .ok_or(StoreError::RecordContract("invalid approval projection"))?;
                    let state = value
                        .get_mut("state")
                        .and_then(serde_json::Value::as_object_mut)
                        .ok_or(StoreError::RecordContract("invalid approval state"))?;
                    state.insert(
                        "answer".into(),
                        serde::Serialize::serialize(&*answer, serde_json::value::Serializer)?,
                    );
                    SyncDocumentsRepository::put_entity(
                        tx,
                        collection,
                        approval.id.as_str(),
                        value,
                        false,
                    )?;
                }
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use studio_runtime_schema::approvals::{Answer, ApprovalId, ApprovalState};

    #[tokio::test]
    async fn redacts_existing_approval_and_history_without_touching_public_answers() {
        let dir = tempfile::tempdir().unwrap();
        let store = crate::RuntimeStore::open(Some(dir.path())).await.unwrap();
        store.transaction(|tx| {
            let legacy = json!({"id":"s","method":"agent/asyncQuestion","agent":"a","status":"answered","createdAt":1.0,
                "params":{"questions":[{"id":"0","question":"?","isSecret":true},{"id":"1","question":"?"}]},
                "decision":"answer","answeredBy":"user","answeredAt":2.0,
                "answerHistory":[{"id":"0","question":"?","isSecret":true,"answer":["sensitive-value"]},{"id":"1","question":"?","isSecret":false,"answer":["public-value"]}]});
            let mut approval = super::super::m2026_10_30_approvals::approval_from_record("s", &legacy.to_string())?;
            let ApprovalState::Answered { answer: Answer::Answers { answers }, .. } = &mut approval.state else { panic!("answered"); };
            answers[0].answer = vec!["sensitive-value".into()]; answers[0].digest = None;
            approvals::put(tx, &approval)?;
            tx.execute("INSERT INTO runtime_requests(id,record) VALUES('s',?1)", [legacy.to_string()])?;
            SyncDocumentsRepository::put_entity(tx, "questionHistory", "s", &approval, false)?;
            apply(tx)?;
            let row = approvals::get(tx, &ApprovalId("s".into()))?.unwrap();
            let stored = serde_json::to_string(&row)?;
            assert!(!stored.contains("sensitive-value"));
            assert!(stored.contains("public-value"));
            let projection: String = tx.query_row("SELECT payload FROM sync_entities WHERE collection='questionHistory' AND id='s'", [], |r| r.get(0))?;
            assert!(!projection.contains("sensitive-value"));
            assert!(projection.contains("public-value"));
            let seq = SyncDocumentsRepository::entity_sequence(tx)?;
            apply(tx)?;
            assert_eq!(seq, SyncDocumentsRepository::entity_sequence(tx)?);
            Ok::<_, StoreError>(())
        }).await.unwrap();
    }

    #[tokio::test]
    async fn a_legacy_row_without_created_at_is_skipped_for_its_legacy_secrets_only() {
        let dir = tempfile::tempdir().unwrap();
        let store = crate::RuntimeStore::open(Some(dir.path())).await.unwrap();
        store.transaction(|tx| {
            // The question history migration converts a row without createdAt with time 0.
            // The raw row keeps no createdAt, so the strict legacy read cannot place it.
            let raw = json!({"id":"s","method":"agent/asyncQuestion","agent":"a","status":"answered",
                "params":{"questions":[{"id":"0","question":"?","isSecret":true}]},
                "decision":"answer","answeredBy":"user","answeredAt":2.0,
                "answerHistory":[{"id":"0","question":"?","isSecret":true,"answer":["sensitive-value"]}]});
            let mut converted = raw.clone();
            converted["createdAt"] = json!(0.0);
            let mut approval = super::super::m2026_10_30_approvals::approval_from_record("s", &converted.to_string())?;
            let ApprovalState::Answered { answer: Answer::Answers { answers }, .. } = &mut approval.state else { panic!("answered"); };
            answers[0].answer = vec!["sensitive-value".into()]; answers[0].digest = None;
            approvals::put(tx, &approval)?;
            tx.execute("INSERT INTO runtime_requests(id,record) VALUES('s',?1)", [raw.to_string()])?;
            SyncDocumentsRepository::put_entity(tx, "questionHistory", "s", &approval, false)?;
            apply(tx)?;
            let row = approvals::get(tx, &ApprovalId("s".into()))?.unwrap();
            let stored = serde_json::to_string(&row)?;
            assert!(!stored.contains("sensitive-value"));
            Ok::<_, StoreError>(())
        }).await.unwrap();
    }
}
