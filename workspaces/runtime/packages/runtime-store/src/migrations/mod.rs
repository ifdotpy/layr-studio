//! Supported baseline verification and the remaining dated Canvas transitions.
//! The caller holds runtime.lock; baseline inspection precedes every state write.
mod baseline_manifest;
mod m2026_11_09_immutable_entity_scopes;
pub(crate) mod m2026_11_09_online_keys;
mod m2026_11_09_repair_invalid_entity_scopes;
mod m2026_11_09_retired_objects;
mod m2026_11_11_consolidate_f1;
mod m2026_11_11_remove_account_throttle;
mod m2026_11_11_retired_agent_keys;
mod m2026_11_11_strict_native_review;
mod m2026_11_11_writer_query_indexes;
mod m2026_11_11_writer_receipt_indexes;
mod m2026_11_12_changes_label_sequence;
mod m2026_11_12_direct_durable_keys;
mod m2026_11_12_event_publication_members;
mod m2026_11_12_scheduler_wake_indexes;
mod m2026_11_12_writer_window_indexes;

use crate::{StoreError, StoreResult};
use baseline_manifest::RETIRED;
use rusqlite::{Connection, OpenFlags, OptionalExtension, Transaction, TransactionBehavior};
use std::path::Path;
use studio_runtime_schema::studio_errors::{
    MigrationPhase, MigrationRollbackOutcome, MigrationTarget,
};

/// Existing analytics evidence of the provider-identity transition; format tag.
pub(crate) const ANALYTICS_PROVIDER_FORMAT_KEY: &str = "providerIdentityFormat";
pub(crate) const PROVIDER_IDENTITY_FORMAT: &str = "2026-10-26";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MigrationDate {
    year: u16,
    month: u8,
    day: u8,
}
#[derive(Clone, Copy, Debug)]
pub struct MigrationDefinition {
    pub id: &'static str,
    pub introduced: MigrationDate,
    pub target: MigrationTarget,
    pub supported_source: &'static str,
    pub remove_when: &'static str,
}
struct SqlMigration {
    definition: MigrationDefinition,
    apply: fn(&Transaction<'_>) -> StoreResult<()>,
}
impl SqlMigration {
    fn definition(&self) -> MigrationDefinition {
        self.definition
    }
}
const fn definition(id: &'static str, source: &'static str) -> MigrationDefinition {
    MigrationDefinition {
        id,
        introduced: MigrationDate {
            year: 2026,
            month: 11,
            day: 9,
        },
        target: MigrationTarget::Canvas,
        supported_source: source,
        remove_when: "Supported installations and backups no longer admit this source format",
    }
}
static CATALOG: &[SqlMigration] = &[
    SqlMigration {
        definition: definition(
            "2026-11-09-online-keys",
            "Integer-only online migration progress",
        ),
        apply: m2026_11_09_online_keys::apply,
    },
    SqlMigration {
        definition: definition(
            "2026-11-09-immutable-entity-scopes",
            "Installations supporting agent and task re-parenting",
        ),
        apply: m2026_11_09_immutable_entity_scopes::apply,
    },
    SqlMigration {
        definition: definition(
            "2026-11-09-repair-invalid-entity-scopes",
            "Immutable guards with invalid historical selectors",
        ),
        apply: m2026_11_09_repair_invalid_entity_scopes::apply,
    },
    SqlMigration {
        definition: definition(
            "2026-11-09-retired-objects",
            "Retained obsolete budget and orphan schema objects",
        ),
        apply: m2026_11_09_retired_objects::apply,
    },
    SqlMigration {
        definition: MigrationDefinition {
            target: MigrationTarget::Analytics,
            ..definition(
                "2026-11-09-retired-objects",
                "Retired analytics capture receipt tables",
            )
        },
        apply: m2026_11_09_retired_objects::analytics,
    },
    SqlMigration {
        definition: MigrationDefinition {
            id: "2026-11-11-remove-account-throttle",
            introduced: MigrationDate {
                year: 2026,
                month: 11,
                day: 11,
            },
            target: MigrationTarget::Canvas,
            supported_source: "Per-account throttle waits on agent records",
            remove_when: "Supported installations and backups no longer contain account throttle waits",
        },
        apply: m2026_11_11_remove_account_throttle::apply,
    },
    SqlMigration {
        definition: MigrationDefinition {
            introduced: MigrationDate {
                year: 2026,
                month: 11,
                day: 11,
            },
            ..definition(
                "2026-11-11-strict-native-review",
                "Legacy native review markers on deleted agents",
            )
        },
        apply: m2026_11_11_strict_native_review::apply,
    },
    SqlMigration {
        definition: MigrationDefinition {
            introduced: MigrationDate {
                year: 2026,
                month: 11,
                day: 11,
            },
            ..definition(
                "2026-11-11-retired-agent-keys",
                "Retired agent root fields on supported historical rows",
            )
        },
        apply: m2026_11_11_retired_agent_keys::apply,
    },
    SqlMigration {
        definition: MigrationDefinition {
            introduced: MigrationDate {
                year: 2026,
                month: 11,
                day: 11,
            },
            ..definition(
                "2026-11-11-writer-receipt-indexes",
                "Window reads and receipt aliases without covering indexes",
            )
        },
        apply: m2026_11_11_writer_receipt_indexes::apply,
    },
    SqlMigration {
        definition: MigrationDefinition {
            introduced: MigrationDate {
                year: 2026,
                month: 11,
                day: 11,
            },
            ..definition(
                "2026-11-11-writer-query-indexes",
                "Writer lookups without item/turn and successful-rule indexes",
            )
        },
        apply: m2026_11_11_writer_query_indexes::apply,
    },
];
#[cfg(test)]
pub(crate) use m2026_11_11_writer_receipt_indexes::install as install_writer_receipt_indexes;

fn catalog_through_f1() -> impl Iterator<Item = MigrationDefinition> {
    CATALOG
        .iter()
        .map(SqlMigration::definition)
        .chain(std::iter::once(
            m2026_11_11_remove_account_throttle::REGISTRY_DEFINITION,
        ))
        .chain(std::iter::once(m2026_11_11_consolidate_f1::DEFINITION))
}

// Append new serving migrations here; the released F1 prefix remains frozen.
static POST_F1_CATALOG: &[SqlMigration] = &[
    SqlMigration {
        definition: MigrationDefinition {
            id: "2026-11-12-writer-window-indexes",
            introduced: MigrationDate {
                year: 2026,
                month: 11,
                day: 12,
            },
            target: MigrationTarget::Canvas,
            supported_source: "Consolidated F1 databases without bounded writer window indexes",
            remove_when: "Supported installations and backups all contain these writer indexes",
        },
        apply: m2026_11_12_writer_window_indexes::apply,
    },
    SqlMigration {
        definition: MigrationDefinition {
            id: "2026-11-12-changes-label-sequence",
            introduced: MigrationDate {
                year: 2026,
                month: 11,
                day: 12,
            },
            target: MigrationTarget::Canvas,
            supported_source: "Consolidated changes without a label and sequence range access path",
            remove_when: "Supported installations and backups all contain the label sequence index",
        },
        apply: m2026_11_12_changes_label_sequence::apply,
    },
    SqlMigration {
        definition: m2026_11_12_direct_durable_keys::DEFINITION,
        apply: m2026_11_12_direct_durable_keys::apply,
    },
    SqlMigration {
        definition: MigrationDefinition {
            id: "2026-11-12-event-publication-members",
            introduced: MigrationDate {
                year: 2026,
                month: 11,
                day: 12,
            },
            target: MigrationTarget::Canvas,
            supported_source: "Event retirement walks publications across collections and history",
            remove_when: "Supported installations and backups all contain the ordered collection member index",
        },
        apply: m2026_11_12_event_publication_members::apply,
    },
    SqlMigration {
        definition: m2026_11_12_scheduler_wake_indexes::DEFINITION,
        apply: m2026_11_12_scheduler_wake_indexes::apply,
    },
];
pub fn catalog() -> impl Iterator<Item = MigrationDefinition> {
    catalog_through_f1().chain(POST_F1_CATALOG.iter().map(SqlMigration::definition))
}

/// Position in the currently migrating source, measured in SQLite rowids.
#[derive(Clone, Debug, serde::Serialize)]
pub struct MigrationProgress {
    pub id: String,
    pub done: i64,
    pub total: i64,
}

fn too_old(target: &str, id: &str) -> StoreError {
    StoreError::SchemaMismatch(format!("state too old: missing/incomplete {target}/{id}"))
}
fn catalog_fingerprint(catalog: &[SqlMigration], target: MigrationTarget) -> String {
    let ids = RETIRED
        .iter()
        .filter(|(t, _, receipt)| *t == target && !receipt)
        .map(|(_, id, _)| *id)
        .chain(
            catalog
                .iter()
                .filter(|step| step.definition.target == target)
                .map(|step| step.definition.id),
        )
        .collect::<Vec<_>>();
    crate::sha256_hex(ids.join("\0").as_bytes())
}
/// Canvas catalog marker key; its value is catalog fingerprint plus SQLite schema generation.
pub(crate) const MIGRATION_CATALOG_KEY: &str = "migration_catalog";

fn canvas_catalog_prefix_fingerprint(post_f1_end: usize) -> String {
    let ids = RETIRED
        .iter()
        .filter(|(target, _, receipt)| *target == MigrationTarget::Canvas && !receipt)
        .map(|(_, id, _)| *id)
        .chain(
            catalog_through_f1()
                .chain(
                    POST_F1_CATALOG[..post_f1_end]
                        .iter()
                        .map(SqlMigration::definition),
                )
                .filter(|definition| definition.target == MigrationTarget::Canvas)
                .map(|definition| definition.id),
        )
        .collect::<Vec<_>>();
    crate::sha256_hex(ids.join("\0").as_bytes())
}
fn write_current_canvas_catalog(tx: &Transaction<'_>) -> StoreResult<()> {
    write_canvas_catalog_at(tx, POST_F1_CATALOG.len())
}
fn write_canvas_catalog_at(tx: &Transaction<'_>, post_f1_end: usize) -> StoreResult<()> {
    crate::clock_schema::SyncEntityMeta::put(
        tx,
        MIGRATION_CATALOG_KEY,
        &catalog_marker(tx, &canvas_catalog_prefix_fingerprint(post_f1_end))?,
    )?;
    Ok(())
}

fn schema_version(db: &Connection) -> rusqlite::Result<i64> {
    db.pragma_query_value(None, "schema_version", |row| row.get(0))
}
fn catalog_marker(db: &Connection, fingerprint: &str) -> rusqlite::Result<String> {
    let version = schema_version(db)?;
    Ok(format!("{fingerprint}:{version}"))
}
fn receipt_finish_name(id: &str) -> String {
    let finish = format!("{id}-finish");
    format!("receipts:{}:{finish}:", finish.len())
}
// These retired SQL steps wrote independent checkpoints in addition to the catalog.
const INDIVIDUAL: &[&str] = &[
    "2026-10-31-work-records",
    "2026-10-30-task-records",
    "2026-11-01-room-lead-projection",
    "2026-11-01-radio-controls-projection",
    PRE_LEDGER_REVIEW_MODEL,
];
// ff02c9d ran this step before its ledger existed; f832d09 kept the same catalog hash.
const PRE_LEDGER_REVIEW_MODEL: &str = "2026-11-07-explicit-review-model";
fn checkpoints() -> impl Iterator<Item = String> {
    RETIRED
        .iter()
        .filter(|(_, _, receipt)| *receipt)
        .map(|(_, id, _)| receipt_finish_name(id))
        .chain(INDIVIDUAL.iter().map(|id| (*id).to_owned()))
}
fn current_canvas_authority(db: &Connection) -> StoreResult<bool> {
    Ok(!crate::peer_update_settings::canvas_read(db)
        .table_exists::<crate::settings_schema::PayloadMigrations>()?)
}
fn current_canvas_catalog_end(db: &Connection) -> StoreResult<usize> {
    let marker = crate::clock_schema::SyncEntityMeta::value(db, MIGRATION_CATALOG_KEY)?
        .ok_or_else(|| too_old("canvas", MIGRATION_CATALOG_KEY))?;
    let current = schema_version(db)?;
    let Some((fingerprint, version)) = marker.split_once(':') else {
        return Err(too_old("canvas", MIGRATION_CATALOG_KEY));
    };
    if version.is_empty()
        || !version.bytes().all(|b| b.is_ascii_digit())
        || !version
            .parse::<i64>()
            .is_ok_and(|version| version > 0 && version <= current)
    {
        return Err(too_old("canvas", MIGRATION_CATALOG_KEY));
    }
    (0..=POST_F1_CATALOG.len())
        .find(|end| fingerprint == canvas_catalog_prefix_fingerprint(*end))
        .ok_or_else(|| too_old("canvas", MIGRATION_CATALOG_KEY))
}
fn verify_current_canvas_catalog(db: &Connection) -> StoreResult<()> {
    current_canvas_catalog_end(db).map(|_| ())
}
fn apply_post_f1(tx: &Transaction<'_>, admitted_end: usize) -> StoreResult<()> {
    for step in &POST_F1_CATALOG[admitted_end..] {
        (step.apply)(tx)?;
    }
    Ok(())
}
fn upgrade_current_canvas(db: &mut Connection) -> StoreResult<()> {
    let admitted_end = current_canvas_catalog_end(db)?;
    let tx = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
    apply_post_f1(&tx, admitted_end)?;
    prepare_current_canvas(&tx)?;
    write_current_canvas_catalog(&tx)?;
    tx.commit()?;
    Ok(())
}

fn verify_catalog(
    db: &Connection,
    table: &str,
    target: MigrationTarget,
    name: &str,
) -> StoreResult<()> {
    let read = || -> rusqlite::Result<Option<String>> {
        db.query_row(
            &format!("SELECT value FROM {table} WHERE key='migration_catalog'"),
            [],
            |r| r.get(0),
        )
        .optional()
    };
    let marker = read()
        .map_err(|_| too_old(name, "migration_catalog"))?
        .ok_or_else(|| too_old(name, "migration_catalog"))?;
    let current = schema_version(db)?;
    let valid = marker
        .split_once(':')
        .is_some_and(|(fingerprint, version)| {
            version.bytes().all(|byte| byte.is_ascii_digit())
                && version
                    .parse::<i64>()
                    .is_ok_and(|version| version > 0 && version <= current)
                && (0..=CATALOG.len())
                    .any(|end| fingerprint == catalog_fingerprint(&CATALOG[..end], target))
        });
    if !valid {
        return Err(too_old(name, "migration_catalog"));
    }
    Ok(())
}
/// Read only, under the state lock, before claiming ownership or bootstrap.
pub(crate) fn verify_baseline(root: &Path) -> StoreResult<()> {
    let canvas = crate::open_database(
        &root.join("canvas.sqlite3"),
        OpenFlags::SQLITE_OPEN_READ_ONLY,
    )?;
    let analytics = crate::open_database(
        &root.join("analytics.sqlite3"),
        OpenFlags::SQLITE_OPEN_READ_ONLY,
    )?;
    if current_canvas_authority(&canvas)? {
        verify_current_canvas_catalog(&canvas)?;
    } else {
        verify_catalog(
            &canvas,
            "sync_entity_meta",
            MigrationTarget::Canvas,
            "canvas",
        )?;
    }
    verify_catalog(
        &analytics,
        "analytics_meta",
        MigrationTarget::Analytics,
        "analytics",
    )?;
    for id in checkpoints() {
        let state = crate::migration_progress::completion(
            &crate::peer_update_settings::canvas_read(&canvas),
            &id,
        )
        .map_err(|_| too_old("canvas", &id))?;
        // The verified catalog covers this pre-ledger transition. Its missing row
        // is valid; an explicitly incomplete row still refuses, as apply_once did.
        if state != Some((1, "complete".into()))
            && !(id == PRE_LEDGER_REVIEW_MODEL && state.is_none())
        {
            return Err(too_old("canvas", &id));
        }
    }
    let provider = analytics
        .query_row(
            "SELECT value FROM analytics_meta WHERE key=?1",
            [ANALYTICS_PROVIDER_FORMAT_KEY],
            |r| r.get::<_, String>(0),
        )
        .optional()?;
    if provider.as_deref() != Some(PROVIDER_IDENTITY_FORMAT) {
        return Err(too_old("analytics", ANALYTICS_PROVIDER_FORMAT_KEY));
    }
    let supervisor = root.join("supervisor.sqlite3");
    if supervisor.exists() {
        let db = crate::open_database(&supervisor, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
        crate::supervisor_journal::validate_schema(&db)
            .map_err(|_| too_old("supervisor", "2026-10-29-supervisor-written"))?;
    }
    let registry = root.join("accounts/registry.json");
    match std::fs::read(registry) {
        Ok(bytes) => {
            let migrated = m2026_11_11_remove_account_throttle::registry_destination(&bytes)?;
            let bytes = migrated.as_deref().unwrap_or(&bytes);
            let (_, _, registry) = crate::account_registry::decode_registry_typed(bytes)
                .map_err(|_| too_old("account-registry", "2026-10-26-account-registry"))?;
            if registry.accounts.contains_key("claude-local") {
                return Err(too_old(
                    "account-registry",
                    "2026-10-31-claude-home-account",
                ));
            }
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error.into()),
    }
    crate::schema::verify_core(
        &root.join("canvas.sqlite3"),
        &root.join("analytics.sqlite3"),
    )?;
    crate::verify_observed_tables_are_rowid(&canvas)?;
    if let Some(finding) = crate::bootstrap::inspect_sync_tables(&canvas)?
        .into_iter()
        .next()
    {
        return Err(StoreError::SchemaObjectMismatch {
            database: "canvas.sqlite3".into(),
            object: finding.object,
            reason: finding.reason,
        });
    }
    Ok(())
}

/// Fresh stores seed retired evidence only after the current catalog was installed.
pub(crate) fn seed_baseline(tx: &Transaction<'_>, target: MigrationTarget) -> StoreResult<()> {
    if target == MigrationTarget::Canvas {
        use crate::metadata_schema::{AccessSettings, SyncIdentity};
        use crate::metadata_values::StoredBoolean;
        use crate::peer_update_settings::canvas_tx;
        use studio_db_api::migration::encode_key;
        crate::clock_schema::seed_fresh_clock_rows(tx)?;
        let mut typed = canvas_tx(tx);
        typed.insert_new(&AccessSettings {
            identity: encode_key(&(1_i64,)),
            source_rowid: 1,
            integer_key: 1,
            auto_pair: StoredBoolean::Valid(true),
        })?;
        let text_key = Some(uuid::Uuid::new_v4().simple().to_string());
        typed.insert_new(&SyncIdentity {
            identity: crate::consolidation_codec::nullable_text_identity(&text_key, 1),
            source_rowid: 1,
            text_key,
        })?;
        crate::command_watermarks::CommandWatermarks::install_send_admissions(tx, 0)?;
        for id in checkpoints() {
            crate::migration_progress::save(
                &mut typed,
                crate::migration_progress::State {
                    name: id,
                    cursor: 0,
                    complete: 1,
                    status: "complete".into(),
                    updated: Some(0.0),
                    error: None,
                    cursor_key: None,
                    copy_source_rows: None,
                    copy_target_rows: None,
                },
            )?;
        }
        return write_current_canvas_catalog(tx);
    }
    write_catalog(tx, target, &[])
}
fn write_catalog(
    tx: &Transaction<'_>,
    target: MigrationTarget,
    catalog: &[SqlMigration],
) -> StoreResult<()> {
    let table = match target {
        MigrationTarget::Canvas => "sync_entity_meta",
        MigrationTarget::Analytics => "analytics_meta",
        MigrationTarget::AccountRegistry | MigrationTarget::Supervisor => return Ok(()),
    };
    tx.execute(&format!("INSERT INTO {table}(key,value) VALUES('migration_catalog',?1) ON CONFLICT(key) DO UPDATE SET value=excluded.value"), [catalog_marker(tx, &catalog_fingerprint(catalog, target))?])?;
    Ok(())
}
fn rolled_back(
    tx: Transaction<'_>,
    id: &'static str,
    target: MigrationTarget,
    source: StoreError,
) -> StoreError {
    let result = tx.rollback();
    StoreError::Migration {
        id,
        target,
        phase: if result.is_ok() {
            MigrationPhase::Apply
        } else {
            MigrationPhase::Rollback
        },
        rollback: if result.is_ok() {
            MigrationRollbackOutcome::RolledBack
        } else {
            MigrationRollbackOutcome::Unknown
        },
        source: Box::new(source),
        rollback_error: result.err(),
    }
}
pub(crate) fn prepare_current_canvas(tx: &Transaction<'_>) -> StoreResult<()> {
    crate::schema::prepare_peer_update_settings(tx)?;
    crate::schema::prepare_item_turn_scope(tx)?;
    crate::bootstrap::prepare_runtime_event_indexes(tx)?;
    crate::feature_schema::prepare(tx)?;
    crate::MonitorsRepository::prepare_indexes(tx)?;
    crate::AgentIndexedReads::prepare_index(tx)?;
    crate::schema::prepare_receipt_owners(tx)?;
    Ok(())
}
/// Run under runtime.lock; each target commits independently and can restart.
pub(crate) fn run(
    root: &Path,
    _progress: Option<&(dyn Fn(MigrationProgress) + Send + Sync)>,
) -> StoreResult<()> {
    m2026_11_11_remove_account_throttle::registry(root)?;
    let mut analytics = crate::open_database(
        &root.join("analytics.sqlite3"),
        OpenFlags::SQLITE_OPEN_READ_WRITE,
    )?;
    crate::sqlite_durability::configure_writer(&analytics)?;
    let tx = analytics.transaction_with_behavior(TransactionBehavior::Immediate)?;
    crate::schema::prepare_publication_overflow_receipts(&tx)?;
    crate::schema::ensure_analytics_limits_index(&tx)?;
    crate::fresh_state::prepare_analytics_revision_schema(&tx)?;
    crate::repositories::cost_aggregate::install(&tx)?;
    for step in CATALOG
        .iter()
        .filter(|step| step.definition.target == MigrationTarget::Analytics)
    {
        if let Err(source) = (step.apply)(&tx) {
            return Err(rolled_back(
                tx,
                step.definition.id,
                MigrationTarget::Analytics,
                source,
            ));
        }
    }
    write_catalog(&tx, MigrationTarget::Analytics, CATALOG)?;
    tx.commit()?;
    let mut canvas = crate::open_database(
        &root.join("canvas.sqlite3"),
        OpenFlags::SQLITE_OPEN_READ_WRITE,
    )?;
    crate::schema::register_python_trigger_functions(&canvas)?;
    crate::sqlite_durability::configure_writer(&canvas)?;
    if current_canvas_authority(&canvas)? {
        return upgrade_current_canvas(&mut canvas);
    }
    let tx = canvas.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let marker: Option<String> = tx
        .query_row(
            "SELECT value FROM sync_entity_meta WHERE key='migration_catalog'",
            [],
            |r| r.get(0),
        )
        .optional()?;
    let admitted_end = marker
        .as_deref()
        .and_then(|value| value.split_once(':'))
        .and_then(|(fingerprint, _)| {
            (0..=CATALOG.len()).find(|end| {
                fingerprint == catalog_fingerprint(&CATALOG[..*end], MigrationTarget::Canvas)
            })
        })
        .ok_or_else(|| too_old("canvas", MIGRATION_CATALOG_KEY))?;
    for step in CATALOG[admitted_end..]
        .iter()
        .filter(|step| step.definition.target == MigrationTarget::Canvas)
    {
        if let Err(source) = (step.apply)(&tx) {
            return Err(rolled_back(
                tx,
                step.definition.id,
                MigrationTarget::Canvas,
                source,
            ));
        }
    }
    // Lazy legacy owners are materialized before the bounded copy begins. No target
    // serving caller runs until the final authority transaction has committed.
    for descriptor in m2026_11_11_consolidate_f1::legacy_source_descriptors() {
        descriptor.install_if_absent(&tx)?;
    }
    crate::clock_schema::prepare_durable_scope(&tx)?;
    write_catalog(&tx, MigrationTarget::Canvas, CATALOG)?;
    tx.commit()?;
    m2026_11_11_consolidate_f1::run(&mut canvas)?;
    let tx = canvas.transaction_with_behavior(TransactionBehavior::Immediate)?;
    apply_post_f1(&tx, 0)?;
    prepare_current_canvas(&tx)?;
    write_current_canvas_catalog(&tx)?;
    tx.commit()?;
    Ok(())
}
pub(crate) fn supervisor(db: &mut Connection) -> StoreResult<()> {
    let exists: bool = db.query_row("SELECT EXISTS(SELECT 1 FROM sqlite_schema WHERE type='table' AND name NOT LIKE 'sqlite_%')", [], |r| r.get(0))?;
    if exists {
        return crate::supervisor_journal::validate_schema(db);
    }
    let tx = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
    crate::supervisor_schema::install(&tx)?;
    tx.commit()?;
    Ok(())
}

#[cfg(any(test, feature = "test-support"))]
pub(crate) fn retired_agent_root_keys() -> &'static [&'static str] {
    m2026_11_11_retired_agent_keys::RETIRED
}

#[cfg(any(test, feature = "test-support"))]
pub(crate) fn normalize_retired_agent_record(id: &str, raw: &str) -> crate::StoreResult<String> {
    Ok(m2026_11_11_retired_agent_keys::destination(id, raw)?.unwrap_or_else(|| raw.to_owned()))
}
#[cfg(any(test, feature = "test-support"))]
pub(crate) fn retire_agent_fixture_keys(tx: &Transaction<'_>) -> crate::StoreResult<()> {
    m2026_11_11_retired_agent_keys::apply(tx)
}

/// Canvas catalog fingerprints written by released servers. The catalog is
/// append-only: each one must stay a prefix fingerprint so those stores open.
/// Released F1 Canvas catalog SHA-256 fingerprint (abc6f249).
#[cfg(test)]
const RELEASED_F1_CANVAS_FINGERPRINT: &str =
    "18cac9181eff0e01c7010b22e772e9e782d19523076bb3fdfea7a5bc00394f8f";
#[cfg(test)]
const RELEASED_CANVAS_CATALOGS: &[&str] = &[
    RELEASED_F1_CANVAS_FINGERPRINT,
    "aca2e15f31243e831f133e58486adb630a4dd0f929a4e7c5881dcf29d53f2a53",
    "cc2c41ac023a533092d0d96648db3996bf606f5761d4a653b946049160cf1a89",
    "561c94aa6bffe48d6de89805793b296560ace24b89a3b914d38635215eae5f53",
    "e07877dd5a4eef610f66b99336716fb9e32515094d81713b45d30ec61d09cd63",
    "85c8d18a41f9de456bfa310c7b4f119d2b78045f6a2dabb8fb2634c7b3c29bc9",
    "44933893bb8ff0173863fdf28513266c0d42dc3b07d980ecc53a51a598f390ae",
];

#[cfg(test)]
mod released_catalog_tests {
    #[test]
    fn wake_eligibility_indexes_upgrade_once_and_rollback_after_publication_members() {
        let names = [
            "runtime_agent_wake_recovery",
            "chats_wake_queued",
            "turns_wake_submitted",
        ];
        let root = tempfile::tempdir().unwrap();
        let mut db =
            crate::fresh_state::canvas_before_indexes(&root.path().join("canvas.sqlite3"), &names)
                .unwrap();
        let position = super::POST_F1_CATALOG
            .iter()
            .position(|step| {
                step.definition.id == super::m2026_11_12_scheduler_wake_indexes::DEFINITION.id
            })
            .unwrap();
        assert_eq!(
            super::POST_F1_CATALOG[position - 1].definition.id,
            "2026-11-12-event-publication-members"
        );
        assert!(super::canvas_catalog_prefix_fingerprint(position).starts_with("7d472d70"));
        let tx = db.transaction().unwrap();
        super::write_canvas_catalog_at(&tx, position).unwrap();
        tx.commit().unwrap();
        let version = super::schema_version(&db).unwrap();
        let tx = db.transaction().unwrap();
        super::m2026_11_12_scheduler_wake_indexes::apply(&tx).unwrap();
        tx.rollback().unwrap();
        assert_eq!(super::schema_version(&db).unwrap(), version);
        for name in names {
            assert!(
                !studio_db_api::catalog::export(&db)
                    .unwrap()
                    .iter()
                    .any(|entry| entry.name == name)
            );
        }
        super::upgrade_current_canvas(&mut db).unwrap();
        let version = super::schema_version(&db).unwrap();
        for name in names {
            assert!(
                studio_db_api::catalog::export(&db)
                    .unwrap()
                    .iter()
                    .any(|entry| entry.name == name)
            );
        }
        super::upgrade_current_canvas(&mut db).unwrap();
        assert_eq!(super::schema_version(&db).unwrap(), version);
    }

    #[test]
    fn event_publication_member_index_upgrades_once_without_changing_triggers() {
        let root = tempfile::tempdir().unwrap();
        let mut db = crate::fresh_state::canvas_before_indexes(
            &root.path().join("canvas.sqlite3"),
            &["publications_entity_collection_members"],
        )
        .unwrap();
        let position = super::POST_F1_CATALOG
            .iter()
            .position(|step| step.definition.id == "2026-11-12-event-publication-members")
            .unwrap();
        let tx = db.transaction().unwrap();
        super::write_canvas_catalog_at(&tx, position).unwrap();
        tx.commit().unwrap();
        let triggers = |db: &rusqlite::Connection| {
            studio_db_api::catalog::export(db)
                .unwrap()
                .into_iter()
                .filter(|entry| entry.r#type == "trigger")
                .map(|entry| (entry.name, entry.sql))
                .collect::<Vec<_>>()
        };
        let before = triggers(&db);
        super::upgrade_current_canvas(&mut db).unwrap();
        assert_eq!(triggers(&db), before);
        let index = studio_db_api::catalog::export(&db)
            .unwrap()
            .into_iter()
            .find(|entry| entry.name == "publications_entity_collection_members")
            .unwrap();
        assert_eq!(
            index.sql,
            crate::queue_schema::index_sql::<crate::publication_schema::Publications>(
                "publications_entity_collection_members"
            )
        );
        let version = super::schema_version(&db).unwrap();
        super::upgrade_current_canvas(&mut db).unwrap();
        assert_eq!(super::schema_version(&db).unwrap(), version);
    }

    #[test]
    fn consolidated_f1_prefix_receives_writer_indexes_once() {
        use studio_runtime_schema::studio_errors::MigrationTarget;
        let released = super::RELEASED_F1_CANVAS_FINGERPRINT;
        assert_eq!(super::canvas_catalog_prefix_fingerprint(0), released);
        let root = tempfile::tempdir().unwrap();
        let names = [
            "runtime_agent_limit_recovery",
            "runtime_execution_run_terminal",
            "runtime_monitor_window_status",
        ];
        let mut db =
            crate::fresh_state::canvas_before_indexes(&root.path().join("canvas.sqlite3"), &names)
                .unwrap();
        let tx = db.transaction().unwrap();
        super::write_canvas_catalog_at(&tx, 0).unwrap();
        tx.commit().unwrap();
        assert!(super::current_canvas_authority(&db).unwrap());
        assert_eq!(super::current_canvas_catalog_end(&db).unwrap(), 0);
        let before = studio_db_api::catalog::export(&db).unwrap();
        assert!(
            names
                .iter()
                .all(|name| before.iter().all(|entry| entry.name != *name))
        );
        super::upgrade_current_canvas(&mut db).unwrap();
        let after = studio_db_api::catalog::export(&db).unwrap();
        assert!(
            names
                .iter()
                .all(|name| after.iter().any(|entry| entry.name == *name))
        );
        assert_eq!(
            super::current_canvas_catalog_end(&db).unwrap(),
            super::POST_F1_CATALOG.len()
        );
        let version = super::schema_version(&db).unwrap();
        let marker =
            crate::clock_schema::SyncEntityMeta::value(&db, super::MIGRATION_CATALOG_KEY).unwrap();
        super::upgrade_current_canvas(&mut db).unwrap();
        assert_eq!(super::schema_version(&db).unwrap(), version);
        assert_eq!(
            crate::clock_schema::SyncEntityMeta::value(&db, super::MIGRATION_CATALOG_KEY).unwrap(),
            marker
        );
        assert!(super::catalog_fingerprint(super::CATALOG, MigrationTarget::Canvas) != released);
    }

    #[test]
    fn released_canvas_prefix_receives_label_sequence_index_once() {
        let root = tempfile::tempdir().unwrap();
        let name = "changes_durable_label_sequence";
        let mut db =
            crate::fresh_state::canvas_before_indexes(&root.path().join("canvas.sqlite3"), &[name])
                .unwrap();
        let tx = db.transaction().unwrap();
        super::write_canvas_catalog_at(&tx, 1).unwrap();
        tx.commit().unwrap();
        super::upgrade_current_canvas(&mut db).unwrap();
        assert!(
            studio_db_api::catalog::export(&db)
                .unwrap()
                .iter()
                .any(|entry| entry.name == name)
        );
        let marker =
            crate::clock_schema::SyncEntityMeta::value(&db, super::MIGRATION_CATALOG_KEY).unwrap();
        super::upgrade_current_canvas(&mut db).unwrap();
        assert_eq!(
            crate::clock_schema::SyncEntityMeta::value(&db, super::MIGRATION_CATALOG_KEY).unwrap(),
            marker
        );
    }

    #[test]
    fn released_canvas_catalogs_remain_prefixes() {
        for released in super::RELEASED_CANVAS_CATALOGS {
            assert!(
                (0..=super::CATALOG.len()).any(|end| super::catalog_fingerprint(
                    &super::CATALOG[..end],
                    super::MigrationTarget::Canvas
                ) == *released)
                    || (0..=super::POST_F1_CATALOG.len())
                        .any(|end| super::canvas_catalog_prefix_fingerprint(end) == *released),
                "released catalog {released} is no longer a prefix"
            );
        }
    }
}
