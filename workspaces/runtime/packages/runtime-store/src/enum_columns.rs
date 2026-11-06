//! Enum columns that generated triggers check, and the parity check that the stored
//! triggers equal the generated ones (#630 item 142). A new enum value then needs a
//! trigger replacement (a migration step), never a table rebuild.
use rusqlite::Connection;
use studio_runtime_schema::persistence::{
    GraphEdgeKind, RequestReferenceKind, RequestResultStatus, VoiceApprovalStatus,
    VoiceSessionState, VoiceSpeechState, check_enum_triggers, enum_check_triggers,
};

use crate::{StoreError, StoreResult};

/// One checked column: its table, its name, and the generated trigger SQL and parity
/// check of its enum.
pub(crate) struct EnumColumn {
    pub table: &'static str,
    pub column: &'static str,
    triggers: fn(&str, &str) -> String,
    check: fn(&Connection, &str, &str) -> Result<(), String>,
}

macro_rules! column {
    ($table:literal, $column:literal, $enum:ty) => {
        EnumColumn {
            table: $table,
            column: $column,
            triggers: enum_check_triggers::<$enum>,
            check: check_enum_triggers::<$enum>,
        }
    };
}

/// Every enum column with generated check triggers. The `chats`, `turns` and
/// `sessions` rows have their own generated triggers (`domain::check_triggers_sql`);
/// other enum columns have a table `CHECK`.
pub(crate) const ENUM_COLUMNS: &[EnumColumn] = &[
    column!("request_results", "status", RequestResultStatus),
    column!("request_results", "ref_kind", RequestReferenceKind),
    column!("voice_sessions", "state", VoiceSessionState),
    column!("voice_approvals", "status", VoiceApprovalStatus),
    column!("voice_speech_requests", "state", VoiceSpeechState),
    column!("graph_edges", "kind", GraphEdgeKind),
];

/// Install (or replace) the generated triggers of every listed column whose table exists.
pub(crate) fn install(connection: &Connection) -> StoreResult<()> {
    for column in ENUM_COLUMNS {
        if crate::schema::table_exists(connection, column.table)? {
            connection.execute_batch(&(column.triggers)(column.table, column.column))?;
        }
    }
    Ok(())
}

/// The parity check on a store: each listed column of an existing table has exactly the
/// generated triggers.
pub(crate) fn check(connection: &Connection) -> StoreResult<()> {
    for column in ENUM_COLUMNS {
        if crate::schema::table_exists(connection, column.table)? {
            (column.check)(connection, column.table, column.column)
                .map_err(StoreError::SchemaMismatch)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn a_fresh_store_has_the_generated_triggers_and_refuses_an_unknown_value() {
        let db = crate::fresh_state::test_canvas();
        super::check(&db).expect("parity");
        let error = db
            .execute(
                "INSERT INTO voice_speech_requests(id,session,record_id,state) VALUES('r','s','x','sent')",
                [],
            )
            .unwrap_err()
            .to_string();
        assert!(error.contains("outside its enum"), "{error}");
        db.execute("DROP TRIGGER \"enum_check_graph_edges_kind_ins\"", [])
            .unwrap();
        assert!(
            super::check(&db).is_err(),
            "a missing trigger fails the check"
        );
    }
}
