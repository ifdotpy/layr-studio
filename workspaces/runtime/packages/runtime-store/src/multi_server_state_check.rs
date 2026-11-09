//! Read-only validation of the open access records and SQL delivery projections.
use crate::{StateCheckFailure, StateCheckTable, StoreError, StoreResult};
use rusqlite::Connection;
use std::collections::BTreeMap;
use studio_json::Value;

fn check_table(
    connection: &Connection,
    table: &'static str,
    projection: &str,
) -> StoreResult<StateCheckTable> {
    let mut report = StateCheckTable {
        table,
        row_count: 0,
        rejected_count: 0,
        failures: Vec::new(),
    };
    let exists: bool = connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE name=?1 AND type='table')",
        [table],
        |r| r.get(0),
    )?;
    if exists {
        let mut counts = BTreeMap::new();
        let mut statement = connection.prepare(&format!("SELECT id,{projection} FROM {table}"))?;
        let mut rows = statement.query([])?;
        while let Some(row) = rows.next()? {
            report.row_count += 1;
            let id = row.get::<_, String>(0).ok();
            let raw = row.get::<_, String>(1).ok();
            let error = if table == "runtime_access_invites" {
                match raw.as_deref().and_then(|raw| {
                    studio_json::from_str::<
                        studio_runtime_schema::http_ingress::Object<
                            studio_runtime_schema::multi_server_protocol::PersistedAccessInvite,
                        >,
                    >(raw)
                    .ok()
                }) {
                    Some(_) => None,
                    None => Some((String::new(), "recordType".to_owned())),
                }
            } else if matches!(
                table,
                "runtime_access_clients" | "runtime_access_discovered"
            ) {
                let decoded = raw.as_deref().and_then(|raw| {
                    studio_json::from_str::<
                        studio_runtime_schema::http_ingress::Object<
                            studio_runtime_schema::multi_server_access::PersistedAccessClient,
                        >,
                    >(raw)
                    .ok()
                });
                match (id.as_deref(), decoded) {
                    (Some(_), Some(_)) => None,
                    _ => Some((String::new(), "recordType".to_owned())),
                }
            } else {
                match raw.and_then(|s| studio_json::from_str::<Value>(&s).ok()) {
                    Some(value) => studio_runtime_schema::first_error(
                        studio_runtime_schema::table_schema(table).ok_or_else(|| {
                            StoreError::SchemaValidation(format!("no storage schema for {table}"))
                        })?,
                        &value,
                    )
                    .map_err(StoreError::SchemaValidation)?,
                    None => Some((String::new(), "JSON".to_owned())),
                }
            };
            if let Some(error) = error {
                report.rejected_count += 1;
                *counts.entry(error).or_insert(0) += 1;
            }
        }
        report.failures = counts
            .into_iter()
            .map(|((pointer, keyword), count)| StateCheckFailure {
                pointer,
                keyword,
                count,
            })
            .collect();
    }
    Ok(report)
}

fn legacy_check(
    connection: &Connection,
    table: &'static str,
    projection: &str,
) -> rusqlite::Result<studio_db_api::StateCheck> {
    crate::state_check::storage_report(check_table(connection, table, projection))
}

pub(crate) fn check_access_clients(
    connection: &Connection,
) -> rusqlite::Result<studio_db_api::StateCheck> {
    legacy_check(connection, "runtime_access_clients", "record")
}

pub(crate) fn check_access_discovered(
    connection: &Connection,
) -> rusqlite::Result<studio_db_api::StateCheck> {
    legacy_check(connection, "runtime_access_discovered", "record")
}

pub(crate) fn check_access_invites(
    connection: &Connection,
) -> rusqlite::Result<studio_db_api::StateCheck> {
    legacy_check(connection, "runtime_access_invites", "record")
}

pub(crate) fn check_server_inbox(
    connection: &Connection,
) -> rusqlite::Result<studio_db_api::StateCheck> {
    legacy_check(
        connection,
        "runtime_server_inbox",
        "json_object('id',id,'signature',signature,'state',state,'result',result,'created_at',created_at,'completed_at',completed_at)",
    )
}
