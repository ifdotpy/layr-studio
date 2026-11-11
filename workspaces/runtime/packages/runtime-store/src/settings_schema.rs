//! Legacy settings and migration metadata descriptors. Physical format and owner units are unchanged.
use crate::json_record_codec::{ExactJson, ExactJsonCodec};
use crate::peer_update_settings::Canvas;
use studio_db_api::DbTable;
#[cfg(test)]
use studio_db_api::Table;

/// Known federation selector; all other members remain exact evidence.
#[derive(serde::Serialize, serde::Deserialize)]
pub struct FederationSettingsFields {
    pub enabled: bool,
}
studio_runtime_schema::raw_json_payload! {
    pub struct FederationSettingsPayload {
        pub fields: FederationSettingsFields,
        pub extensions,
    }
}

#[derive(DbTable)]
#[db(state_check = crate::state_check::count_legacy_rows::<ClientCommandWatermarks>)]
#[db(table = "client_command_watermarks", database = Canvas, legacy_ddl = "CREATE TABLE client_command_watermarks (\n    client  TEXT PRIMARY KEY,\n    applied INTEGER NOT NULL CHECK (applied >= 0)\n) WITHOUT ROWID")]
pub struct ClientCommandWatermarks {
    #[db(key)]
    pub client: String,
    #[db(check = "applied>=0")]
    pub applied: i64,
}

#[derive(DbTable)]
#[db(state_check = crate::state_check::count_legacy_rows::<AccessLocalAlias>)]
#[db(table = "runtime_access_local_alias", database = Canvas, legacy_ddl = "CREATE TABLE runtime_access_local_alias (\n          id INTEGER PRIMARY KEY CHECK(id=1), alias TEXT NOT NULL)")]
pub struct AccessLocalAlias {
    #[db(key, check = "id=1")]
    pub id: i64,
    pub alias: String,
}

#[derive(DbTable)]
#[db(state_check = crate::state_check::count_legacy_rows::<AccessSettings>)]
#[db(table = "runtime_access_settings", database = Canvas, legacy_ddl = "CREATE TABLE runtime_access_settings(id INTEGER PRIMARY KEY CHECK(id=1), auto_pair INTEGER NOT NULL)")]
pub struct AccessSettings {
    #[db(key, check = "id=1")]
    pub id: i64,
    pub auto_pair: i64,
}

#[derive(DbTable)]
#[db(table = "runtime_federation_settings", database = Canvas, state_check = federation_settings_state_check, legacy_ddl = "CREATE TABLE runtime_federation_settings (\n          id TEXT PRIMARY KEY, record TEXT NOT NULL)")]
pub struct FederationSettings {
    #[db(key)]
    pub id: String,
    #[db(codec = ExactJsonCodec<FederationSettingsPayload>)]
    pub record: ExactJson<FederationSettingsPayload>,
}

#[derive(DbTable)]
#[db(state_check = crate::state_check::count_legacy_rows::<OrchestrationSettings>)]
#[db(table = "runtime_orchestration_settings", database = Canvas, legacy_ddl = "CREATE TABLE runtime_orchestration_settings (id TEXT PRIMARY KEY, record TEXT NOT NULL)")]
pub(crate) struct OrchestrationSettings {
    #[db(key)]
    pub id: String,
    #[db(codec = ExactJsonCodec<studio_runtime_schema::json_object::JsonObject<crate::repositories::orchestration_settings::StoredSettings>>)]
    pub(crate) record: ExactJson<
        studio_runtime_schema::json_object::JsonObject<
            crate::repositories::orchestration_settings::StoredSettings,
        >,
    >,
}

#[derive(DbTable)]
#[db(state_check = crate::state_check::count_legacy_rows::<PayloadMigrations>)]
#[db(table = "runtime_payload_migrations", database = Canvas, legacy_ddl = "CREATE TABLE runtime_payload_migrations (name TEXT PRIMARY KEY,cursor INTEGER NOT NULL DEFAULT 0,complete INTEGER NOT NULL DEFAULT 0,status TEXT NOT NULL DEFAULT 'pending',updated REAL,error TEXT, cutover_ms INTEGER, receipt_enum_guard INTEGER GENERATED ALWAYS AS (NULL) VIRTUAL CHECK(\"status\" IN ('pending','running','waitingForSpace','error','complete')), cursor_key BLOB, copy_source_rows INTEGER, copy_target_rows INTEGER)")]
pub struct PayloadMigrations {
    #[db(key)]
    pub name: String,
    pub cursor: i64,
    pub complete: i64,
    pub status: String,
    pub updated: Option<f64>,
    pub error: Option<String>,
    pub cutover_ms: Option<i64>,
    #[db(generated)]
    pub receipt_enum_guard: Option<i64>,
    pub cursor_key: Option<Vec<u8>>,
    /// Row counts: both present record durable empty-target admission.
    pub copy_source_rows: Option<i64>,
    pub copy_target_rows: Option<i64>,
}

#[derive(DbTable)]
#[db(state_check = crate::state_check::count_legacy_rows::<SearchRollout>)]
#[db(table = "runtime_search_rollout", database = Canvas, legacy_ddl = "CREATE TABLE runtime_search_rollout (\n                id INTEGER PRIMARY KEY CHECK(id=1), phase TEXT NOT NULL,\n                cursor INTEGER NOT NULL DEFAULT 0, updated REAL NOT NULL)")]
pub struct SearchRollout {
    #[db(key, check = "id=1")]
    pub id: i64,
    pub phase: String,
    pub cursor: i64,
    pub updated: f64,
}

#[derive(DbTable)]
#[db(state_check = crate::state_check::count_legacy_rows::<SidebarOrder>)]
#[db(table = "runtime_sidebar_order", database = Canvas, legacy_ddl = "CREATE TABLE runtime_sidebar_order (id TEXT PRIMARY KEY, record TEXT NOT NULL)")]
pub struct SidebarOrder {
    #[db(key)]
    pub id: String,
    // Raw historical evidence: existing readers admit shapes beyond a typed envelope.
    pub record: String,
}

#[derive(DbTable)]
#[db(state_check = crate::state_check::count_legacy_rows::<StoredTextBatches>)]
#[db(table = "runtime_stored_text_batches", database = Canvas, legacy_ddl = "CREATE TABLE runtime_stored_text_batches (\n        name TEXT NOT NULL, batch INTEGER NOT NULL, first_rowid INTEGER NOT NULL,\n        last_rowid INTEGER NOT NULL, rows INTEGER NOT NULL, plain_bytes INTEGER NOT NULL,\n        stored_bytes INTEGER NOT NULL, sha256 TEXT NOT NULL, created REAL NOT NULL,\n        PRIMARY KEY(name, batch))")]
pub struct StoredTextBatches {
    #[db(key)]
    pub name: String,
    #[db(key)]
    pub batch: i64,
    pub first_rowid: i64,
    pub last_rowid: i64,
    pub rows: i64,
    pub plain_bytes: i64,
    pub stored_bytes: i64,
    pub sha256: String,
    pub created: f64,
}

#[derive(DbTable)]
#[db(state_check = crate::state_check::count_legacy_rows::<OnlineMigrationDirty>)]
#[db(table = "runtime_online_migration_dirty", database = Canvas, legacy_ddl = "CREATE TABLE runtime_online_migration_dirty(migration TEXT NOT NULL, key INTEGER NOT NULL, PRIMARY KEY(migration,key)) WITHOUT ROWID")]
pub struct OnlineMigrationDirty {
    #[db(key)]
    pub migration: String,
    #[db(key)]
    pub key: studio_db_api::migration::DirtyKey,
}

#[cfg(test)]
pub(crate) fn prepare_features(connection: &rusqlite::Connection) -> rusqlite::Result<()> {
    crate::canvas_schema::install(
        connection,
        &[
            AccessLocalAlias::descriptor(),
            AccessSettings::descriptor(),
            FederationSettings::descriptor(),
            OrchestrationSettings::descriptor(),
            SearchRollout::descriptor(),
            SidebarOrder::descriptor(),
        ],
    )
}

impl AccessSettings {
    pub fn get(connection: &rusqlite::Connection) -> rusqlite::Result<Option<Self>> {
        Ok(crate::peer_update_settings::canvas_read(connection)
            .get::<crate::metadata_schema::AccessSettings>(&(
                studio_db_api::migration::encode_key(&(1_i64,)),
            ))?
            .map(|row| Self {
                id: row.integer_key,
                auto_pair: row.auto_pair.as_stored(),
            }))
    }
    pub fn set_auto_pair(
        transaction: &rusqlite::Transaction<'_>,
        value: i64,
    ) -> rusqlite::Result<()> {
        crate::peer_update_settings::canvas_tx(transaction).update_existing(
            &crate::metadata_schema::AccessSettings {
                identity: studio_db_api::migration::encode_key(&(1_i64,)),
                source_rowid: 1,
                integer_key: 1,
                auto_pair: crate::metadata_values::StoredBoolean::from_stored(value),
            },
        )?;
        Ok(())
    }
}

impl AccessLocalAlias {
    pub fn get(connection: &rusqlite::Connection) -> rusqlite::Result<Option<Self>> {
        Ok(crate::peer_update_settings::canvas_read(connection)
            .get::<crate::metadata_schema::AccessLocalAlias>(&(
                studio_db_api::migration::encode_key(&(1_i64,)),
            ))?
            .map(|row| Self {
                id: row.integer_key,
                alias: row.alias,
            }))
    }
    pub fn upsert(&self, transaction: &rusqlite::Transaction<'_>) -> rusqlite::Result<()> {
        crate::peer_update_settings::canvas_tx(transaction)
            .partition::<crate::metadata_schema::AccessLocalAlias>()
            .put(&crate::metadata_schema::AccessLocalAlias {
                identity: studio_db_api::migration::encode_key(&(self.id,)),
                source_rowid: self.id,
                integer_key: self.id,
                alias: self.alias.clone(),
            })
    }
}

fn federation_settings_state_check(
    connection: &rusqlite::Connection,
) -> rusqlite::Result<studio_db_api::StateCheck> {
    use crate::repositories::federation_messaging::{FederationTable, check_table_state};
    crate::state_check::storage_report(check_table_state(connection, FederationTable::Settings))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn access_metadata_preserves_absence_update_only_alias_and_rollback() {
        let mut db = rusqlite::Connection::open_in_memory().unwrap();
        crate::metadata_schema::Metadata::descriptor()
            .install_if_absent(&db)
            .unwrap();
        assert!(AccessSettings::get(&db).unwrap().is_none());
        assert!(AccessLocalAlias::get(&db).unwrap().is_none());
        {
            let transaction = db.transaction().unwrap();
            AccessSettings::set_auto_pair(&transaction, 2).unwrap();
            assert!(
                AccessSettings::get(&transaction).unwrap().is_none(),
                "setting cannot insert an absent singleton"
            );
            crate::peer_update_settings::canvas_tx(&transaction)
                .partition::<crate::metadata_schema::AccessSettings>()
                .put(&crate::metadata_schema::AccessSettings {
                    identity: studio_db_api::migration::encode_key(&(1_i64,)),
                    source_rowid: 1,
                    integer_key: 1,
                    auto_pair: crate::metadata_values::StoredBoolean::from_stored(-7),
                })
                .unwrap();
            assert_eq!(
                AccessSettings::get(&transaction)
                    .unwrap()
                    .unwrap()
                    .auto_pair,
                -7
            );
            AccessSettings::set_auto_pair(&transaction, 2).unwrap();
            assert_eq!(
                AccessSettings::get(&transaction)
                    .unwrap()
                    .unwrap()
                    .auto_pair,
                2
            );
            AccessLocalAlias {
                id: 1,
                alias: "one".into(),
            }
            .upsert(&transaction)
            .unwrap();
            AccessLocalAlias {
                id: 1,
                alias: "two".into(),
            }
            .upsert(&transaction)
            .unwrap();
            assert!(
                AccessLocalAlias {
                    id: 2,
                    alias: "invalid".into()
                }
                .upsert(&transaction)
                .is_err()
            );
            assert_eq!(
                AccessLocalAlias::get(&transaction).unwrap().unwrap().alias,
                "two"
            );
            let read = crate::peer_update_settings::canvas_read(&transaction);
            assert!(!read.table_exists::<AccessLocalAlias>().unwrap());
            assert!(!read.table_exists::<AccessSettings>().unwrap());
        }
        assert!(AccessSettings::get(&db).unwrap().is_none());
        assert!(AccessLocalAlias::get(&db).unwrap().is_none());
    }
}
