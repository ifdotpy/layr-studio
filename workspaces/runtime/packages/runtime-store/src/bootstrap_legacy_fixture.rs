//! Frozen source-schema fixture for the F1 legacy-table structural preflight.
//! This module is compiled only for tests; current startup never installs these tables.
use super::*;

pub(crate) fn prepare_sync_schema(transaction: &Transaction<'_>) -> StoreResult<()> {
    let sections = parse_sections(SYNC_SCHEMA)?;
    let has_runtime_items = transaction.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='runtime_items' COLLATE NOCASE)",
        [],
        |row| row.get::<_, bool>(0),
    )?;
    let (expected, findings) = expected_catalog(transaction, sections, has_runtime_items)?;
    if let Some(finding) = findings.into_iter().next() {
        return Err(schema_object_mismatch(&finding.object, &finding.reason));
    }

    reconcile_derived_objects(transaction, &expected)?;
    crate::clock_schema::prepare_sync_tables(transaction)?;
    crate::queue_schema::install_index::<crate::queue_schema::EventsRow>(
        transaction,
        "runtime_event_created_id",
    )?;
    transaction.execute_batch(sections[0])?;
    transaction.execute_batch(sections[1])?;
    transaction.execute_batch(sections[2])?;
    if has_runtime_items {
        transaction.execute_batch(sections[3])?;
    }
    Ok(())
}

fn reconcile_derived_objects(
    transaction: &Transaction<'_>,
    expected: &[CatalogObject],
) -> StoreResult<()> {
    for (kind, expected_name, expected_sql) in expected {
        let existing = transaction
            .query_row(
                "SELECT name,sql FROM sqlite_master WHERE type=?1 AND name=?2 COLLATE NOCASE",
                [kind, expected_name],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, Option<String>>(1)?)),
            )
            .optional()?;
        match existing {
            Some((_, Some(sql))) if sql == *expected_sql => {}
            Some((name, _)) => {
                transaction.execute_batch(&format!(
                    "DROP {} {}",
                    kind.to_ascii_uppercase(),
                    quote_identifier(&name)
                ))?;
            }
            None => {}
        }
    }
    Ok(())
}

fn quote_identifier(identifier: &str) -> String {
    format!("\"{}\"", identifier.replace('"', "\"\""))
}
