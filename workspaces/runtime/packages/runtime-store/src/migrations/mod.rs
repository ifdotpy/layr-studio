//! Supported baseline verification and the remaining dated Canvas transitions.
//! The caller holds runtime.lock; baseline inspection precedes every state write.
mod baseline_manifest;
mod m2026_11_09_immutable_entity_scopes;
pub(crate) mod m2026_11_09_online_keys;
mod m2026_11_09_repair_invalid_entity_scopes;
mod m2026_11_09_retired_objects;
mod m2026_11_11_remove_account_throttle;
mod m2026_11_11_retired_agent_keys;
mod m2026_11_11_strict_native_review;
mod m2026_11_11_writer_query_indexes;
mod m2026_11_11_writer_receipt_indexes;

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

pub fn catalog() -> impl Iterator<Item = MigrationDefinition> {
    CATALOG
        .iter()
        .map(SqlMigration::definition)
        .chain(std::iter::once(
            m2026_11_11_remove_account_throttle::REGISTRY_DEFINITION,
        ))
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
fn catalog_marker(db: &Connection, fingerprint: &str) -> rusqlite::Result<String> {
    let version: i64 = db.pragma_query_value(None, "schema_version", |r| r.get(0))?;
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
    let current: i64 = db.pragma_query_value(None, "schema_version", |r| r.get(0))?;
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
    verify_catalog(
        &canvas,
        "sync_entity_meta",
        MigrationTarget::Canvas,
        "canvas",
    )?;
    verify_catalog(
        &analytics,
        "analytics_meta",
        MigrationTarget::Analytics,
        "analytics",
    )?;
    for id in checkpoints() {
        let state = canvas
            .query_row(
                "SELECT complete,status FROM runtime_payload_migrations WHERE name=?1",
                [&id],
                |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)),
            )
            .optional()
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
        crate::command_watermarks::CommandWatermarks::install_send_admissions(tx, 0)?;
        for id in checkpoints() {
            tx.execute("INSERT INTO runtime_payload_migrations(name,cursor,complete,status,updated,cutover_ms) VALUES(?1,0,1,'complete',0,0)", [id])?;
        }
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
    crate::bootstrap::prepare_sync_schema(tx)?;
    crate::bootstrap::prepare_runtime_event_indexes(tx)?;
    crate::AgentRepository::prepare_change_journal(tx)?;
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
    let tx = canvas.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let marker: Option<String> = tx
        .query_row(
            "SELECT value FROM sync_entity_meta WHERE key='migration_catalog'",
            [],
            |r| r.get(0),
        )
        .optional()?;
    let current = catalog_marker(&tx, &catalog_fingerprint(CATALOG, MigrationTarget::Canvas))?;
    for step in CATALOG.iter().filter(|step| {
        step.definition.target == MigrationTarget::Canvas
            && marker.as_deref() != Some(current.as_str())
    }) {
        if let Err(source) = (step.apply)(&tx) {
            return Err(rolled_back(
                tx,
                step.definition.id,
                MigrationTarget::Canvas,
                source,
            ));
        }
    }
    prepare_current_canvas(&tx)?;
    write_catalog(&tx, MigrationTarget::Canvas, CATALOG)?;
    match tx.execute_batch("COMMIT") {
        Ok(()) => Ok(()),
        Err(error) if tx.is_autocommit() => Err(StoreError::Migration {
            id: "2026-11-09-retired-objects",
            target: MigrationTarget::Canvas,
            phase: MigrationPhase::Commit,
            rollback: MigrationRollbackOutcome::Unknown,
            source: Box::new(error.into()),
            rollback_error: None,
        }),
        Err(error) => Err(rolled_back(
            tx,
            "2026-11-09-retired-objects",
            MigrationTarget::Canvas,
            error.into(),
        )),
    }
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
#[cfg(test)]
const RELEASED_CANVAS_CATALOGS: &[&str] =
    &["561c94aa6bffe48d6de89805793b296560ace24b89a3b914d38635215eae5f53"];

#[cfg(test)]
mod released_catalog_tests {
    #[test]
    fn released_canvas_catalogs_remain_prefixes() {
        for released in super::RELEASED_CANVAS_CATALOGS {
            assert!(
                (0..=super::CATALOG.len()).any(|end| super::catalog_fingerprint(
                    &super::CATALOG[..end],
                    super::MigrationTarget::Canvas
                ) == *released),
                "released catalog {released} is no longer a prefix"
            );
        }
    }
}
