//! Receipt schema and fixture support retained after historical migration retirement.
use crate::request_results::{AdmittedRequest, ResultEnvelope};
use rusqlite::{OptionalExtension, Transaction, params};
use serde::Serialize;
use studio_db_api::Table;

#[derive(Debug, thiserror::Error)]
pub enum ReceiptMigrationError {
    #[error(transparent)]
    Sql(#[from] rusqlite::Error),
    #[error("receipt migration cannot classify table={table} key={key} signature={signature}")]
    Unclassifiable {
        table: String,
        key: String,
        signature: String,
    },
}
pub type Result<T> = std::result::Result<T, ReceiptMigrationError>;

fn unclassifiable(table: &str, key: &str, signature: &str) -> ReceiptMigrationError {
    ReceiptMigrationError::Unclassifiable {
        table: table.into(),
        key: key.into(),
        signature: signature.into(),
    }
}

pub fn import_result<R: Serialize>(
    tx: &Transaction<'_>,
    id: &str,
    signature: &str,
    envelope: &ResultEnvelope<R>,
    created: i64,
) -> Result<()> {
    let result = studio_json::to_string(envelope)
        .map_err(|_| unclassifiable("request_results", id, signature))?;
    let existing: Option<(String, String)> = tx
        .query_row(
            "SELECT signature,result FROM request_results WHERE id=?1",
            [id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()?;
    if let Some((saved_signature, saved_result)) = existing {
        if saved_signature != signature || saved_result != result {
            return Err(unclassifiable("request_results", id, signature));
        }
        return Ok(());
    }
    let admitted = admitted_owner_token(id, signature, created);
    tx.execute(
        "INSERT INTO request_results(id,signature,result,created) VALUES(?1,?2,?3,?4)",
        params![
            admitted.id(),
            admitted.signature(),
            result,
            admitted.created()
        ],
    )?;
    Ok(())
}

pub(crate) fn admitted_owner_token(id: &str, signature: &str, cutover_ms: i64) -> AdmittedRequest {
    AdmittedRequest::from_migration(id, signature, cutover_ms)
}

pub fn seed_expired_before_ms(tx: &Transaction<'_>, cutover_ms: i64) -> Result<()> {
    let floor = crate::repositories::orchestration_settings::request_expiration_floor(tx)?;
    let missing = crate::peer_update_settings::canvas_read(tx)
        .get::<crate::metadata_schema::OrchestrationSettings>(&(
            crate::consolidation_codec::nullable_text_identity(&Some("global".into()), 0),
        ))?
        .is_none();
    if missing || cutover_ms > floor {
        crate::repositories::orchestration_settings::set_request_expiration_floor(tx, cutover_ms)?;
    }
    Ok(())
}

pub fn prepare_schema(tx: &Transaction<'_>) -> Result<()> {
    crate::receipt_schema::create(tx)?;
    crate::metadata_schema::Metadata::descriptor().install_if_absent(tx)?;
    Ok(())
}
#[cfg(test)]
mod canonical_tests {
    #[test]
    fn receipt_preparation_and_expiry_use_canonical_metadata() {
        let mut db = crate::test_canvas();
        let transaction = db.transaction().unwrap();
        super::prepare_schema(&transaction).unwrap();
        assert!(
            !crate::schema::table_exists(&transaction, "runtime_orchestration_settings").unwrap()
        );
        assert!(!crate::schema::table_exists(&transaction, "runtime_payload_migrations").unwrap());
        super::seed_expired_before_ms(&transaction, 25).unwrap();
        super::seed_expired_before_ms(&transaction, 10).unwrap();
        assert_eq!(
            crate::repositories::orchestration_settings::request_expiration_floor(&transaction)
                .unwrap(),
            25
        );
        transaction.rollback().unwrap();
        assert_eq!(
            crate::repositories::orchestration_settings::request_expiration_floor(&db).unwrap(),
            0
        );
    }
}
