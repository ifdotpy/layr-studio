mod analytics_state_check;
mod cross_runtime;
mod fresh_state;
mod local_server_id;
mod migration_baseline;
mod online_migration;
mod receipts_cutover;
mod schema_compat;
mod sql_callback_guard;
mod startup_schema;
mod stored_text_analytics;
mod stored_text_owner;
mod terminal_output;
mod wal_backup_store;
mod writer_lanes;
// Register every integration test module here; the tooling witness checks coverage.
