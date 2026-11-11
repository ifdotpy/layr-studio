//! One dated F1 family copy; target preparation does not switch writer authority.

use crate::clock_schema::SyncGeneration as GenerationSource;
use crate::metadata_schema::SyncGeneration as GenerationTarget;
use studio_db_api::rusqlite::types::Value;
use studio_db_api::{
    Table,
    migration::{CopyPlan, Evidence, encode_key},
};

pub(super) const DEFINITION: super::MigrationDefinition = super::MigrationDefinition {
    id: "2026-11-11-consolidate-f1",
    introduced: super::MigrationDate {
        year: 2026,
        month: 11,
        day: 11,
    },
    target: studio_runtime_schema::studio_errors::MigrationTarget::Canvas,
    supported_source: "The 24 retained metadata, change and publication source tables",
    remove_when: "Supported installations and backups no longer admit the 24-table source format",
};

/// Fixed stream identities for this dated family; all progress keys derive here.
const BASE_NAMES: [&str; 24] = [
    "f1_2026_11_11_client_command_watermarks",
    "f1_2026_11_11_durable_floor",
    "f1_2026_11_11_access_local_alias",
    "f1_2026_11_11_access_settings",
    "f1_2026_11_11_agent_record_generation",
    "f1_2026_11_11_federation_settings",
    "f1_2026_11_11_orchestration_settings",
    "f1_2026_11_11_payload_migrations",
    "f1_2026_11_11_scheduler_agent_change_state",
    "f1_2026_11_11_search_rollout",
    "f1_2026_11_11_sidebar_order",
    "f1_2026_11_11_stored_text_batches",
    "f1_2026_11_11_sync_entity_meta",
    "f1_2026_11_11_sync_generation",
    "f1_2026_11_11_sync_identity",
    "f1_2026_11_11_task_window_state",
    "f1_2026_11_11_agent_publication_state",
    "f1_2026_11_11_peer_update_settings",
    "f1_2026_11_11_online_migration_dirty",
    "f1_2026_11_11_durable_rows",
    "f1_2026_11_11_scheduler_agent_changes",
    "f1_2026_11_11_transcript_revisions",
    "f1_2026_11_11_sync_documents",
    "f1_2026_11_11_sync_entities",
];
const PROGRESS_NAMES: [&str; BASE_NAMES.len()] = [
    studio_db_api::static_sql!(crate::online_migration::PROGRESS_PREFIX, BASE_NAMES[0]),
    studio_db_api::static_sql!(crate::online_migration::PROGRESS_PREFIX, BASE_NAMES[1]),
    studio_db_api::static_sql!(crate::online_migration::PROGRESS_PREFIX, BASE_NAMES[2]),
    studio_db_api::static_sql!(crate::online_migration::PROGRESS_PREFIX, BASE_NAMES[3]),
    studio_db_api::static_sql!(crate::online_migration::PROGRESS_PREFIX, BASE_NAMES[4]),
    studio_db_api::static_sql!(crate::online_migration::PROGRESS_PREFIX, BASE_NAMES[5]),
    studio_db_api::static_sql!(crate::online_migration::PROGRESS_PREFIX, BASE_NAMES[6]),
    studio_db_api::static_sql!(crate::online_migration::PROGRESS_PREFIX, BASE_NAMES[7]),
    studio_db_api::static_sql!(crate::online_migration::PROGRESS_PREFIX, BASE_NAMES[8]),
    studio_db_api::static_sql!(crate::online_migration::PROGRESS_PREFIX, BASE_NAMES[9]),
    studio_db_api::static_sql!(crate::online_migration::PROGRESS_PREFIX, BASE_NAMES[10]),
    studio_db_api::static_sql!(crate::online_migration::PROGRESS_PREFIX, BASE_NAMES[11]),
    studio_db_api::static_sql!(crate::online_migration::PROGRESS_PREFIX, BASE_NAMES[12]),
    studio_db_api::static_sql!(crate::online_migration::PROGRESS_PREFIX, BASE_NAMES[13]),
    studio_db_api::static_sql!(crate::online_migration::PROGRESS_PREFIX, BASE_NAMES[14]),
    studio_db_api::static_sql!(crate::online_migration::PROGRESS_PREFIX, BASE_NAMES[15]),
    studio_db_api::static_sql!(crate::online_migration::PROGRESS_PREFIX, BASE_NAMES[16]),
    studio_db_api::static_sql!(crate::online_migration::PROGRESS_PREFIX, BASE_NAMES[17]),
    studio_db_api::static_sql!(crate::online_migration::PROGRESS_PREFIX, BASE_NAMES[18]),
    studio_db_api::static_sql!(crate::online_migration::PROGRESS_PREFIX, BASE_NAMES[19]),
    studio_db_api::static_sql!(crate::online_migration::PROGRESS_PREFIX, BASE_NAMES[20]),
    studio_db_api::static_sql!(crate::online_migration::PROGRESS_PREFIX, BASE_NAMES[21]),
    studio_db_api::static_sql!(crate::online_migration::PROGRESS_PREFIX, BASE_NAMES[22]),
    studio_db_api::static_sql!(crate::online_migration::PROGRESS_PREFIX, BASE_NAMES[23]),
];
const EXCLUDED_NAMES_SQL: &str = studio_db_api::static_sql!(
    "'",
    PROGRESS_NAMES[0],
    "','",
    PROGRESS_NAMES[1],
    "','",
    PROGRESS_NAMES[2],
    "','",
    PROGRESS_NAMES[3],
    "','",
    PROGRESS_NAMES[4],
    "','",
    PROGRESS_NAMES[5],
    "','",
    PROGRESS_NAMES[6],
    "','",
    PROGRESS_NAMES[7],
    "','",
    PROGRESS_NAMES[8],
    "','",
    PROGRESS_NAMES[9],
    "','",
    PROGRESS_NAMES[10],
    "','",
    PROGRESS_NAMES[11],
    "','",
    PROGRESS_NAMES[12],
    "','",
    PROGRESS_NAMES[13],
    "','",
    PROGRESS_NAMES[14],
    "','",
    PROGRESS_NAMES[15],
    "','",
    PROGRESS_NAMES[16],
    "','",
    PROGRESS_NAMES[17],
    "','",
    PROGRESS_NAMES[18],
    "','",
    PROGRESS_NAMES[19],
    "','",
    PROGRESS_NAMES[20],
    "','",
    PROGRESS_NAMES[21],
    "','",
    PROGRESS_NAMES[22],
    "','",
    PROGRESS_NAMES[23],
    "'",
);

/// Copy batches run before target-only callers are exposed. The returned durations
/// are total copy time and the largest writer transaction, including its commit.
pub(super) fn run(connection: &mut studio_db_api::rusqlite::Connection) -> crate::StoreResult<()> {
    if !crate::peer_update_settings::canvas_read(connection)
        .table_exists::<crate::settings_schema::PayloadMigrations>()?
    {
        return Ok(());
    }
    copy_sources(connection)?;
    verify_sources(connection)?;
    let transaction = connection
        .transaction_with_behavior(studio_db_api::rusqlite::TransactionBehavior::Immediate)?;
    validate_ready(&crate::peer_update_settings::canvas_read(&transaction))?;
    finish_authority(&transaction)?;
    transaction.commit()?;
    Ok(())
}

pub(super) fn copy_sources(
    connection: &mut studio_db_api::rusqlite::Connection,
) -> crate::StoreResult<(std::time::Duration, std::time::Duration)> {
    use crate::online_migration::{OnlineMigration, Step, SystemClock, begin, step};
    use std::time::{Duration, Instant};
    fn drive<M: OnlineMigration>(
        connection: &mut studio_db_api::rusqlite::Connection,
        migration: &M,
        largest: &mut Duration,
    ) -> crate::StoreResult<()> {
        let plan_started = Instant::now();
        let started = Instant::now();
        let transaction = connection
            .transaction_with_behavior(studio_db_api::rusqlite::TransactionBehavior::Immediate)?;
        begin(&transaction, migration)?;
        transaction.commit()?;
        let mut plan_largest = started.elapsed();
        loop {
            let started = Instant::now();
            let transaction = connection.transaction_with_behavior(
                studio_db_api::rusqlite::TransactionBehavior::Immediate,
            )?;
            let outcome = step(&transaction, migration, &SystemClock::new())?;
            transaction.commit()?;
            plan_largest = plan_largest.max(started.elapsed());
            if matches!(outcome, Step::Ready | Step::Complete) {
                *largest = (*largest).max(plan_largest);
                let name = format!(
                    "{}{}",
                    crate::online_migration::PROGRESS_PREFIX,
                    migration.name()
                );
                let state = crate::migration_progress::load(
                    &crate::peer_update_settings::canvas_read(connection),
                    &name,
                )?
                .ok_or(crate::StoreError::RecordContract(
                    "copy progress disappeared",
                ))?;
                eprintln!(
                    "F1_PLAN name={} wall_seconds={} source_rows={} copied_rows={} max_transaction_seconds={}",
                    migration.name(),
                    plan_started.elapsed().as_secs_f64(),
                    state.copy_source_rows.unwrap_or(0),
                    state.copy_target_rows.unwrap_or(0),
                    plan_largest.as_secs_f64()
                );
                return Ok(());
            }
        }
    }
    let started = Instant::now();
    let mut largest = Duration::ZERO;
    drive(
        connection,
        &FamilyCopy::new(watermarks_plan()),
        &mut largest,
    )?;
    drive(connection, &FamilyCopy::new(floor_plan()), &mut largest)?;
    drive(connection, &FamilyCopy::new(alias_plan()), &mut largest)?;
    drive(connection, &FamilyCopy::new(access_plan()), &mut largest)?;
    drive(
        connection,
        &FamilyCopy::new(agent_generation_plan()),
        &mut largest,
    )?;
    drive(
        connection,
        &FamilyCopy::new(federation_plan()),
        &mut largest,
    )?;
    drive(
        connection,
        &FamilyCopy::new(orchestration_plan()),
        &mut largest,
    )?;
    drive(
        connection,
        &FamilyCopy::new(scheduler_state_plan()),
        &mut largest,
    )?;
    drive(connection, &FamilyCopy::new(search_plan()), &mut largest)?;
    drive(connection, &FamilyCopy::new(sidebar_plan()), &mut largest)?;
    drive(connection, &FamilyCopy::new(batches_plan()), &mut largest)?;
    drive(connection, &FamilyCopy::new(sync_meta_plan()), &mut largest)?;
    drive(
        connection,
        &FamilyCopy::new(generation_plan()),
        &mut largest,
    )?;
    drive(
        connection,
        &FamilyCopy::new(sync_identity_plan()),
        &mut largest,
    )?;
    drive(
        connection,
        &FamilyCopy::new(task_window_plan()),
        &mut largest,
    )?;
    drive(
        connection,
        &FamilyCopy::new(publication_state_plan()),
        &mut largest,
    )?;
    drive(
        connection,
        &FamilyCopy::new(peer_settings_plan()),
        &mut largest,
    )?;
    drive(connection, &FamilyCopy::new(durable_plan()), &mut largest)?;
    drive(
        connection,
        &FamilyCopy::new(scheduler_changes_plan()),
        &mut largest,
    )?;
    drive(connection, &FamilyCopy::new(revisions_plan()), &mut largest)?;
    drive(connection, &documents_copy(), &mut largest)?;
    drive(connection, &entities_copy(), &mut largest)?;
    // Progress and dirty evidence are last and exclude only our closed family set.
    drive(connection, &ledger_copy(), &mut largest)?;
    drive(connection, &dirty_copy(), &mut largest)?;
    Ok((started.elapsed(), largest))
}

/// Frozen source owners for this dated transition. Active registration must not
/// retain these tables after authority switches; migration fixtures use the same DDL.
pub(super) fn legacy_source_descriptors() -> [studio_db_api::Descriptor; 24] {
    use crate::{clock_schema as clock, settings_schema as settings};
    [
        settings::ClientCommandWatermarks::descriptor(),
        clock::DurableFloor::descriptor(),
        settings::AccessLocalAlias::descriptor(),
        settings::AccessSettings::descriptor(),
        clock::AgentRecordGeneration::descriptor(),
        settings::FederationSettings::descriptor(),
        settings::OrchestrationSettings::descriptor(),
        settings::PayloadMigrations::descriptor(),
        clock::SchedulerAgentChangeState::descriptor(),
        settings::SearchRollout::descriptor(),
        settings::SidebarOrder::descriptor(),
        settings::StoredTextBatches::descriptor(),
        clock::SyncEntityMeta::descriptor(),
        clock::SyncGeneration::descriptor(),
        clock::SyncIdentity::descriptor(),
        clock::TaskWindowState::descriptor(),
        clock::AgentPublicationState::descriptor(),
        crate::peer_update_settings::PeerUpdateSettings::descriptor(),
        settings::OnlineMigrationDirty::descriptor(),
        clock::DurableRows::descriptor(),
        clock::SchedulerAgentChanges::descriptor(),
        crate::context_settings::TranscriptRevisionsStorageRow::descriptor(),
        clock::SyncDocuments::descriptor(),
        clock::SyncEntities::descriptor(),
    ]
}

fn validate_ready(
    read: &studio_db_api::Read<'_, crate::peer_update_settings::Canvas>,
) -> crate::StoreResult<()> {
    for name in PROGRESS_NAMES {
        let state = crate::migration_progress::load(read, name)?.ok_or(
            crate::StoreError::RecordContract("F1 progress missing before authority"),
        )?;
        if state.complete != 0
            || state.status != "running"
            || state.error.is_some()
            || state.copy_source_rows.is_none()
            || state.copy_source_rows != state.copy_target_rows
            || !crate::migration_progress::dirty(read, name, 1)?.is_empty()
        {
            return Err(crate::StoreError::RecordContract(
                "F1 progress not ready for authority",
            ));
        }
    }
    Ok(())
}

/// Final bounded ledger patch; called only after all family source writers stop.
fn retire_family_progress(
    tx: &mut studio_db_api::Tx<'_, crate::peer_update_settings::Canvas>,
) -> crate::StoreResult<()> {
    // This vector is bounded by the closed family, never by source table size.
    // Read and validate everything before making the first authority write.
    validate_ready(&tx.read())?;
    let mut rows = Vec::with_capacity(PROGRESS_NAMES.len());
    let plan = ledger_plan();
    for name in PROGRESS_NAMES {
        let source = tx
            .read()
            .named(
                &LedgerLookup::QUERY,
                &LedgerLookup {
                    name: Some(name.to_owned()),
                    source_rowid: 0,
                },
            )?
            .ok_or(crate::StoreError::RecordContract(
                "F1 source ledger disappeared",
            ))?;
        let mut target = (plan.transform)(NullableCursor(source))?;
        target.migration_complete = crate::metadata_values::StoredBoolean::from_stored(1);
        target.migration_status = "complete".parse().expect("fixed migration status");
        rows.push(target);
    }
    for row in rows {
        tx.partition::<crate::metadata_schema::PayloadMigrations>()
            .put(&row)?;
    }
    // All unrelated progress and dirty rows were copied and independently verified.
    // Dropping both sources changes adapter authority in this same transaction.
    let started = std::time::Instant::now();
    studio_db_api::migration::retire_source::<crate::settings_schema::OnlineMigrationDirty>(tx)?;
    eprintln!(
        "F1_AUTHORITY name={} wall_seconds={}",
        dirty_plan().name,
        started.elapsed().as_secs_f64()
    );
    let started = std::time::Instant::now();
    studio_db_api::migration::retire_source::<crate::settings_schema::PayloadMigrations>(tx)?;
    eprintln!(
        "F1_AUTHORITY name={} wall_seconds={}",
        ledger_plan().name,
        started.elapsed().as_secs_f64()
    );
    Ok(())
}

// SQLite owns this counter table. It is read only by this dated transition.
#[derive(studio_db_api::DbTable)]
#[db(table="sqlite_sequence",database=crate::peer_update_settings::Canvas,legacy_ddl="")]
struct LegacyAutoincrement {
    #[db(key)]
    name: String,
    seq: Option<i64>,
}
#[derive(studio_db_api::DbQuery)]
#[db_query(database=crate::peer_update_settings::Canvas,mode=read,cardinality=one,result=Option<i64>,decode=LegacyAutoincrement::SEQ,
    tables(rows=LegacyAutoincrement),columns(name=LegacyAutoincrement::NAME,seq=LegacyAutoincrement::SEQ),
    sql="SELECT MAX({seq}) AS {seq} FROM {rows} WHERE {name}=?1")]
struct DocumentAutoincrement<'a> {
    #[db(column=LegacyAutoincrement::NAME)]
    name: &'a str,
}

fn finish_authority(
    transaction: &studio_db_api::rusqlite::Transaction<'_>,
) -> crate::StoreResult<()> {
    use crate::metadata_schema::{AgentRecordGeneration, DurableFloor, SchedulerAgentChangeState};
    use crate::peer_update_settings::{Canvas, canvas_read, canvas_tx};
    validate_ready(&canvas_read(transaction))?;
    let mut typed = canvas_tx(transaction);
    let key = (encode_key(&(1_i64,)),);
    if typed.read().get::<DurableFloor>(&key)?.is_none()
        || typed.read().get::<AgentRecordGeneration>(&key)?.is_none()
        || typed
            .read()
            .get::<SchedulerAgentChangeState>(&key)?
            .is_none()
    {
        return Err(crate::StoreError::RecordContract(
            "F1 required clock singleton missing before authority",
        ));
    }
    let mut generation =
        typed
            .read()
            .get::<GenerationTarget>(&key)?
            .ok_or(crate::StoreError::RecordContract(
                "F1 sync generation missing before authority",
            ))?;
    let autoincrement = typed
        .read()
        .named(
            &DocumentAutoincrement::QUERY,
            &DocumentAutoincrement {
                name: crate::clock_schema::SyncDocuments::NAME,
            },
        )?
        .unwrap_or(0);
    generation.sync_generation = generation
        .sync_generation
        .max(crate::durable_seq::high(transaction)?)
        .max(autoincrement);
    typed.partition::<GenerationTarget>().put(&generation)?;
    // There are no source-row scans or backfills in this writer transaction.
    // Every producer references only target partitions before its sources retire.
    crate::entity_scopes::install(transaction)?;
    crate::bootstrap::install_current_producers(transaction)?;
    fn retire<L, T>(
        typed: &mut studio_db_api::Tx<'_, Canvas>,
        plan: &CopyPlan<L, T>,
    ) -> crate::StoreResult<()>
    where
        L: Table<Database = Canvas>,
        T: studio_db_api::MutableTable<Database = Canvas>,
    {
        let started = std::time::Instant::now();
        studio_db_api::migration::retire_source::<L>(typed)?;
        eprintln!(
            "F1_AUTHORITY name={} wall_seconds={}",
            plan.name,
            started.elapsed().as_secs_f64()
        );
        Ok(())
    }
    retire(&mut typed, &watermarks_plan())?;
    retire(&mut typed, &floor_plan())?;
    retire(&mut typed, &alias_plan())?;
    retire(&mut typed, &access_plan())?;
    retire(&mut typed, &agent_generation_plan())?;
    retire(&mut typed, &federation_plan())?;
    retire(&mut typed, &orchestration_plan())?;
    retire(&mut typed, &scheduler_state_plan())?;
    retire(&mut typed, &search_plan())?;
    retire(&mut typed, &sidebar_plan())?;
    retire(&mut typed, &batches_plan())?;
    retire(&mut typed, &sync_meta_plan())?;
    retire(&mut typed, &generation_plan())?;
    retire(&mut typed, &sync_identity_plan())?;
    retire(&mut typed, &task_window_plan())?;
    retire(&mut typed, &publication_state_plan())?;
    retire(&mut typed, &peer_settings_plan())?;
    retire(&mut typed, &durable_plan())?;
    retire(&mut typed, &scheduler_changes_plan())?;
    retire(&mut typed, &revisions_plan())?;
    retire(&mut typed, &documents_plan())?;
    retire(&mut typed, &entities_plan())?;
    retire_family_progress(&mut typed)?;
    // Only ItemSeqBackfill has external source tracking in the supported catalog.
    // Refresh its existing incomplete job without changing its cursor or queue.
    if canvas_read(transaction).table_exists::<crate::item_monitor_terminal::ItemsRow>()? {
        use crate::online_migration::OnlineMigration;
        let name = format!(
            "{}{}",
            crate::online_migration::PROGRESS_PREFIX,
            crate::item_seq_backfill::ItemSeqBackfill::NAME
        );
        if crate::migration_progress::load(&typed.read(), &name)?
            .is_some_and(|state| state.complete == 0)
        {
            crate::online_migration::begin(
                transaction,
                &crate::item_seq_backfill::ItemSeqBackfill,
            )?;
        }
    }
    super::write_current_canvas_catalog(transaction)?;
    Ok(())
}

/// Independent evidence checks finish before the single authority transaction.
fn verify_sources(
    connection: &mut studio_db_api::rusqlite::Connection,
) -> crate::StoreResult<std::time::Duration> {
    let started = std::time::Instant::now();
    verify_plan(connection, &watermarks_plan())?;
    verify_plan(connection, &floor_plan())?;
    verify_plan(connection, &alias_plan())?;
    verify_plan(connection, &access_plan())?;
    verify_plan(connection, &agent_generation_plan())?;
    verify_plan(connection, &federation_plan())?;
    verify_plan(connection, &orchestration_plan())?;
    verify_plan(connection, &scheduler_state_plan())?;
    verify_plan(connection, &search_plan())?;
    verify_plan(connection, &sidebar_plan())?;
    verify_plan(connection, &batches_plan())?;
    verify_plan(connection, &sync_meta_plan())?;
    verify_plan(connection, &generation_plan())?;
    verify_plan(connection, &sync_identity_plan())?;
    verify_plan(connection, &task_window_plan())?;
    verify_plan(connection, &publication_state_plan())?;
    verify_plan(connection, &peer_settings_plan())?;
    verify_plan(connection, &durable_plan())?;
    verify_plan(connection, &scheduler_changes_plan())?;
    verify_plan(connection, &revisions_plan())?;
    verify_plan(connection, &documents_plan())?;
    verify_plan(connection, &entities_plan())?;
    verify_plan(connection, &ledger_plan())?;
    verify_plan(connection, &dirty_plan())?;
    Ok(started.elapsed())
}

/// Exact row evidence and an independent target-key count. Each read snapshot is
/// bounded by the existing row/time and encoded-cell byte budgets.
fn verify_plan<L, T>(
    connection: &mut studio_db_api::rusqlite::Connection,
    plan: &CopyPlan<L, T>,
) -> crate::StoreResult<(u64, std::time::Duration)>
where
    L: Table<Database = crate::peer_update_settings::Canvas>,
    L::Key: studio_db_api::migration::MigrationKey,
    T: studio_db_api::MutableTable<Database = L::Database>,
    T::Key: studio_db_api::migration::MigrationKey,
{
    use crate::online_migration::{BATCH_MAX_ROWS, BATCH_TIME_LIMIT_MS};
    use std::time::{Duration, Instant};
    let started = Instant::now();
    let mut largest = Duration::ZERO;
    let mut after = None;
    let mut source_rows = 0u64;
    loop {
        let batch_started = Instant::now();
        let transaction = connection.transaction()?;
        let page = plan.read_after(&transaction, after.as_ref(), BATCH_MAX_ROWS)?;
        let exhausted = page.is_empty();
        for (key, row) in page {
            let target_key = (plan.target_key)(&key);
            let failure = || crate::StoreError::InvalidStoredRecord {
                object: plan.name,
                id: format!("{:02x?}", encode_key(&key)),
            };
            let actual = crate::peer_update_settings::canvas_read(&transaction)
                .get::<T>(&target_key)?
                .ok_or_else(failure)?;
            if (plan.source_evidence)(&row)?.digest() != (plan.target_evidence)(&actual)?.digest() {
                return Err(failure());
            }
            source_rows += 1;
            after = Some(key);
            if batch_started.elapsed() >= Duration::from_millis(BATCH_TIME_LIMIT_MS) {
                break;
            }
        }
        transaction.commit()?;
        largest = largest.max(batch_started.elapsed());
        if exhausted {
            break;
        }
    }
    let mut target_after = None;
    let mut target_rows = 0u64;
    loop {
        let batch_started = Instant::now();
        let transaction = connection.transaction()?;
        let keys = plan.target_keys_after(&transaction, target_after.as_ref(), BATCH_MAX_ROWS)?;
        let exhausted = keys.is_empty();
        target_rows += keys.len() as u64;
        target_after = keys.into_iter().last();
        transaction.commit()?;
        largest = largest.max(batch_started.elapsed());
        if exhausted {
            break;
        }
    }
    if source_rows != target_rows {
        return Err(crate::StoreError::InvalidStoredRecord {
            object: plan.name,
            id: format!("source-count={source_rows},target-count={target_rows}"),
        });
    }
    eprintln!(
        "F1_VERIFY name={} wall_seconds={} rows={} max_transaction_seconds={}",
        plan.name,
        started.elapsed().as_secs_f64(),
        source_rows,
        largest.as_secs_f64()
    );
    Ok((source_rows, largest))
}

const GENERATION_PLAN_NAME: &str = BASE_NAMES[13];

fn generation_plan() -> CopyPlan<GenerationSource, GenerationTarget> {
    CopyPlan {
        name: GENERATION_PLAN_NAME,
        source_keys: &["id"],
        source_key: |row| (row.id,),
        target_key: |key| (encode_key(key),),
        transform: |row| {
            Ok(GenerationTarget {
                identity: encode_key(&(row.id,)),
                source_rowid: row.id,
                integer_key: row.id,
                sync_generation: row.value,
            })
        },
        source_evidence: |row| {
            Ok(Evidence {
                identity: vec![row.id.into()],
                ordering: vec![row.id.into()],
                values: vec![row.value.into()],
            })
        },
        target_evidence: |row| {
            Ok(Evidence {
                identity: vec![row.integer_key.into()],
                ordering: vec![row.source_rowid.into()],
                values: vec![row.sync_generation.into()],
            })
        },
        target_select: GenerationTarget::SELECT_QUERY,
        // The family performs the one authority switch after every plan is ready.
        finish: |_| Ok(()),
    }
}

fn floor_plan() -> CopyPlan<crate::clock_schema::DurableFloor, crate::metadata_schema::DurableFloor>
{
    CopyPlan {
        name: BASE_NAMES[1],
        source_keys: &["id"],
        source_key: |row| (row.id,),
        target_key: |key| (encode_key(key),),
        transform: |row| {
            Ok(crate::metadata_schema::DurableFloor {
                identity: encode_key(&(row.id,)),
                source_rowid: row.id,
                integer_key: row.id,
                durable_floor: row.floor,
            })
        },
        source_evidence: |row| {
            Ok(Evidence {
                identity: vec![row.id.into()],
                ordering: vec![row.id.into()],
                values: vec![row.floor.into()],
            })
        },
        target_evidence: |row| {
            Ok(Evidence {
                identity: vec![row.integer_key.into()],
                ordering: vec![row.source_rowid.into()],
                values: vec![row.durable_floor.into()],
            })
        },
        target_select: crate::metadata_schema::DurableFloor::SELECT_QUERY,
        finish: |_| Ok(()),
    }
}

fn agent_generation_plan() -> CopyPlan<
    crate::clock_schema::AgentRecordGeneration,
    crate::metadata_schema::AgentRecordGeneration,
> {
    CopyPlan {
        name: BASE_NAMES[4],
        source_keys: &["id"],
        source_key: |row| (row.id,),
        target_key: |key| (encode_key(key),),
        transform: |row| {
            Ok(crate::metadata_schema::AgentRecordGeneration {
                identity: encode_key(&(row.id,)),
                source_rowid: row.id,
                integer_key: row.id,
                agent_record_generation: row.value,
            })
        },
        source_evidence: |row| {
            Ok(Evidence {
                identity: vec![row.id.into()],
                ordering: vec![row.id.into()],
                values: vec![row.value.into()],
            })
        },
        target_evidence: |row| {
            Ok(Evidence {
                identity: vec![row.integer_key.into()],
                ordering: vec![row.source_rowid.into()],
                values: vec![row.agent_record_generation.into()],
            })
        },
        target_select: crate::metadata_schema::AgentRecordGeneration::SELECT_QUERY,
        finish: |_| Ok(()),
    }
}

fn scheduler_state_plan() -> CopyPlan<
    crate::clock_schema::SchedulerAgentChangeState,
    crate::metadata_schema::SchedulerAgentChangeState,
> {
    CopyPlan {
        name: BASE_NAMES[8],
        source_keys: &["id"],
        source_key: |row| (row.id,),
        target_key: |key| (encode_key(key),),
        transform: |row| {
            Ok(crate::metadata_schema::SchedulerAgentChangeState {
                identity: encode_key(&(row.id,)),
                source_rowid: row.id,
                integer_key: row.id,
                scheduler_floor: row.floor,
                scheduler_generation: row.generation,
                scheduler_entries: row.entries,
                scheduler_total: row.total,
            })
        },
        source_evidence: |row| {
            Ok(Evidence {
                identity: vec![row.id.into()],
                ordering: vec![row.id.into()],
                values: vec![
                    row.floor.into(),
                    row.generation.into(),
                    row.entries.into(),
                    row.total.into(),
                ],
            })
        },
        target_evidence: |row| {
            Ok(Evidence {
                identity: vec![row.integer_key.into()],
                ordering: vec![row.source_rowid.into()],
                values: vec![
                    row.scheduler_floor.into(),
                    row.scheduler_generation.into(),
                    row.scheduler_entries.into(),
                    row.scheduler_total.into(),
                ],
            })
        },
        target_select: crate::metadata_schema::SchedulerAgentChangeState::SELECT_QUERY,
        finish: |_| Ok(()),
    }
}

fn peer_settings_plan() -> CopyPlan<
    crate::peer_update_settings::PeerUpdateSettings,
    crate::metadata_schema::PeerUpdateSettings,
> {
    CopyPlan {
        name: BASE_NAMES[17],
        source_keys: &["id"],
        source_key: |row| (row.id,),
        target_key: |key| (encode_key(key),),
        transform: |row| {
            Ok(crate::metadata_schema::PeerUpdateSettings {
                identity: encode_key(&(row.id,)),
                source_rowid: row.id,
                integer_key: row.id,
                auto_update: crate::metadata_values::StoredBoolean::from_stored(row.auto_update),
            })
        },
        source_evidence: |row| {
            Ok(Evidence {
                identity: vec![row.id.into()],
                ordering: vec![row.id.into()],
                values: vec![row.auto_update.into()],
            })
        },
        target_evidence: |row| {
            Ok(Evidence {
                identity: vec![row.integer_key.into()],
                ordering: vec![row.source_rowid.into()],
                values: vec![row.auto_update.as_stored().into()],
            })
        },
        target_select: crate::metadata_schema::PeerUpdateSettings::SELECT_QUERY,
        finish: |_| Ok(()),
    }
}

fn watermarks_plan() -> CopyPlan<
    crate::settings_schema::ClientCommandWatermarks,
    crate::metadata_schema::ClientCommandWatermarks,
> {
    CopyPlan {
        name: BASE_NAMES[0],
        source_keys: &["client"],
        source_key: |row| (row.client.clone(),),
        target_key: |key| (encode_key(key),),
        transform: |row| {
            Ok(crate::metadata_schema::ClientCommandWatermarks {
                identity: encode_key(&(row.client.clone(),)),
                text_key: row.client,
                applied: row.applied,
            })
        },
        source_evidence: |row| {
            Ok(Evidence {
                identity: vec![row.client.clone().into()],
                ordering: vec![],
                values: vec![row.applied.into()],
            })
        },
        target_evidence: |row| {
            Ok(Evidence {
                identity: vec![row.text_key.clone().into()],
                ordering: vec![],
                values: vec![row.applied.into()],
            })
        },
        target_select: crate::metadata_schema::ClientCommandWatermarks::SELECT_QUERY,
        finish: |_| Ok(()),
    }
}

fn alias_plan()
-> CopyPlan<crate::settings_schema::AccessLocalAlias, crate::metadata_schema::AccessLocalAlias> {
    CopyPlan {
        name: BASE_NAMES[2],
        source_keys: &["id"],
        source_key: |row| (row.id,),
        target_key: |key| (encode_key(key),),
        transform: |row| {
            Ok(crate::metadata_schema::AccessLocalAlias {
                identity: encode_key(&(row.id,)),
                source_rowid: row.id,
                integer_key: row.id,
                alias: row.alias,
            })
        },
        source_evidence: |row| {
            Ok(Evidence {
                identity: vec![row.id.into()],
                ordering: vec![row.id.into()],
                values: vec![row.alias.clone().into()],
            })
        },
        target_evidence: |row| {
            Ok(Evidence {
                identity: vec![row.integer_key.into()],
                ordering: vec![row.source_rowid.into()],
                values: vec![row.alias.clone().into()],
            })
        },
        target_select: crate::metadata_schema::AccessLocalAlias::SELECT_QUERY,
        finish: |_| Ok(()),
    }
}

fn access_plan()
-> CopyPlan<crate::settings_schema::AccessSettings, crate::metadata_schema::AccessSettings> {
    CopyPlan {
        name: BASE_NAMES[3],
        source_keys: &["id"],
        source_key: |row| (row.id,),
        target_key: |key| (encode_key(key),),
        transform: |row| {
            Ok(crate::metadata_schema::AccessSettings {
                identity: encode_key(&(row.id,)),
                source_rowid: row.id,
                integer_key: row.id,
                auto_pair: crate::metadata_values::StoredBoolean::from_stored(row.auto_pair),
            })
        },
        source_evidence: |row| {
            Ok(Evidence {
                identity: vec![row.id.into()],
                ordering: vec![row.id.into()],
                values: vec![row.auto_pair.into()],
            })
        },
        target_evidence: |row| {
            Ok(Evidence {
                identity: vec![row.integer_key.into()],
                ordering: vec![row.source_rowid.into()],
                values: vec![row.auto_pair.as_stored().into()],
            })
        },
        target_select: crate::metadata_schema::AccessSettings::SELECT_QUERY,
        finish: |_| Ok(()),
    }
}

fn documents_plan()
-> CopyPlan<crate::clock_schema::SyncDocuments, crate::publication_schema::SyncDocuments> {
    use crate::metadata_values::StoredBoolean;
    use crate::publication_schema::{DocumentValue, SyncDocuments};
    CopyPlan {
        name: BASE_NAMES[22],
        source_keys: &["seq"],
        source_key: |row| (row.seq,),
        target_key: |key| (encode_key(key),),
        transform: |row| {
            let payload = DocumentValue::from_stored(&row.scope, &row.id, row.payload)?;
            Ok(SyncDocuments {
                identity: encode_key(&(row.seq,)),
                source_rowid: row.seq,
                publication_seq: row.seq,
                deleted: StoredBoolean::from_stored(row.deleted),
                document_scope: row.scope,
                document_id: row.id,
                document_payload: payload,
            })
        },
        source_evidence: |row| {
            Ok(Evidence {
                identity: vec![row.seq.into()],
                ordering: vec![row.seq.into()],
                values: vec![
                    row.scope.clone().into(),
                    row.id.clone().into(),
                    row.payload.clone().into(),
                    row.deleted.into(),
                ],
            })
        },
        target_evidence: |row| {
            Ok(Evidence {
                identity: vec![row.publication_seq.into()],
                ordering: vec![row.source_rowid.into()],
                values: vec![
                    row.document_scope.clone().into(),
                    row.document_id.clone().into(),
                    row.document_payload.value().raw().to_owned().into(),
                    row.deleted.as_stored().into(),
                ],
            })
        },
        target_select: SyncDocuments::SELECT_QUERY,
        finish: |_| Ok(()),
    }
}

#[derive(studio_db_api::DbTable)]
#[db(table="sync_identity",database=crate::peer_update_settings::Canvas)]
struct IdentitySnapshot {
    #[db(key, column = "rowid")]
    source_rowid: i64,
    id: Option<String>,
}

#[derive(studio_db_api::DbQuery)]
#[db_query(database=crate::peer_update_settings::Canvas,mode=read,cardinality=optional,
    result=IdentitySnapshot,row=IdentitySnapshot,
    tables(source=IdentitySnapshot),
    columns(rowid=IdentitySnapshot::SOURCE_ROWID,id=IdentitySnapshot::ID),
    sql="SELECT {rowid},{id} FROM {source} WHERE {id} IS ?1 AND (?1 IS NOT NULL OR {rowid}=?2)")]
struct IdentityLookup {
    #[db(column=IdentitySnapshot::ID)]
    id: Option<String>,
    #[db(column=IdentitySnapshot::SOURCE_ROWID)]
    source_rowid: i64,
}

// This read-only cursor carries a tie key only for NULL legacy identities.
trait NullableSnapshot: Table<Database = crate::peer_update_settings::Canvas> {
    const NULLABLE_GET: &'static str;
    const ELIGIBLE_SELECT: &'static str = Self::SELECT;
}
impl NullableSnapshot for IdentitySnapshot {
    const NULLABLE_GET: &'static str = IdentityLookup::QUERY.sql();
}
struct NullableCursor<S: NullableSnapshot>(S);
type IdentitySource = NullableCursor<IdentitySnapshot>;
impl<S: NullableSnapshot> Table for NullableCursor<S> {
    type Database = crate::peer_update_settings::Canvas;
    type Key = (Option<String>, i64);
    const NAME: &'static str = S::NAME;
    const DDL: &'static str = S::DDL;
    const INDEXES: &'static [studio_db_api::IndexDescriptor] = S::INDEXES;
    const SELECT: &'static str = S::ELIGIBLE_SELECT;
    const STATE_CHECK: &'static str = S::STATE_CHECK;
    const VALID_ROWS: &'static str = S::VALID_ROWS;
    const GET: &'static str = S::NULLABLE_GET;
    const INSERT: &'static str = S::INSERT;
    const DELETE: &'static str = S::DELETE;
    const COLUMNS: &'static [studio_db_api::ColumnDescriptor] = S::COLUMNS;
    fn read(
        db: &studio_db_api::CodecContext,
        row: &studio_db_api::CodecRow<'_>,
    ) -> Result<Self, studio_db_api::CodecError> {
        S::read(db, row).map(Self)
    }
    fn bind<R>(
        &self,
        db: &studio_db_api::CodecContext,
        f: impl FnOnce(
            &[&dyn studio_db_api::rusqlite::types::ToSql],
        ) -> Result<R, studio_db_api::CodecError>,
    ) -> Result<R, studio_db_api::CodecError> {
        self.0.bind(db, f)
    }
    fn bind_key<R>(
        key: &Self::Key,
        f: impl FnOnce(
            &[&dyn studio_db_api::rusqlite::types::ToSql],
        ) -> Result<R, studio_db_api::CodecError>,
    ) -> Result<R, studio_db_api::CodecError> {
        f(&[&key.0, &key.1])
    }
}

fn sync_identity_plan() -> CopyPlan<IdentitySource, crate::metadata_schema::SyncIdentity> {
    use crate::consolidation_codec::nullable_text_identity;
    CopyPlan {
        name: BASE_NAMES[14],
        source_keys: &["id", "rowid"],
        source_key: |row| (row.0.id.clone(), row.0.source_rowid),
        target_key: |key| (nullable_text_identity(&key.0, key.1),),
        transform: |row| {
            Ok(crate::metadata_schema::SyncIdentity {
                identity: nullable_text_identity(&row.0.id, row.0.source_rowid),
                source_rowid: row.0.source_rowid,
                text_key: row.0.id,
            })
        },
        source_evidence: |row| {
            Ok(Evidence {
                identity: vec![Value::Blob(nullable_text_identity(
                    &row.0.id,
                    row.0.source_rowid,
                ))],
                ordering: vec![row.0.source_rowid.into()],
                values: vec![row.0.id.clone().map_or(Value::Null, Value::Text)],
            })
        },
        target_evidence: |row| {
            Ok(Evidence {
                identity: vec![Value::Blob(row.identity.clone())],
                ordering: vec![row.source_rowid.into()],
                values: vec![row.text_key.clone().map_or(Value::Null, Value::Text)],
            })
        },
        target_select: crate::metadata_schema::SyncIdentity::SELECT_QUERY,
        finish: |_| Ok(()),
    }
}

#[derive(studio_db_api::DbTable)]
// This migration read projection owns no installation DDL. The legacy owner
// remains clock_schema::SyncEntityMeta, including its quoted `key` column.
#[db(table="sync_entity_meta",database=crate::peer_update_settings::Canvas,legacy_ddl="")]
struct MetaSnapshot {
    #[db(key, column = "rowid")]
    source_rowid: i64,
    key: Option<String>,
    value: String,
}
#[derive(studio_db_api::DbQuery)]
#[db_query(database=crate::peer_update_settings::Canvas,mode=read,cardinality=optional,
    result=MetaSnapshot,row=MetaSnapshot,tables(source=MetaSnapshot),
    columns(rowid=MetaSnapshot::SOURCE_ROWID,key=MetaSnapshot::KEY,value=MetaSnapshot::VALUE),
    sql="SELECT {rowid},{key},{value} FROM {source} WHERE {key} IS ?1 AND (?1 IS NOT NULL OR {rowid}=?2)")]
struct MetaLookup {
    #[db(column=MetaSnapshot::KEY)]
    key: Option<String>,
    #[db(column=MetaSnapshot::SOURCE_ROWID)]
    source_rowid: i64,
}
impl NullableSnapshot for MetaSnapshot {
    const NULLABLE_GET: &'static str = MetaLookup::QUERY.sql();
}

fn sync_meta_plan() -> CopyPlan<NullableCursor<MetaSnapshot>, crate::metadata_schema::SyncEntityMeta>
{
    use crate::consolidation_codec::nullable_text_identity;
    use crate::metadata_schema::StoredSyncMetaValue;
    CopyPlan {
        name: BASE_NAMES[12],
        source_keys: &["key", "rowid"],
        source_key: |row| (row.0.key.clone(), row.0.source_rowid),
        target_key: |key| (nullable_text_identity(&key.0, key.1),),
        transform: |row| {
            Ok(crate::metadata_schema::SyncEntityMeta {
                identity: nullable_text_identity(&row.0.key, row.0.source_rowid),
                source_rowid: row.0.source_rowid,
                meta_value: StoredSyncMetaValue::from_stored(row.0.key.clone(), row.0.value),
                text_key: row.0.key,
            })
        },
        source_evidence: |row| {
            Ok(Evidence {
                identity: vec![Value::Blob(nullable_text_identity(
                    &row.0.key,
                    row.0.source_rowid,
                ))],
                ordering: vec![row.0.source_rowid.into()],
                values: vec![
                    row.0.key.clone().map_or(Value::Null, Value::Text),
                    row.0.value.clone().into(),
                ],
            })
        },
        target_evidence: |row| {
            Ok(Evidence {
                identity: vec![Value::Blob(row.identity.clone())],
                ordering: vec![row.source_rowid.into()],
                values: vec![
                    row.text_key.clone().map_or(Value::Null, Value::Text),
                    row.meta_value.as_stored().to_owned().into(),
                ],
            })
        },
        target_select: crate::metadata_schema::SyncEntityMeta::SELECT_QUERY,
        finish: |_| Ok(()),
    }
}

#[derive(studio_db_api::DbTable)]
#[db(table="durable_rows",database=crate::peer_update_settings::Canvas)]
struct DurableSnapshot {
    #[db(column = "rowid")]
    source_rowid: i64,
    #[db(key)]
    tbl: String,
    #[db(key)]
    id: String,
    seq: i64,
    deleted: i64,
    at: i64,
    #[db(codec=crate::json_record_codec::ExactJsonCodec<crate::entity_scopes::DeletedScope>)]
    scope: Option<crate::json_record_codec::ExactJson<crate::entity_scopes::DeletedScope>>,
}

fn durable_plan() -> CopyPlan<DurableSnapshot, crate::change_schema::DurableRows> {
    use crate::change_schema::{DurableRows, DurableSource};
    use crate::metadata_values::StoredBoolean;
    CopyPlan {
        name: BASE_NAMES[19],
        source_keys: &["tbl", "id"],
        source_key: |row| (row.tbl.clone(), row.id.clone()),
        target_key: |key| (encode_key(key),),
        transform: |row| {
            Ok(DurableRows {
                identity: encode_key(&(row.tbl.clone(), row.id.clone())),
                source_rowid: row.source_rowid,
                source_label: DurableSource::from_stored(row.tbl),
                durable_id: row.id,
                sequence: row.seq,
                deleted: StoredBoolean::from_stored(row.deleted),
                deleted_at: row.at,
                scope: row.scope,
            })
        },
        source_evidence: |row| {
            Ok(Evidence {
                identity: vec![row.tbl.clone().into(), row.id.clone().into()],
                ordering: vec![row.source_rowid.into()],
                values: vec![
                    row.seq.into(),
                    row.deleted.into(),
                    row.at.into(),
                    row.scope
                        .as_ref()
                        .map_or(Value::Null, |scope| Value::Text(scope.raw().to_owned())),
                ],
            })
        },
        target_evidence: |row| {
            Ok(Evidence {
                identity: vec![
                    row.source_label.as_str().to_owned().into(),
                    row.durable_id.clone().into(),
                ],
                ordering: vec![row.source_rowid.into()],
                values: vec![
                    row.sequence.into(),
                    row.deleted.as_stored().into(),
                    row.deleted_at.into(),
                    row.scope
                        .as_ref()
                        .map_or(Value::Null, |scope| Value::Text(scope.raw().to_owned())),
                ],
            })
        },
        target_select: DurableRows::SELECT_QUERY,
        finish: |_| Ok(()),
    }
}

#[derive(studio_db_api::DbTable)]
#[db(table="runtime_scheduler_agent_changes",database=crate::peer_update_settings::Canvas)]
struct SchedulerSnapshot {
    #[db(key, column = "rowid")]
    source_rowid: i64,
    id: Option<String>,
    generation: i64,
}
#[derive(studio_db_api::DbQuery)]
#[db_query(database=crate::peer_update_settings::Canvas,mode=read,cardinality=optional,
    result=SchedulerSnapshot,row=SchedulerSnapshot,tables(source=SchedulerSnapshot),
    columns(rowid=SchedulerSnapshot::SOURCE_ROWID,original=SchedulerSnapshot::ID,value=SchedulerSnapshot::GENERATION),
    sql="SELECT {rowid},{original},{value} FROM {source} WHERE {original} IS ?1 AND (?1 IS NOT NULL OR {rowid}=?2)")]
struct SchedulerLookup {
    #[db(column=SchedulerSnapshot::ID)]
    original: Option<String>,
    #[db(column=SchedulerSnapshot::SOURCE_ROWID)]
    source_rowid: i64,
}
impl NullableSnapshot for SchedulerSnapshot {
    const NULLABLE_GET: &'static str = SchedulerLookup::QUERY.sql();
}
fn scheduler_changes_plan()
-> CopyPlan<NullableCursor<SchedulerSnapshot>, crate::change_schema::SchedulerAgentChanges> {
    use crate::consolidation_codec::nullable_text_identity;
    CopyPlan {
        name: BASE_NAMES[20],
        source_keys: &["id", "rowid"],
        source_key: |row| (row.0.id.clone(), row.0.source_rowid),
        target_key: |key| (nullable_text_identity(&key.0, key.1),),
        transform: |row| {
            Ok(crate::change_schema::SchedulerAgentChanges {
                identity: nullable_text_identity(&row.0.id, row.0.source_rowid),
                source_rowid: row.0.source_rowid,
                original_key: row.0.id,
                generation: row.0.generation,
            })
        },
        source_evidence: |row| {
            Ok(Evidence {
                identity: vec![Value::Blob(nullable_text_identity(
                    &row.0.id,
                    row.0.source_rowid,
                ))],
                ordering: vec![row.0.source_rowid.into()],
                values: vec![
                    row.0.id.clone().map_or(Value::Null, Value::Text),
                    row.0.generation.into(),
                ],
            })
        },
        target_evidence: |row| {
            Ok(Evidence {
                identity: vec![Value::Blob(row.identity.clone())],
                ordering: vec![row.source_rowid.into()],
                values: vec![
                    row.original_key.clone().map_or(Value::Null, Value::Text),
                    row.generation.into(),
                ],
            })
        },
        target_select: crate::change_schema::SchedulerAgentChanges::SELECT_QUERY,
        finish: |_| Ok(()),
    }
}

#[derive(studio_db_api::DbTable)]
#[db(table="runtime_transcript_revisions",database=crate::peer_update_settings::Canvas)]
struct RevisionSnapshot {
    #[db(key, column = "rowid")]
    source_rowid: i64,
    agent: Option<String>,
    revision: i64,
}
#[derive(studio_db_api::DbQuery)]
#[db_query(database=crate::peer_update_settings::Canvas,mode=read,cardinality=optional,
    result=RevisionSnapshot,row=RevisionSnapshot,tables(source=RevisionSnapshot),
    columns(rowid=RevisionSnapshot::SOURCE_ROWID,original=RevisionSnapshot::AGENT,value=RevisionSnapshot::REVISION),
    sql="SELECT {rowid},{original},{value} FROM {source} WHERE {original} IS ?1 AND (?1 IS NOT NULL OR {rowid}=?2)")]
struct RevisionLookup {
    #[db(column=RevisionSnapshot::AGENT)]
    original: Option<String>,
    #[db(column=RevisionSnapshot::SOURCE_ROWID)]
    source_rowid: i64,
}
impl NullableSnapshot for RevisionSnapshot {
    const NULLABLE_GET: &'static str = RevisionLookup::QUERY.sql();
}
fn revisions_plan()
-> CopyPlan<NullableCursor<RevisionSnapshot>, crate::change_schema::TranscriptRevisions> {
    use crate::consolidation_codec::nullable_text_identity;
    CopyPlan {
        name: BASE_NAMES[21],
        source_keys: &["agent", "rowid"],
        source_key: |row| (row.0.agent.clone(), row.0.source_rowid),
        target_key: |key| (nullable_text_identity(&key.0, key.1),),
        transform: |row| {
            Ok(crate::change_schema::TranscriptRevisions {
                identity: nullable_text_identity(&row.0.agent, row.0.source_rowid),
                source_rowid: row.0.source_rowid,
                original_key: row.0.agent,
                revision: row.0.revision,
            })
        },
        source_evidence: |row| {
            Ok(Evidence {
                identity: vec![Value::Blob(nullable_text_identity(
                    &row.0.agent,
                    row.0.source_rowid,
                ))],
                ordering: vec![row.0.source_rowid.into()],
                values: vec![
                    row.0.agent.clone().map_or(Value::Null, Value::Text),
                    row.0.revision.into(),
                ],
            })
        },
        target_evidence: |row| {
            Ok(Evidence {
                identity: vec![Value::Blob(row.identity.clone())],
                ordering: vec![row.source_rowid.into()],
                values: vec![
                    row.original_key.clone().map_or(Value::Null, Value::Text),
                    row.revision.into(),
                ],
            })
        },
        target_select: crate::change_schema::TranscriptRevisions::SELECT_QUERY,
        finish: |_| Ok(()),
    }
}

#[derive(studio_db_api::DbTable)]
#[db(table="runtime_sidebar_order",database=crate::peer_update_settings::Canvas)]
struct SidebarSnapshot {
    #[db(key, column = "rowid")]
    source_rowid: i64,
    id: Option<String>,
    record: String,
}
#[derive(studio_db_api::DbQuery)]
#[db_query(database=crate::peer_update_settings::Canvas,mode=read,cardinality=optional,
    result=SidebarSnapshot,row=SidebarSnapshot,tables(source=SidebarSnapshot),
    columns(rowid=SidebarSnapshot::SOURCE_ROWID,id=SidebarSnapshot::ID,record=SidebarSnapshot::RECORD),
    sql="SELECT {rowid},{id},{record} FROM {source} WHERE {id} IS ?1 AND (?1 IS NOT NULL OR {rowid}=?2)")]
struct SidebarLookup {
    #[db(column=SidebarSnapshot::ID)]
    id: Option<String>,
    #[db(column=SidebarSnapshot::SOURCE_ROWID)]
    source_rowid: i64,
}
impl NullableSnapshot for SidebarSnapshot {
    const NULLABLE_GET: &'static str = SidebarLookup::QUERY.sql();
}
fn sidebar_plan() -> CopyPlan<NullableCursor<SidebarSnapshot>, crate::metadata_schema::SidebarOrder>
{
    use crate::consolidation_codec::nullable_text_identity;
    CopyPlan {
        name: BASE_NAMES[10],
        source_keys: &["id", "rowid"],
        source_key: |row| (row.0.id.clone(), row.0.source_rowid),
        target_key: |key| (nullable_text_identity(&key.0, key.1),),
        transform: |row| {
            Ok(crate::metadata_schema::SidebarOrder {
                identity: nullable_text_identity(&row.0.id, row.0.source_rowid),
                source_rowid: row.0.source_rowid,
                text_key: row.0.id,
                sidebar_record: crate::json_record_codec::ExactJson::from_stored(row.0.record)
                    .map_err(|error| {
                        studio_db_api::CodecError::FromSqlConversionFailure(
                            0,
                            studio_db_api::rusqlite::types::Type::Text,
                            Box::new(error),
                        )
                    })?,
            })
        },
        source_evidence: |row| {
            Ok(Evidence {
                identity: vec![Value::Blob(nullable_text_identity(
                    &row.0.id,
                    row.0.source_rowid,
                ))],
                ordering: vec![row.0.source_rowid.into()],
                values: vec![
                    row.0.id.clone().map_or(Value::Null, Value::Text),
                    row.0.record.clone().into(),
                ],
            })
        },
        target_evidence: |row| {
            Ok(Evidence {
                identity: vec![Value::Blob(row.identity.clone())],
                ordering: vec![row.source_rowid.into()],
                values: vec![
                    row.text_key.clone().map_or(Value::Null, Value::Text),
                    row.sidebar_record.raw().to_owned().into(),
                ],
            })
        },
        target_select: crate::metadata_schema::SidebarOrder::SELECT_QUERY,
        finish: |_| Ok(()),
    }
}

#[derive(studio_db_api::DbTable)]
#[db(table="agent_publication_state",database=crate::peer_update_settings::Canvas)]
struct PublicationStateSnapshot {
    #[db(key, column = "rowid")]
    source_rowid: i64,
    id: Option<String>,
    live: i64,
    in_flight: i64,
    team_counts: Option<String>,
}
#[derive(studio_db_api::DbQuery)]
#[db_query(database=crate::peer_update_settings::Canvas,mode=read,cardinality=optional,
    result=PublicationStateSnapshot,row=PublicationStateSnapshot,tables(source=PublicationStateSnapshot),
    columns(rowid=PublicationStateSnapshot::SOURCE_ROWID,id=PublicationStateSnapshot::ID,live=PublicationStateSnapshot::LIVE,in_flight=PublicationStateSnapshot::IN_FLIGHT,team_counts=PublicationStateSnapshot::TEAM_COUNTS),
    sql="SELECT {rowid},{id},{live},{in_flight},{team_counts} FROM {source} WHERE {id} IS ?1 AND (?1 IS NOT NULL OR {rowid}=?2)")]
struct PublicationStateLookup {
    #[db(column=PublicationStateSnapshot::ID)]
    id: Option<String>,
    #[db(column=PublicationStateSnapshot::SOURCE_ROWID)]
    source_rowid: i64,
}
impl NullableSnapshot for PublicationStateSnapshot {
    const NULLABLE_GET: &'static str = PublicationStateLookup::QUERY.sql();
}
fn publication_state_plan()
-> CopyPlan<NullableCursor<PublicationStateSnapshot>, crate::metadata_schema::AgentPublicationState>
{
    use crate::consolidation_codec::nullable_text_identity;
    CopyPlan {
        name: BASE_NAMES[16],
        source_keys: &["id", "rowid"],
        source_key: |row| (row.0.id.clone(), row.0.source_rowid),
        target_key: |key| (nullable_text_identity(&key.0, key.1),),
        transform: |row| {
            Ok(crate::metadata_schema::AgentPublicationState {
                identity: nullable_text_identity(&row.0.id, row.0.source_rowid),
                source_rowid: row.0.source_rowid,
                text_key: row.0.id,
                publication_live: crate::metadata_values::StoredBoolean::from_stored(row.0.live),
                publication_in_flight: crate::metadata_values::StoredBoolean::from_stored(
                    row.0.in_flight,
                ),
                publication_team_counts: row
                    .0
                    .team_counts
                    .map(crate::metadata_values::PublicationTeamCounts::from_stored),
            })
        },
        source_evidence: |row| {
            Ok(Evidence {
                identity: vec![Value::Blob(nullable_text_identity(
                    &row.0.id,
                    row.0.source_rowid,
                ))],
                ordering: vec![row.0.source_rowid.into()],
                values: vec![
                    row.0.id.clone().map_or(Value::Null, Value::Text),
                    row.0.live.into(),
                    row.0.in_flight.into(),
                    row.0.team_counts.clone().map_or(Value::Null, Value::Text),
                ],
            })
        },
        target_evidence: |row| {
            Ok(Evidence {
                identity: vec![Value::Blob(row.identity.clone())],
                ordering: vec![row.source_rowid.into()],
                values: vec![
                    row.text_key.clone().map_or(Value::Null, Value::Text),
                    row.publication_live.as_stored().into(),
                    row.publication_in_flight.as_stored().into(),
                    row.publication_team_counts
                        .as_ref()
                        .map_or(Value::Null, |value| Value::Text(value.as_raw().to_owned())),
                ],
            })
        },
        target_select: crate::metadata_schema::AgentPublicationState::SELECT_QUERY,
        finish: |_| Ok(()),
    }
}

#[derive(studio_db_api::DbTable)]
#[db(table="runtime_federation_settings",database=crate::peer_update_settings::Canvas)]
struct FederationSnapshot {
    #[db(key, column = "rowid")]
    source_rowid: i64,
    id: Option<String>,
    #[db(codec=crate::json_record_codec::ExactJsonCodec<crate::settings_schema::FederationSettingsPayload>)]
    record: crate::json_record_codec::ExactJson<crate::settings_schema::FederationSettingsPayload>,
}
#[derive(studio_db_api::DbQuery)]
#[db_query(database=crate::peer_update_settings::Canvas,mode=read,cardinality=optional,
    result=FederationSnapshot,row=FederationSnapshot,tables(source=FederationSnapshot),
    columns(rowid=FederationSnapshot::SOURCE_ROWID,id=FederationSnapshot::ID,record=FederationSnapshot::RECORD),
    sql="SELECT {rowid},{id},{record} FROM {source} WHERE {id} IS ?1 AND (?1 IS NOT NULL OR {rowid}=?2)")]
struct FederationLookup {
    #[db(column=FederationSnapshot::ID)]
    id: Option<String>,
    #[db(column=FederationSnapshot::SOURCE_ROWID)]
    source_rowid: i64,
}
impl NullableSnapshot for FederationSnapshot {
    const NULLABLE_GET: &'static str = FederationLookup::QUERY.sql();
}
fn federation_plan()
-> CopyPlan<NullableCursor<FederationSnapshot>, crate::metadata_schema::FederationSettings> {
    use crate::consolidation_codec::nullable_text_identity;
    CopyPlan {
        name: BASE_NAMES[5],
        source_keys: &["id", "rowid"],
        source_key: |row| (row.0.id.clone(), row.0.source_rowid),
        target_key: |key| (nullable_text_identity(&key.0, key.1),),
        transform: |row| {
            Ok(crate::metadata_schema::FederationSettings {
                identity: nullable_text_identity(&row.0.id, row.0.source_rowid),
                source_rowid: row.0.source_rowid,
                text_key: row.0.id,
                federation_record: row.0.record,
            })
        },
        source_evidence: |row| {
            Ok(Evidence {
                identity: vec![Value::Blob(nullable_text_identity(
                    &row.0.id,
                    row.0.source_rowid,
                ))],
                ordering: vec![row.0.source_rowid.into()],
                values: vec![
                    row.0.id.clone().map_or(Value::Null, Value::Text),
                    row.0.record.raw().to_owned().into(),
                ],
            })
        },
        target_evidence: |row| {
            Ok(Evidence {
                identity: vec![Value::Blob(row.identity.clone())],
                ordering: vec![row.source_rowid.into()],
                values: vec![
                    row.text_key.clone().map_or(Value::Null, Value::Text),
                    row.federation_record.raw().to_owned().into(),
                ],
            })
        },
        target_select: crate::metadata_schema::FederationSettings::SELECT_QUERY,
        finish: |_| Ok(()),
    }
}

#[derive(studio_db_api::DbTable)]
#[db(table="runtime_orchestration_settings",database=crate::peer_update_settings::Canvas)]
struct OrchestrationSnapshot {
    #[db(key, column = "rowid")]
    source_rowid: i64,
    id: Option<String>,
    #[db(codec=crate::metadata_values::OrchestrationSettingsCodec)]
    record: crate::metadata_values::OrchestrationSettingsEvidence,
}
#[derive(studio_db_api::DbQuery)]
#[db_query(database=crate::peer_update_settings::Canvas,mode=read,cardinality=optional,
    result=OrchestrationSnapshot,row=OrchestrationSnapshot,tables(source=OrchestrationSnapshot),
    columns(rowid=OrchestrationSnapshot::SOURCE_ROWID,id=OrchestrationSnapshot::ID,record=OrchestrationSnapshot::RECORD),
    sql="SELECT {rowid},{id},{record} FROM {source} WHERE {id} IS ?1 AND (?1 IS NOT NULL OR {rowid}=?2)")]
struct OrchestrationLookup {
    #[db(column=OrchestrationSnapshot::ID)]
    id: Option<String>,
    #[db(column=OrchestrationSnapshot::SOURCE_ROWID)]
    source_rowid: i64,
}
impl NullableSnapshot for OrchestrationSnapshot {
    const NULLABLE_GET: &'static str = OrchestrationLookup::QUERY.sql();
}
fn orchestration_plan()
-> CopyPlan<NullableCursor<OrchestrationSnapshot>, crate::metadata_schema::OrchestrationSettings> {
    use crate::consolidation_codec::nullable_text_identity;
    CopyPlan {
        name: BASE_NAMES[6],
        source_keys: &["id", "rowid"],
        source_key: |row| (row.0.id.clone(), row.0.source_rowid),
        target_key: |key| (nullable_text_identity(&key.0, key.1),),
        transform: |row| {
            Ok(crate::metadata_schema::OrchestrationSettings {
                identity: nullable_text_identity(&row.0.id, row.0.source_rowid),
                source_rowid: row.0.source_rowid,
                text_key: row.0.id,
                orchestration_record: row.0.record,
            })
        },
        source_evidence: |row| {
            Ok(Evidence {
                identity: vec![Value::Blob(nullable_text_identity(
                    &row.0.id,
                    row.0.source_rowid,
                ))],
                ordering: vec![row.0.source_rowid.into()],
                values: vec![
                    row.0.id.clone().map_or(Value::Null, Value::Text),
                    row.0.record.as_raw().to_owned().into(),
                ],
            })
        },
        target_evidence: |row| {
            Ok(Evidence {
                identity: vec![Value::Blob(row.identity.clone())],
                ordering: vec![row.source_rowid.into()],
                values: vec![
                    row.text_key.clone().map_or(Value::Null, Value::Text),
                    row.orchestration_record.as_raw().to_owned().into(),
                ],
            })
        },
        target_select: crate::metadata_schema::OrchestrationSettings::SELECT_QUERY,
        finish: |_| Ok(()),
    }
}

fn search_plan()
-> CopyPlan<crate::settings_schema::SearchRollout, crate::metadata_schema::SearchRollout> {
    CopyPlan {
        name: BASE_NAMES[9],
        source_keys: &["id"],
        source_key: |row| (row.id,),
        target_key: |key| (encode_key(key),),
        transform: |row| {
            Ok(crate::metadata_schema::SearchRollout {
                identity: encode_key(&(row.id,)),
                source_rowid: row.id,
                integer_key: row.id,
                search_phase: crate::metadata_values::SearchPhase::from_stored(row.phase),
                search_cursor: row.cursor,
                search_updated: row.updated,
            })
        },
        source_evidence: |row| {
            Ok(Evidence {
                identity: vec![row.id.into()],
                ordering: vec![row.id.into()],
                values: vec![
                    row.phase.clone().into(),
                    row.cursor.into(),
                    row.updated.into(),
                ],
            })
        },
        target_evidence: |row| {
            Ok(Evidence {
                identity: vec![row.integer_key.into()],
                ordering: vec![row.source_rowid.into()],
                values: vec![
                    row.search_phase.as_stored().to_owned().into(),
                    row.search_cursor.into(),
                    row.search_updated.into(),
                ],
            })
        },
        target_select: crate::metadata_schema::SearchRollout::SELECT_QUERY,
        finish: |_| Ok(()),
    }
}

#[derive(studio_db_api::DbTable)]
#[db(table="task_window_state",database=crate::peer_update_settings::Canvas)]
struct TaskSnapshot {
    #[db(key, column = "rowid")]
    source_rowid: i64,
    id: Option<String>,
    agent: String,
    created: Option<f64>,
    running: i64,
}
#[derive(studio_db_api::DbQuery)]
#[db_query(database=crate::peer_update_settings::Canvas,mode=read,cardinality=optional,
    result=TaskSnapshot,row=TaskSnapshot,tables(source=TaskSnapshot),
    columns(rowid=TaskSnapshot::SOURCE_ROWID,id=TaskSnapshot::ID,agent=TaskSnapshot::AGENT,created=TaskSnapshot::CREATED,running=TaskSnapshot::RUNNING),
    sql="SELECT {rowid},{id},{agent},{created},{running} FROM {source} WHERE {id} IS ?1 AND (?1 IS NOT NULL OR {rowid}=?2)")]
struct TaskLookup {
    #[db(column=TaskSnapshot::ID)]
    id: Option<String>,
    #[db(column=TaskSnapshot::SOURCE_ROWID)]
    source_rowid: i64,
}
impl NullableSnapshot for TaskSnapshot {
    const NULLABLE_GET: &'static str = TaskLookup::QUERY.sql();
}
fn task_window_plan()
-> CopyPlan<NullableCursor<TaskSnapshot>, crate::metadata_schema::TaskWindowState> {
    use crate::consolidation_codec::nullable_text_identity;
    CopyPlan {
        name: BASE_NAMES[15],
        source_keys: &["id", "rowid"],
        source_key: |row| (row.0.id.clone(), row.0.source_rowid),
        target_key: |key| (nullable_text_identity(&key.0, key.1),),
        transform: |row| {
            Ok(crate::metadata_schema::TaskWindowState {
                identity: nullable_text_identity(&row.0.id, row.0.source_rowid),
                source_rowid: row.0.source_rowid,
                text_key: row.0.id,
                task_agent: row.0.agent,
                task_created: row.0.created,
                task_running: crate::metadata_values::StoredBoolean::from_stored(row.0.running),
            })
        },
        source_evidence: |row| {
            Ok(Evidence {
                identity: vec![Value::Blob(nullable_text_identity(
                    &row.0.id,
                    row.0.source_rowid,
                ))],
                ordering: vec![row.0.source_rowid.into()],
                values: vec![
                    row.0.id.clone().map_or(Value::Null, Value::Text),
                    row.0.agent.clone().into(),
                    row.0.created.map_or(Value::Null, Value::Real),
                    row.0.running.into(),
                ],
            })
        },
        target_evidence: |row| {
            Ok(Evidence {
                identity: vec![Value::Blob(row.identity.clone())],
                ordering: vec![row.source_rowid.into()],
                values: vec![
                    row.text_key.clone().map_or(Value::Null, Value::Text),
                    row.task_agent.clone().into(),
                    row.task_created.map_or(Value::Null, Value::Real),
                    row.task_running.as_stored().into(),
                ],
            })
        },
        target_select: crate::metadata_schema::TaskWindowState::SELECT_QUERY,
        finish: |_| Ok(()),
    }
}

#[derive(studio_db_api::DbTable)]
// Read projection only; settings_schema::StoredTextBatches owns physical DDL.
#[db(table="runtime_stored_text_batches",database=crate::peer_update_settings::Canvas,legacy_ddl="")]
struct BatchSnapshot {
    #[db(column = "rowid")]
    source_rowid: i64,
    #[db(key)]
    name: String,
    #[db(key)]
    batch: i64,
    first_rowid: i64,
    last_rowid: i64,
    rows: i64,
    plain_bytes: i64,
    stored_bytes: i64,
    sha256: String,
    created: f64,
}
fn batches_plan() -> CopyPlan<BatchSnapshot, crate::metadata_schema::StoredTextBatches> {
    CopyPlan {
        name: BASE_NAMES[11],
        source_keys: &["name", "batch"],
        source_key: |row| (row.name.clone(), row.batch),
        target_key: |key| (encode_key(key),),
        transform: |row| {
            Ok(crate::metadata_schema::StoredTextBatches {
                identity: encode_key(&(row.name.clone(), row.batch)),
                source_rowid: row.source_rowid,
                text_key: row.name,
                batch: row.batch,
                first_rowid: row.first_rowid,
                last_rowid: row.last_rowid,
                batch_rows: row.rows,
                plain_bytes: row.plain_bytes,
                stored_bytes: row.stored_bytes,
                sha256: row.sha256,
                batch_created: row.created,
            })
        },
        source_evidence: |row| {
            Ok(Evidence {
                identity: vec![row.name.clone().into(), row.batch.into()],
                ordering: vec![row.source_rowid.into()],
                values: vec![
                    row.first_rowid.into(),
                    row.last_rowid.into(),
                    row.rows.into(),
                    row.plain_bytes.into(),
                    row.stored_bytes.into(),
                    row.sha256.clone().into(),
                    row.created.into(),
                ],
            })
        },
        target_evidence: |row| {
            Ok(Evidence {
                identity: vec![row.text_key.clone().into(), row.batch.into()],
                ordering: vec![row.source_rowid.into()],
                values: vec![
                    row.first_rowid.into(),
                    row.last_rowid.into(),
                    row.batch_rows.into(),
                    row.plain_bytes.into(),
                    row.stored_bytes.into(),
                    row.sha256.clone().into(),
                    row.batch_created.into(),
                ],
            })
        },
        target_select: crate::metadata_schema::StoredTextBatches::SELECT_QUERY,
        finish: |_| Ok(()),
    }
}

#[derive(studio_db_api::DbTable)]
// Read projection only; settings_schema::PayloadMigrations owns physical DDL.
#[db(table="runtime_payload_migrations",database=crate::peer_update_settings::Canvas,legacy_ddl="")]
struct LedgerSnapshot {
    #[db(key, column = "rowid")]
    source_rowid: i64,
    name: Option<String>,
    cursor: i64,
    complete: i64,
    status: String,
    updated: Option<f64>,
    error: Option<String>,
    cutover_ms: Option<i64>,
    cursor_key: Option<Vec<u8>>,
    copy_source_rows: Option<i64>,
    copy_target_rows: Option<i64>,
}
#[derive(studio_db_api::DbQuery)]
#[db_query(database=crate::peer_update_settings::Canvas,mode=read,cardinality=optional,
    result=LedgerSnapshot,row=LedgerSnapshot,tables(source=LedgerSnapshot),
    columns(rowid=LedgerSnapshot::SOURCE_ROWID,name=LedgerSnapshot::NAME,cursor=LedgerSnapshot::CURSOR,complete=LedgerSnapshot::COMPLETE,status=LedgerSnapshot::STATUS,updated=LedgerSnapshot::UPDATED,error=LedgerSnapshot::ERROR,cutover_ms=LedgerSnapshot::CUTOVER_MS,cursor_key=LedgerSnapshot::CURSOR_KEY,copy_source_rows=LedgerSnapshot::COPY_SOURCE_ROWS,copy_target_rows=LedgerSnapshot::COPY_TARGET_ROWS),
    sql="SELECT {rowid},{name},{cursor},{complete},{status},{updated},{error},{cutover_ms},{cursor_key},{copy_source_rows},{copy_target_rows} FROM {source} WHERE {name} IS ?1 AND (?1 IS NOT NULL OR {rowid}=?2)")]
struct LedgerLookup {
    #[db(column=LedgerSnapshot::NAME)]
    name: Option<String>,
    #[db(column=LedgerSnapshot::SOURCE_ROWID)]
    source_rowid: i64,
}
const LEDGER_FIRST_KEY: &str =
    studio_db_api::query_column::<crate::peer_update_settings::Canvas, _>(LedgerSnapshot::NAME);
const LEDGER_ELIGIBLE: &str = studio_db_api::static_sql!(
    LEDGER_FIRST_KEY,
    " IS NULL OR ",
    LEDGER_FIRST_KEY,
    " NOT IN (",
    EXCLUDED_NAMES_SQL,
    ")"
);
impl NullableSnapshot for LedgerSnapshot {
    const ELIGIBLE_SELECT: &'static str = studio_db_api::static_sql!(
        "SELECT * FROM (",
        LedgerSnapshot::SELECT,
        " WHERE (",
        LEDGER_ELIGIBLE,
        "))"
    );
    const NULLABLE_GET: &'static str =
        studio_db_api::static_sql!(LedgerLookup::QUERY.sql(), " AND (", LEDGER_ELIGIBLE, ")");
}
fn ledger_plan()
-> CopyPlan<NullableCursor<LedgerSnapshot>, crate::metadata_schema::PayloadMigrations> {
    use crate::consolidation_codec::nullable_text_identity;
    CopyPlan {
        name: BASE_NAMES[7],
        source_keys: &["name", "rowid"],
        source_key: |row| (row.0.name.clone(), row.0.source_rowid),
        target_key: |key| (nullable_text_identity(&key.0, key.1),),
        transform: |row| {
            Ok(crate::metadata_schema::PayloadMigrations {
                identity: nullable_text_identity(&row.0.name, row.0.source_rowid),
                source_rowid: row.0.source_rowid,
                text_key: row.0.name,
                migration_cursor: row.0.cursor,
                migration_complete: crate::metadata_values::StoredBoolean::from_stored(
                    row.0.complete,
                ),
                migration_status: row
                    .0
                    .status
                    .parse()
                    .map_err(|_| studio_db_api::CodecError::InvalidQuery)?,
                migration_updated: row.0.updated,
                migration_error: row.0.error,
                migration_cutover_ms: row.0.cutover_ms,
                migration_cursor_key: row.0.cursor_key,
                migration_source_rows: row.0.copy_source_rows,
                migration_target_rows: row.0.copy_target_rows,
            })
        },
        source_evidence: |row| {
            Ok(Evidence {
                identity: vec![Value::Blob(nullable_text_identity(
                    &row.0.name,
                    row.0.source_rowid,
                ))],
                ordering: vec![row.0.source_rowid.into()],
                values: vec![
                    row.0.name.clone().map_or(Value::Null, Value::Text),
                    row.0.cursor.into(),
                    row.0.complete.into(),
                    row.0.status.clone().into(),
                    row.0.updated.map_or(Value::Null, Value::Real),
                    row.0.error.clone().map_or(Value::Null, Value::Text),
                    row.0.cutover_ms.map_or(Value::Null, Value::Integer),
                    row.0.cursor_key.clone().map_or(Value::Null, Value::Blob),
                    row.0.copy_source_rows.map_or(Value::Null, Value::Integer),
                    row.0.copy_target_rows.map_or(Value::Null, Value::Integer),
                ],
            })
        },
        target_evidence: |row| {
            Ok(Evidence {
                identity: vec![Value::Blob(row.identity.clone())],
                ordering: vec![row.source_rowid.into()],
                values: vec![
                    row.text_key.clone().map_or(Value::Null, Value::Text),
                    row.migration_cursor.into(),
                    row.migration_complete.as_stored().into(),
                    row.migration_status.as_str().to_owned().into(),
                    row.migration_updated.map_or(Value::Null, Value::Real),
                    row.migration_error.clone().map_or(Value::Null, Value::Text),
                    row.migration_cutover_ms.map_or(Value::Null, Value::Integer),
                    row.migration_cursor_key
                        .clone()
                        .map_or(Value::Null, Value::Blob),
                    row.migration_source_rows
                        .map_or(Value::Null, Value::Integer),
                    row.migration_target_rows
                        .map_or(Value::Null, Value::Integer),
                ],
            })
        },
        target_select: crate::metadata_schema::PayloadMigrations::SELECT_QUERY,
        finish: |_| Ok(()),
    }
}
fn ledger_copy()
-> FamilyCopy<NullableCursor<LedgerSnapshot>, crate::metadata_schema::PayloadMigrations> {
    let mut copy = FamilyCopy::new(ledger_plan());
    copy.excluded_first_keys = &PROGRESS_NAMES;
    copy
}

type LegacyDirty = crate::settings_schema::OnlineMigrationDirty;
const DIRTY_FIRST_KEY: &str =
    studio_db_api::query_column::<crate::peer_update_settings::Canvas, _>(LegacyDirty::MIGRATION);
const DIRTY_ELIGIBLE: &str = studio_db_api::static_sql!(
    DIRTY_FIRST_KEY,
    " IS NULL OR ",
    DIRTY_FIRST_KEY,
    " NOT IN (",
    EXCLUDED_NAMES_SQL,
    ")"
);
struct DirtySource(LegacyDirty);
impl Table for DirtySource {
    type Database = crate::peer_update_settings::Canvas;
    type Key = <LegacyDirty as Table>::Key;
    const NAME: &'static str = LegacyDirty::NAME;
    const DDL: &'static str = LegacyDirty::DDL;
    const INDEXES: &'static [studio_db_api::IndexDescriptor] = LegacyDirty::INDEXES;
    const SELECT: &'static str = studio_db_api::static_sql!(
        "SELECT * FROM (",
        LegacyDirty::SELECT,
        " WHERE (",
        DIRTY_ELIGIBLE,
        "))"
    );
    const STATE_CHECK: &'static str = LegacyDirty::STATE_CHECK;
    const VALID_ROWS: &'static str = LegacyDirty::VALID_ROWS;
    const GET: &'static str =
        studio_db_api::static_sql!(LegacyDirty::GET, " AND (", DIRTY_ELIGIBLE, ")");
    const INSERT: &'static str = LegacyDirty::INSERT;
    const DELETE: &'static str = LegacyDirty::DELETE;
    const COLUMNS: &'static [studio_db_api::ColumnDescriptor] = LegacyDirty::COLUMNS;
    fn read(
        db: &studio_db_api::CodecContext,
        row: &studio_db_api::CodecRow<'_>,
    ) -> Result<Self, studio_db_api::CodecError> {
        LegacyDirty::read(db, row).map(Self)
    }
    fn bind<R>(
        &self,
        db: &studio_db_api::CodecContext,
        f: impl FnOnce(
            &[&dyn studio_db_api::rusqlite::types::ToSql],
        ) -> Result<R, studio_db_api::CodecError>,
    ) -> Result<R, studio_db_api::CodecError> {
        self.0.bind(db, f)
    }
    fn bind_key<R>(
        key: &Self::Key,
        f: impl FnOnce(
            &[&dyn studio_db_api::rusqlite::types::ToSql],
        ) -> Result<R, studio_db_api::CodecError>,
    ) -> Result<R, studio_db_api::CodecError> {
        LegacyDirty::bind_key(key, f)
    }
}
fn dirty_plan() -> CopyPlan<DirtySource, crate::metadata_schema::OnlineMigrationDirty> {
    use studio_db_api::migration::KeyPart;
    CopyPlan {
        name: BASE_NAMES[18],
        source_keys: &["migration", "key"],
        source_key: |row| (row.0.migration.clone(), row.0.key.clone()),
        target_key: |key| (encode_key(key),),
        transform: |row| {
            Ok(crate::metadata_schema::OnlineMigrationDirty {
                identity: encode_key(&(row.0.migration.clone(), row.0.key.clone())),
                dirty_migration: row.0.migration,
                dirty_key: row.0.key,
            })
        },
        source_evidence: |row| {
            Ok(Evidence {
                identity: vec![row.0.migration.clone().into(), row.0.key.value()],
                ordering: vec![],
                values: vec![],
            })
        },
        target_evidence: |row| {
            Ok(Evidence {
                identity: vec![row.dirty_migration.clone().into(), row.dirty_key.value()],
                ordering: vec![],
                values: vec![],
            })
        },
        target_select: crate::metadata_schema::OnlineMigrationDirty::SELECT_QUERY,
        finish: |_| Ok(()),
    }
}
fn dirty_copy() -> FamilyCopy<DirtySource, crate::metadata_schema::OnlineMigrationDirty> {
    let mut copy = FamilyCopy::new(dirty_plan());
    copy.excluded_first_keys = &PROGRESS_NAMES;
    copy
}

// Migration snapshot includes the physical rowid; ordinary publication readers
// intentionally expose only the logical collection/id key.
#[derive(studio_db_api::DbTable)]
#[db(table="sync_entities",database=crate::peer_update_settings::Canvas)]
struct EntitySource {
    #[db(column = "rowid")]
    source_rowid: i64,
    #[db(key)]
    collection: String,
    #[db(key)]
    id: String,
    seq: i64,
    hash: String,
    payload: Option<String>,
    deleted: i64,
}

fn entities_plan() -> CopyPlan<EntitySource, crate::publication_schema::SyncEntities> {
    use crate::metadata_values::StoredBoolean;
    use crate::publication_collection::CollectionName;
    use crate::publication_schema::{EntityValue, SyncEntities};
    CopyPlan {
        name: BASE_NAMES[23],
        source_keys: &["collection", "id"],
        source_key: |row| (row.collection.clone(), row.id.clone()),
        target_key: |key| (encode_key(key),),
        transform: |row| {
            let deleted = StoredBoolean::from_stored(row.deleted);
            let entity_payload =
                EntityValue::from_stored(&row.collection, &row.id, deleted, row.payload)?;
            let collection_order = CollectionName::from_stored(row.collection);
            Ok(SyncEntities {
                identity: encode_key(&(collection_order.as_str().to_owned(), row.id.clone())),
                source_rowid: row.source_rowid,
                publication_seq: row.seq,
                deleted,
                collection_kind: collection_order.kind(),
                collection_agent: collection_order.agent().cloned(),
                non_transcript: collection_order.non_transcript(),
                collection_order,
                entity_id: row.id,
                entity_hash: row.hash,
                entity_payload,
            })
        },
        source_evidence: |row| {
            Ok(Evidence {
                identity: vec![row.collection.clone().into(), row.id.clone().into()],
                ordering: vec![row.source_rowid.into()],
                values: vec![
                    row.seq.into(),
                    row.hash.clone().into(),
                    row.payload.clone().map_or(Value::Null, Value::Text),
                    row.deleted.into(),
                ],
            })
        },
        target_evidence: |row| {
            Ok(Evidence {
                identity: vec![
                    row.collection_order.as_str().to_owned().into(),
                    row.entity_id.clone().into(),
                ],
                ordering: vec![row.source_rowid.into()],
                values: vec![
                    row.publication_seq.into(),
                    row.entity_hash.clone().into(),
                    row.entity_payload
                        .value()
                        .raw()
                        .map_or(Value::Null, |raw| Value::Text(raw.to_owned())),
                    row.deleted.as_stored().into(),
                ],
            })
        },
        target_select: SyncEntities::SELECT_QUERY,
        finish: |_| Ok(()),
    }
}

/// Reuses CopyPlan admission, cursor, evidence and typed writes. Family completion
/// remains separate so every stream can keep tracking until one final transaction.
type Reconcile<L, T> = fn(
    &CopyPlan<L, T>,
    &mut studio_db_api::Tx<'_, crate::peer_update_settings::Canvas>,
    &T,
) -> crate::StoreResult<i64>;
struct FamilyCopy<L: Table, T: studio_db_api::MutableTable<Database = L::Database>> {
    plan: CopyPlan<L, T>,
    reconcile: Reconcile<L, T>,
    excluded_first_keys: &'static [&'static str],
}
impl<L: Table, T: studio_db_api::MutableTable<Database = L::Database>> FamilyCopy<L, T> {
    fn new(plan: CopyPlan<L, T>) -> Self {
        Self {
            plan,
            reconcile: |_, _, _| Ok(0),
            excluded_first_keys: &[],
        }
    }
}

fn documents_copy()
-> FamilyCopy<crate::clock_schema::SyncDocuments, crate::publication_schema::SyncDocuments> {
    FamilyCopy {
        plan: documents_plan(),
        reconcile: reconcile_document,
        excluded_first_keys: &[],
    }
}

fn reconcile_document(
    plan: &CopyPlan<crate::clock_schema::SyncDocuments, crate::publication_schema::SyncDocuments>,
    tx: &mut studio_db_api::Tx<'_, crate::peer_update_settings::Canvas>,
    out: &crate::publication_schema::SyncDocuments,
) -> crate::StoreResult<i64> {
    use crate::online_migration::OnlineMigration;
    use crate::publication_schema::SyncDocuments;
    let displaced = studio_db_api::db_query!(
        tx.read(),
        SyncDocuments,
        filter(
            SyncDocuments::DOCUMENT_SCOPE.eq(&out.document_scope),
            SyncDocuments::DOCUMENT_ID.eq(&out.document_id)
        )
    )?
    .into_iter()
    .next();
    if let Some(old) = displaced
        && old.identity != out.identity
    {
        let source_key = studio_db_api::migration::decode_key::<(i64,)>(&old.identity)?;
        return plan.remove(tx, &source_key);
    }
    Ok(0)
}

fn entities_copy() -> FamilyCopy<EntitySource, crate::publication_schema::SyncEntities> {
    FamilyCopy {
        plan: entities_plan(),
        reconcile: reconcile_entity,
        excluded_first_keys: &[],
    }
}

fn reconcile_entity(
    plan: &CopyPlan<EntitySource, crate::publication_schema::SyncEntities>,
    tx: &mut studio_db_api::Tx<'_, crate::peer_update_settings::Canvas>,
    out: &crate::publication_schema::SyncEntities,
) -> crate::StoreResult<i64> {
    use crate::online_migration::OnlineMigration;
    use crate::publication_schema::SyncEntities;
    let displaced = studio_db_api::db_query!(
        tx.read(),
        SyncEntities,
        filter(SyncEntities::SOURCE_ROWID.eq(&out.source_rowid))
    )?
    .into_iter()
    .next();
    if let Some(old) = displaced
        && old.identity != out.identity
    {
        let source_key = studio_db_api::migration::decode_key::<(String, String)>(&old.identity)?;
        return plan.remove(tx, &source_key);
    }
    Ok(0)
}
impl<L, T> crate::online_migration::OnlineMigration for FamilyCopy<L, T>
where
    L: Table<Database = crate::peer_update_settings::Canvas>,
    L::Key: studio_db_api::migration::MigrationKey,
    T: studio_db_api::MutableTable<Database = L::Database> + studio_db_api::PartitionTable,
    T::Key: studio_db_api::migration::MigrationKey,
{
    type Key = L::Key;
    type Row = L;
    type Out = (T, Evidence);
    const NAME: &'static str = L::NAME;
    const SOURCE_TABLE: &'static str = L::NAME;
    const SOURCE_KEYS: &'static [&'static str] = &[];
    const DEFER_FINISH: bool = true;
    fn name(&self) -> &str {
        self.plan.name
    }
    fn source_keys(&self) -> &[&str] {
        self.plan.source_keys
    }
    fn excluded_first_keys(&self) -> &'static [&'static str] {
        self.excluded_first_keys
    }
    fn prepare(&self, tx: &studio_db_api::rusqlite::Transaction<'_>) -> crate::StoreResult<()> {
        // A partition inherits the table DDL, but its physical owner owns the
        // indexes needed both for integrity and displaced-source reconciliation.
        T::Physical::descriptor().install_if_absent(tx)?;
        crate::online_migration::OnlineMigration::prepare(&self.plan, tx)
    }
    fn read_after(
        &self,
        db: &studio_db_api::CodecContext,
        after: Option<&Self::Key>,
        limit: usize,
    ) -> crate::StoreResult<Vec<(Self::Key, Self::Row)>> {
        Ok(self.plan.read_after(db, after, limit)?)
    }
    fn read_one(
        &self,
        db: &studio_db_api::CodecContext,
        key: &Self::Key,
    ) -> crate::StoreResult<Option<Self::Row>> {
        Ok(self.plan.read_one(db, key)?)
    }
    fn transform(&self, row: Self::Row) -> crate::StoreResult<Self::Out> {
        crate::online_migration::OnlineMigration::transform(&self.plan, row)
    }
    fn write(
        &self,
        tx: &mut studio_db_api::Tx<'_, crate::peer_update_settings::Canvas>,
        key: &Self::Key,
        out: Self::Out,
    ) -> crate::StoreResult<i64> {
        let displaced = (self.reconcile)(&self.plan, tx, &out.0)?;
        let copied = crate::online_migration::OnlineMigration::write(&self.plan, tx, key, out)?;
        displaced
            .checked_add(copied)
            .ok_or(crate::StoreError::RecordContract(
                "family target delta overflow",
            ))
    }
    fn remove(
        &self,
        tx: &mut studio_db_api::Tx<'_, crate::peer_update_settings::Canvas>,
        key: &Self::Key,
    ) -> crate::StoreResult<i64> {
        crate::online_migration::OnlineMigration::remove(&self.plan, tx, key)
    }
    fn finish(&self, tx: &studio_db_api::rusqlite::Transaction<'_>) -> crate::StoreResult<()> {
        crate::online_migration::OnlineMigration::finish(&self.plan, tx)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    // Functional copy/replay witnesses must not depend on scheduler load.
    struct FixedClock;
    impl crate::online_migration::Clock for FixedClock {
        fn now(&self) -> std::time::Duration {
            std::time::Duration::ZERO
        }
    }

    use crate::metadata_schema::{DurableFloor, Metadata, SyncGeneration};
    use crate::peer_update_settings::{canvas_read, canvas_tx};
    use studio_db_api::{Table, migration::encode_key};

    /// This dated copy fixture deliberately starts before F1 authority changes.
    fn legacy_source_canvas() -> rusqlite::Connection {
        let mut db = rusqlite::Connection::open_in_memory().unwrap();
        for descriptor in legacy_source_descriptors() {
            descriptor.install_if_absent(&db).unwrap();
        }
        let transaction = db.transaction().unwrap();
        let mut tx = canvas_tx(&transaction);
        tx.insert(&crate::clock_schema::SyncGeneration { id: 1, value: 0 })
            .unwrap();
        tx.insert(&crate::clock_schema::DurableFloor { id: 1, floor: 0 })
            .unwrap();
        tx.insert(&crate::clock_schema::AgentRecordGeneration { id: 1, value: 0 })
            .unwrap();
        tx.insert(&crate::clock_schema::SchedulerAgentChangeState {
            id: 1,
            floor: 0,
            generation: 0,
            entries: 0,
            total: 0,
        })
        .unwrap();
        transaction.commit().unwrap();
        db
    }

    #[test]
    fn bounded_verification_refuses_changed_missing_and_extra_target_rows() {
        let mut db = studio_db_api::rusqlite::Connection::open_in_memory().unwrap();
        GenerationSource::descriptor()
            .install_if_absent(&db)
            .unwrap();
        Metadata::descriptor().install_if_absent(&db).unwrap();
        let transaction = db.transaction().unwrap();
        canvas_tx(&transaction)
            .upsert(&GenerationSource { id: 1, value: 901 })
            .unwrap();
        generation_plan()
            .copy_tx(&mut canvas_tx(&transaction))
            .unwrap();
        transaction.commit().unwrap();
        assert_eq!(verify_plan(&mut db, &generation_plan()).unwrap().0, 1);
        let transaction = db.transaction().unwrap();
        let mut row = canvas_read(&transaction)
            .get::<GenerationTarget>(&(encode_key(&(1_i64,)),))
            .unwrap()
            .unwrap();
        row.sync_generation = 902;
        canvas_tx(&transaction)
            .partition::<GenerationTarget>()
            .put(&row)
            .unwrap();
        transaction.commit().unwrap();
        assert!(verify_plan(&mut db, &generation_plan()).is_err());
        let transaction = db.transaction().unwrap();
        row.sync_generation = 901;
        canvas_tx(&transaction)
            .partition::<GenerationTarget>()
            .put(&row)
            .unwrap();
        let extra = GenerationTarget {
            identity: encode_key(&(2_i64,)),
            source_rowid: 2,
            integer_key: 2,
            sync_generation: 5,
        };
        canvas_tx(&transaction)
            .partition::<GenerationTarget>()
            .put(&extra)
            .unwrap();
        transaction.commit().unwrap();
        assert!(verify_plan(&mut db, &generation_plan()).is_err());
        let transaction = db.transaction().unwrap();
        canvas_tx(&transaction)
            .partition::<GenerationTarget>()
            .delete(&(extra.identity,))
            .unwrap();
        canvas_tx(&transaction)
            .partition::<GenerationTarget>()
            .delete(&(row.identity,))
            .unwrap();
        transaction.commit().unwrap();
        assert!(verify_plan(&mut db, &generation_plan()).is_err());
    }

    #[test]
    fn family_prepares_physical_indexes_before_entity_reconciliation() {
        use crate::online_migration::begin;
        use crate::publication_schema::{Publications, SyncEntities};
        use std::sync::{
            Arc,
            atomic::{AtomicUsize, Ordering},
        };
        let mut db = studio_db_api::rusqlite::Connection::open_in_memory().unwrap();
        EntitySource::descriptor().install_if_absent(&db).unwrap();
        crate::settings_schema::PayloadMigrations::descriptor()
            .install_if_absent(&db)
            .unwrap();
        let transaction = db.transaction().unwrap();
        let plan = entities_copy();
        begin(&transaction, &plan).unwrap();
        let mut typed = canvas_tx(&transaction);
        for id in 0..2000 {
            let row = (plan.plan.transform)(EntitySource {
                source_rowid: id,
                collection: "future".into(),
                id: format!("id-{id}"),
                seq: id,
                hash: "historical".into(),
                payload: None,
                deleted: 0,
            })
            .unwrap();
            typed.partition::<SyncEntities>().put(&row).unwrap();
        }
        let candidate = (plan.plan.transform)(EntitySource {
            source_rowid: 2001,
            collection: "future".into(),
            id: "candidate".into(),
            seq: 2001,
            hash: "historical".into(),
            payload: None,
            deleted: 0,
        })
        .unwrap();
        let steps = Arc::new(AtomicUsize::new(0));
        let observed = steps.clone();
        transaction
            .progress_handler(
                1,
                Some(move || {
                    observed.fetch_add(1, Ordering::Relaxed);
                    false
                }),
            )
            .unwrap();
        reconcile_entity(&plan.plan, &mut typed, &candidate).unwrap();
        transaction
            .progress_handler(0, None::<fn() -> bool>)
            .unwrap();
        assert!(
            steps.load(Ordering::Relaxed) < 1000,
            "displaced rowid scan: {} VM steps",
            steps.load(Ordering::Relaxed)
        );
        let objects = studio_db_api::catalog::export(&transaction).unwrap();
        for index in Publications::INDEXES {
            assert!(
                objects.iter().any(|entry| entry.name == index.name),
                "missing physical owner index {}",
                index.name
            );
        }
    }

    #[test]
    fn all_twenty_four_plans_reach_ready_in_startup_copy_phase() {
        let mut db = legacy_source_canvas();
        copy_sources(&mut db).unwrap();
        verify_sources(&mut db).unwrap();
        let read = canvas_read(&db);
        for name in PROGRESS_NAMES {
            let state = crate::migration_progress::load(&read, name)
                .unwrap()
                .unwrap();
            assert_eq!(state.complete, 0, "copy phase cannot switch authority");
            assert_eq!(state.copy_source_rows, state.copy_target_rows, "{name}");
            assert!(
                crate::migration_progress::dirty(&read, name, 1)
                    .unwrap()
                    .is_empty()
            );
        }
        assert!(
            read.table_exists::<crate::settings_schema::PayloadMigrations>()
                .unwrap()
        );
    }

    #[test]
    fn current_repository_requirements_exclude_retired_family_sources() {
        let retired = legacy_source_descriptors().map(|descriptor| descriptor.name);
        let mut stale = Vec::new();
        for registration in crate::repositories::schema_registrations() {
            let requirements = registration.requirements;
            for table in requirements
                .tables
                .iter()
                .chain(requirements.bootstrap_tables)
                .chain(
                    requirements
                        .conditional_tables
                        .iter()
                        .flat_map(|group| group.tables),
                )
            {
                if retired.contains(&table.name) {
                    stale.push(format!("{} table {}", registration.repository, table.name));
                }
            }
            for trigger in requirements
                .triggers
                .iter()
                .chain(requirements.bootstrap_triggers)
                .chain(
                    requirements
                        .conditional_triggers
                        .iter()
                        .flat_map(|group| group.triggers),
                )
            {
                if retired.contains(&trigger.table) {
                    stale.push(format!(
                        "{} trigger {} on {}",
                        registration.repository, trigger.name, trigger.table
                    ));
                }
            }
            for group in requirements.conditional_tables {
                if retired.contains(&group.table_exists) {
                    stale.push(format!(
                        "{} condition {}",
                        registration.repository, group.table_exists
                    ));
                }
            }
            for group in requirements.conditional_triggers {
                if retired.contains(&group.table_exists) {
                    stale.push(format!(
                        "{} condition {}",
                        registration.repository, group.table_exists
                    ));
                }
            }
        }
        assert!(stale.is_empty(), "retired source requirements: {stale:#?}");
    }

    #[tokio::test]
    async fn current_store_reopens_with_canonical_catalog() {
        let root = tempfile::tempdir().unwrap();
        drop(
            crate::RuntimeStore::open(Some(root.path()))
                .await
                .expect("initial open"),
        );
        let db = crate::open_database(
            &root.path().join("canvas.sqlite3"),
            studio_db_api::rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
        )
        .expect("open baseline Canvas");
        for descriptor in legacy_source_descriptors() {
            assert!(
                !crate::schema::table_exists(&db, descriptor.name).unwrap(),
                "fresh open recreated {}",
                descriptor.name
            );
        }
        assert!(super::super::current_canvas_authority(&db).expect("authority admission"));
        super::super::verify_current_canvas_catalog(&db).expect("catalog admission");
        let analytics = crate::open_database(
            &root.path().join("analytics.sqlite3"),
            studio_db_api::rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
        )
        .expect("open baseline Analytics");
        super::super::verify_catalog(
            &analytics,
            "analytics_meta",
            studio_runtime_schema::studio_errors::MigrationTarget::Analytics,
            "analytics",
        )
        .expect("analytics catalog admission");
        super::super::verify_baseline(root.path()).expect("canonical baseline admission");
        super::super::run(root.path(), None).expect("canonical migration lifecycle");
        drop(
            crate::RuntimeStore::open(Some(root.path()))
                .await
                .expect("canonical reopen"),
        );
        for descriptor in legacy_source_descriptors() {
            assert!(
                !crate::schema::table_exists(&db, descriptor.name).unwrap(),
                "reopen recreated {}",
                descriptor.name
            );
        }
    }

    #[tokio::test]
    async fn canonical_reopen_keeps_schema_version_and_catalog_bytes() {
        let root = tempfile::tempdir().unwrap();
        drop(crate::RuntimeStore::open(Some(root.path())).await.unwrap());
        let db = rusqlite::Connection::open(root.path().join("canvas.sqlite3")).unwrap();
        let before = super::super::schema_version(&db).unwrap();
        let catalog = crate::clock_schema::SyncEntityMeta::value(&db, "migration_catalog").unwrap();
        drop(crate::RuntimeStore::open(Some(root.path())).await.unwrap());
        assert_eq!(
            super::super::schema_version(&db).unwrap(),
            before,
            "current reopen must not rebuild producer triggers"
        );
        assert_eq!(
            crate::clock_schema::SyncEntityMeta::value(&db, "migration_catalog").unwrap(),
            catalog
        );
    }

    #[test]
    fn startup_coordinator_resumes_ready_copy_and_reopens_without_source_tables() {
        let mut db = studio_db_api::rusqlite::Connection::open_in_memory().unwrap();
        for descriptor in legacy_source_descriptors() {
            descriptor.install_if_absent(&db).unwrap();
        }
        let tx = db.transaction().unwrap();
        let mut typed = canvas_tx(&tx);
        typed
            .insert(&crate::clock_schema::SyncGeneration { id: 1, value: 19 })
            .unwrap();
        typed
            .insert(&crate::clock_schema::DurableFloor { id: 1, floor: 7 })
            .unwrap();
        typed
            .insert(&crate::clock_schema::AgentRecordGeneration { id: 1, value: 12 })
            .unwrap();
        typed
            .insert(&crate::clock_schema::SchedulerAgentChangeState {
                id: 1,
                floor: 2,
                generation: 12,
                entries: 0,
                total: 0,
            })
            .unwrap();
        tx.commit().unwrap();
        copy_sources(&mut db).unwrap();
        run(&mut db).unwrap();
        for descriptor in legacy_source_descriptors() {
            assert!(
                !crate::schema::table_exists(&db, descriptor.name).unwrap(),
                "{} remains",
                descriptor.name
            );
        }
        assert_eq!(crate::durable_seq::high_and_floor(&db).unwrap(), (19, 7));
        let version = super::super::schema_version(&db).unwrap();
        run(&mut db).unwrap();
        assert_eq!(super::super::schema_version(&db).unwrap(), version);
    }

    #[test]
    fn family_authority_preserves_deleted_autoincrement_high_and_rolls_back() {
        let mut db = studio_db_api::rusqlite::Connection::open_in_memory().unwrap();
        for descriptor in legacy_source_descriptors() {
            descriptor.install_if_absent(&db).unwrap();
        }
        {
            let transaction = db.transaction().unwrap();
            let mut typed = canvas_tx(&transaction);
            typed
                .insert(&crate::clock_schema::SyncGeneration { id: 1, value: 0 })
                .unwrap();
            typed
                .insert(&crate::clock_schema::SyncGeneration {
                    id: 9,
                    value: 80000,
                })
                .unwrap();
            typed
                .insert(&crate::clock_schema::DurableFloor { id: 1, floor: 7 })
                .unwrap();
            typed
                .insert(&crate::clock_schema::AgentRecordGeneration { id: 1, value: 12 })
                .unwrap();
            typed
                .insert(&crate::clock_schema::SchedulerAgentChangeState {
                    id: 1,
                    floor: 2,
                    generation: 12,
                    entries: 0,
                    total: 0,
                })
                .unwrap();
            typed
                .insert(&crate::clock_schema::SyncDocuments {
                    seq: 9009,
                    scope: "fixture".into(),
                    id: "removed".into(),
                    payload: "{}".into(),
                    deleted: 0,
                })
                .unwrap();
            typed
                .delete::<crate::clock_schema::SyncDocuments>(&(9009,))
                .unwrap();
            transaction.commit().unwrap();
        }
        copy_sources(&mut db).unwrap();
        verify_sources(&mut db).unwrap();
        {
            let transaction = db.transaction().unwrap();
            finish_authority(&transaction).unwrap();
            for descriptor in legacy_source_descriptors() {
                assert!(
                    !crate::schema::table_exists(&transaction, descriptor.name).unwrap(),
                    "{} remains active",
                    descriptor.name
                );
            }
            assert_eq!(crate::durable_seq::high(&transaction).unwrap(), 9009);
            assert_eq!(
                crate::durable_seq::high_and_floor(&transaction).unwrap(),
                (9009, 7)
            );
            let extra = canvas_read(&transaction)
                .get::<GenerationTarget>(&(encode_key(&(9_i64,)),))
                .unwrap()
                .unwrap();
            assert_eq!(extra.sync_generation, 80000);
            for name in PROGRESS_NAMES {
                assert_eq!(
                    crate::migration_progress::load(&canvas_read(&transaction), name)
                        .unwrap()
                        .unwrap()
                        .complete,
                    1
                );
            }
            transaction.rollback().unwrap();
        }
        assert!(
            canvas_read(&db)
                .table_exists::<crate::clock_schema::SyncDocuments>()
                .unwrap()
        );
        assert_eq!(
            canvas_read(&db)
                .get::<GenerationTarget>(&(encode_key(&(1_i64,)),))
                .unwrap()
                .unwrap()
                .sync_generation,
            0
        );
        let transaction = db.transaction().unwrap();
        finish_authority(&transaction).unwrap();
        transaction.commit().unwrap();
        assert_eq!(crate::durable_seq::high(&db).unwrap(), 9009);
    }

    #[test]
    fn family_progress_switch_is_atomic_and_preserves_other_jobs() {
        let mut db = rusqlite::Connection::open_in_memory().unwrap();
        for descriptor in legacy_source_descriptors() {
            descriptor.install_if_absent(&db).unwrap();
        }
        {
            let transaction = db.transaction().unwrap();
            let mut typed = crate::peer_update_settings::canvas_tx(&transaction);
            typed
                .insert(&crate::settings_schema::PayloadMigrations {
                    name: "online:other-family".into(),
                    cursor: 91,
                    complete: 0,
                    status: "running".into(),
                    updated: Some(2.5),
                    error: None,
                    cutover_ms: Some(17),
                    receipt_enum_guard: None,
                    cursor_key: Some(vec![3, 1]),
                    copy_source_rows: Some(99),
                    copy_target_rows: Some(11),
                })
                .unwrap();
            typed
                .insert(&crate::settings_schema::OnlineMigrationDirty {
                    migration: "online:other-family".into(),
                    key: studio_db_api::migration::DirtyKey::Integer(12),
                })
                .unwrap();
            transaction.commit().unwrap();
        }
        copy_sources(&mut db).unwrap();
        verify_sources(&mut db).unwrap();
        {
            let transaction = db.transaction().unwrap();
            retire_family_progress(&mut crate::peer_update_settings::canvas_tx(&transaction))
                .unwrap();
            assert!(
                !canvas_read(&transaction)
                    .table_exists::<crate::settings_schema::PayloadMigrations>()
                    .unwrap()
            );
            for name in PROGRESS_NAMES {
                let state = crate::migration_progress::load(&canvas_read(&transaction), name)
                    .unwrap()
                    .unwrap();
                assert_eq!(
                    (state.complete, state.status.as_str()),
                    (1, "complete"),
                    "{name}"
                );
                assert_eq!(state.copy_source_rows, state.copy_target_rows, "{name}");
            }
            let other = canvas_read(&transaction)
                .get::<crate::metadata_schema::PayloadMigrations>(&(
                    crate::consolidation_codec::nullable_text_identity(
                        &Some("online:other-family".into()),
                        0,
                    ),
                ))
                .unwrap()
                .unwrap();
            assert_eq!(
                (
                    other.migration_cursor,
                    other.migration_complete.as_stored(),
                    other.migration_cutover_ms,
                    other.migration_cursor_key
                ),
                (91, 0, Some(17), Some(vec![3, 1]))
            );
            assert_eq!(
                crate::migration_progress::dirty(
                    &canvas_read(&transaction),
                    "online:other-family",
                    2
                )
                .unwrap(),
                vec![studio_db_api::migration::DirtyKey::Integer(12)]
            );
            transaction.rollback().unwrap();
        }
        assert!(
            canvas_read(&db)
                .table_exists::<crate::settings_schema::PayloadMigrations>()
                .unwrap()
        );
        assert_eq!(
            crate::migration_progress::load(&canvas_read(&db), PROGRESS_NAMES[0])
                .unwrap()
                .unwrap()
                .complete,
            0
        );
    }

    #[test]
    fn deferred_generation_keeps_progress_active_after_exhaustion() {
        use crate::online_migration::{begin, step};
        let mut db = legacy_source_canvas();
        let transaction = db.transaction().unwrap();
        Metadata::descriptor()
            .install_if_absent(&transaction)
            .unwrap();
        let plan = FamilyCopy::new(generation_plan());
        begin(&transaction, &plan).unwrap();
        step(&transaction, &plan, &FixedClock).unwrap();
        assert_eq!(
            step(&transaction, &plan, &FixedClock).unwrap(),
            crate::online_migration::Step::Ready
        );
        let progress = canvas_read(&transaction)
            .get::<crate::settings_schema::PayloadMigrations>(&(format!(
                "online:{GENERATION_PLAN_NAME}"
            ),))
            .unwrap()
            .unwrap();
        assert_eq!(
            progress.complete, 0,
            "ready is transient until the family authority switch"
        );
    }

    #[test]
    fn deferred_generation_replays_writes_after_exhaustion() {
        use crate::online_migration::{begin, step};
        let mut db = legacy_source_canvas();
        let transaction = db.transaction().unwrap();
        Metadata::descriptor()
            .install_if_absent(&transaction)
            .unwrap();
        let plan = FamilyCopy::new(generation_plan());
        begin(&transaction, &plan).unwrap();
        step(&transaction, &plan, &FixedClock).unwrap();
        step(&transaction, &plan, &FixedClock).unwrap();
        canvas_tx(&transaction)
            .upsert(&crate::clock_schema::SyncGeneration { id: 1, value: 902 })
            .unwrap();
        step(&transaction, &plan, &FixedClock).unwrap();
        let generation = canvas_read(&transaction)
            .get::<SyncGeneration>(&(encode_key(&(1_i64,)),))
            .unwrap()
            .unwrap();
        assert_eq!(
            generation.sync_generation, 902,
            "ready streams still observe source writes"
        );
    }

    #[test]
    fn dirty_copy_excludes_family_keys_and_preserves_integer_blob_and_similar_names() {
        use crate::metadata_schema::OnlineMigrationDirty as TargetDirty;
        use crate::online_migration::{begin, step};
        use crate::settings_schema::OnlineMigrationDirty as SourceDirty;
        use studio_db_api::migration::DirtyKey;
        let mut db = legacy_source_canvas();
        let transaction = db.transaction().unwrap();
        SourceDirty::descriptor()
            .install_if_absent(&transaction)
            .unwrap();
        let blob = DirtyKey::Composite(encode_key(&("key α".to_owned(), -7_i64)));
        let expected = [
            ("online:other".to_owned(), DirtyKey::Integer(-4)),
            ("online:other".to_owned(), blob.clone()),
            (
                format!("{}_suffix", PROGRESS_NAMES[0]),
                DirtyKey::Integer(7),
            ),
        ];
        for (migration, key) in &expected {
            canvas_tx(&transaction)
                .upsert(&SourceDirty {
                    migration: migration.clone(),
                    key: key.clone(),
                })
                .unwrap();
        }
        canvas_tx(&transaction)
            .upsert(&SourceDirty {
                migration: PROGRESS_NAMES[0].into(),
                key: DirtyKey::Integer(1),
            })
            .unwrap();
        let plan = dirty_copy();
        begin(&transaction, &plan).unwrap();
        canvas_tx(&transaction)
            .upsert(&SourceDirty {
                migration: PROGRESS_NAMES[1].into(),
                key: blob.clone(),
            })
            .unwrap();
        canvas_tx(&transaction)
            .upsert(&SourceDirty {
                migration: expected[0].0.clone(),
                key: expected[0].1.clone(),
            })
            .unwrap();
        let counters = canvas_read(&transaction)
            .get::<crate::settings_schema::PayloadMigrations>(&(PROGRESS_NAMES[18].into(),))
            .unwrap()
            .unwrap();
        assert_eq!(counters.copy_source_rows, Some(3));
        let mut ready = false;
        for _ in 0..32 {
            if step(&transaction, &plan, &FixedClock).unwrap()
                == crate::online_migration::Step::Ready
            {
                ready = true;
                break;
            }
        }
        assert!(ready, "dirty stream does not chase its own tracking rows");
        crate::online_migration::OnlineMigration::finish(&plan, &transaction)
            .expect("eligible dirty coverage");
        for (migration, key) in &expected {
            let row = canvas_read(&transaction)
                .get::<TargetDirty>(&(encode_key(&(migration.clone(), key.clone())),))
                .unwrap()
                .unwrap();
            assert_eq!(row.dirty_migration, *migration);
            assert_eq!(row.dirty_key, *key);
        }
        for (migration, key) in [
            (PROGRESS_NAMES[0], DirtyKey::Integer(1)),
            (PROGRESS_NAMES[1], blob),
        ] {
            assert!(
                canvas_read(&transaction)
                    .get::<TargetDirty>(&(encode_key(&(migration.to_owned(), key)),))
                    .unwrap()
                    .is_none()
            );
        }
    }

    fn ledger_row(source_rowid: i64, name: Option<String>) -> LedgerSnapshot {
        LedgerSnapshot {
            source_rowid,
            name,
            cursor: 17,
            complete: 2,
            status: "running".into(),
            updated: Some(1.25),
            error: Some("historical error".into()),
            cutover_ms: Some(43),
            cursor_key: Some(vec![]),
            copy_source_rows: Some(4),
            copy_target_rows: Some(3),
        }
    }
    fn ledger_source_count(tx: &studio_db_api::rusqlite::Transaction<'_>) -> i64 {
        canvas_read(tx)
            .get::<crate::settings_schema::PayloadMigrations>(&(PROGRESS_NAMES[7].to_owned(),))
            .unwrap()
            .unwrap()
            .copy_source_rows
            .unwrap()
    }
    #[test]
    fn ledger_excludes_own_progress_but_preserves_null_and_similar_names() {
        use crate::consolidation_codec::nullable_text_identity;
        use crate::metadata_schema::PayloadMigrations;
        use crate::online_migration::{begin, step};
        let mut db = legacy_source_canvas();
        let transaction = db.transaction().unwrap();
        let plan = ledger_copy();
        begin(&transaction, &plan).unwrap();
        let before = ledger_source_count(&transaction);
        canvas_tx(&transaction)
            .upsert(&ledger_row(-7, Some(PROGRESS_NAMES[0].to_owned())))
            .unwrap();
        assert_eq!(
            ledger_source_count(&transaction),
            before,
            "excluded insertion does not change eligible coverage"
        );
        assert_eq!(
            crate::online_migration::progress(&transaction)
                .unwrap()
                .iter()
                .find(|p| p.name == BASE_NAMES[7])
                .unwrap()
                .dirty,
            0,
            "excluded keys never enter dirty tracking"
        );
        let similar = format!("{}_suffix", PROGRESS_NAMES[0]);
        canvas_tx(&transaction)
            .upsert(&ledger_row(-6, None))
            .unwrap();
        canvas_tx(&transaction)
            .upsert(&ledger_row(-5, Some(similar.clone())))
            .unwrap();
        for _ in 0..4 {
            if step(&transaction, &plan, &FixedClock).unwrap()
                == crate::online_migration::Step::Ready
            {
                break;
            }
        }
        assert!(
            canvas_read(&transaction)
                .get::<PayloadMigrations>(&(nullable_text_identity(
                    &Some(PROGRESS_NAMES[0].to_owned()),
                    -7
                ),))
                .unwrap()
                .is_none()
        );
        for (rowid, name) in [(-6, None), (-5, Some(similar))] {
            let row = canvas_read(&transaction)
                .get::<PayloadMigrations>(&(nullable_text_identity(&name, rowid),))
                .unwrap()
                .unwrap();
            assert_eq!(row.migration_cursor, 17);
            assert_eq!(row.migration_complete.as_stored(), 2);
            assert_eq!(row.migration_cursor_key, Some(vec![]));
            assert_eq!(row.migration_status.as_str(), "running");
        }
    }
    #[test]
    fn ledger_eligibility_transitions_adjust_counts_and_replay_null_keys() {
        use crate::consolidation_codec::nullable_text_identity;
        use crate::metadata_schema::PayloadMigrations;
        use crate::online_migration::{begin, step};
        let mut db = legacy_source_canvas();
        let transaction = db.transaction().unwrap();
        let plan = ledger_copy();
        begin(&transaction, &plan).unwrap();
        let before = ledger_source_count(&transaction);
        canvas_tx(&transaction)
            .upsert(&ledger_row(-7, Some(PROGRESS_NAMES[0].to_owned())))
            .unwrap();
        canvas_tx(&transaction)
            .upsert(&ledger_row(-7, None))
            .unwrap();
        assert_eq!(
            ledger_source_count(&transaction),
            before + 1,
            "excluded to NULL becomes eligible"
        );
        for _ in 0..4 {
            step(&transaction, &plan, &FixedClock).unwrap();
        }
        assert!(
            canvas_read(&transaction)
                .get::<PayloadMigrations>(&(nullable_text_identity(&None, -7),))
                .unwrap()
                .is_some()
        );
        canvas_tx(&transaction)
            .upsert(&ledger_row(-7, Some(PROGRESS_NAMES[0].to_owned())))
            .unwrap();
        assert_eq!(
            ledger_source_count(&transaction),
            before,
            "NULL to excluded leaves eligible coverage"
        );
        for _ in 0..4 {
            step(&transaction, &plan, &FixedClock).unwrap();
        }
        assert!(
            canvas_read(&transaction)
                .get::<PayloadMigrations>(&(nullable_text_identity(&None, -7),))
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn search_copy_keeps_open_phase_cursor_and_real_timestamp() {
        use crate::metadata_schema::SearchRollout;
        let mut db = legacy_source_canvas();
        let transaction = db.transaction().unwrap();
        Metadata::descriptor()
            .install_if_absent(&transaction)
            .unwrap();
        canvas_tx(&transaction)
            .upsert(&crate::settings_schema::SearchRollout {
                id: 1,
                phase: "future rollout α".into(),
                cursor: i64::MAX,
                updated: 1.25,
            })
            .unwrap();
        search_plan().copy_tx(&mut canvas_tx(&transaction)).unwrap();
        let row = canvas_read(&transaction)
            .get::<SearchRollout>(&(encode_key(&(1_i64,)),))
            .unwrap()
            .unwrap();
        assert_eq!(row.search_phase.as_stored(), "future rollout α");
        assert_eq!(row.search_cursor, i64::MAX);
        assert_eq!(row.search_updated, 1.25);
    }

    #[test]
    fn task_window_copy_keeps_null_identity_timestamp_and_legacy_running_value() {
        use crate::consolidation_codec::nullable_text_identity;
        use crate::metadata_schema::TaskWindowState;
        let mut db = legacy_source_canvas();
        let transaction = db.transaction().unwrap();
        Metadata::descriptor()
            .install_if_absent(&transaction)
            .unwrap();
        for (source_rowid, id, created) in [
            (-4, None, Some(-0.25)),
            (0, None, None),
            (9, Some("future:task".to_owned()), Some(0.0)),
        ] {
            canvas_tx(&transaction)
                .upsert(&TaskSnapshot {
                    source_rowid,
                    id,
                    agent: "agent α".into(),
                    created,
                    running: -4,
                })
                .unwrap();
        }
        task_window_plan()
            .copy_tx(&mut canvas_tx(&transaction))
            .unwrap();
        for (source_rowid, id, created) in [
            (-4, None, Some(-0.25)),
            (0, None, None),
            (9, Some("future:task".to_owned()), Some(0.0)),
        ] {
            let row = canvas_read(&transaction)
                .get::<TaskWindowState>(&(nullable_text_identity(&id, source_rowid),))
                .unwrap()
                .unwrap();
            assert_eq!(row.task_created, created);
            assert_eq!(row.task_running.as_stored(), -4);
            assert_eq!(row.task_agent, "agent α");
        }
    }

    #[test]
    fn stored_text_batches_copy_keeps_composite_identity_and_all_receipt_values() {
        use crate::metadata_schema::StoredTextBatches;
        let mut db = legacy_source_canvas();
        let transaction = db.transaction().unwrap();
        Metadata::descriptor()
            .install_if_absent(&transaction)
            .unwrap();
        for (source_rowid, name) in [(-2, "future:column α"), (0, "future:column z")] {
            canvas_tx(&transaction)
                .upsert(&BatchSnapshot {
                    source_rowid,
                    name: name.into(),
                    batch: 7,
                    first_rowid: -3,
                    last_rowid: 9,
                    rows: 4,
                    plain_bytes: 100,
                    stored_bytes: 30,
                    sha256: "historical checksum".into(),
                    created: -0.25,
                })
                .unwrap();
        }
        batches_plan()
            .copy_tx(&mut canvas_tx(&transaction))
            .unwrap();
        for (source_rowid, name) in [(-2, "future:column α"), (0, "future:column z")] {
            let row = canvas_read(&transaction)
                .get::<StoredTextBatches>(&(encode_key(&(name.to_owned(), 7_i64)),))
                .unwrap()
                .unwrap();
            assert_eq!(row.source_rowid, source_rowid);
            assert_eq!(
                (
                    row.batch,
                    row.first_rowid,
                    row.last_rowid,
                    row.batch_rows,
                    row.plain_bytes,
                    row.stored_bytes
                ),
                (7, -3, 9, 4, 100, 30)
            );
            assert_eq!(row.sha256, "historical checksum");
            assert_eq!(row.batch_created, -0.25);
        }
    }

    #[test]
    fn settings_record_copies_preserve_exact_extensions_and_nullable_keys() {
        use crate::consolidation_codec::nullable_text_identity;
        use crate::metadata_schema::{FederationSettings, OrchestrationSettings};
        let mut db = legacy_source_canvas();
        let transaction = db.transaction().unwrap();
        Metadata::descriptor()
            .install_if_absent(&transaction)
            .unwrap();
        let federation_raw = r#" {"enabled":true,"future":{"n":1.00,"n":2}} "#;
        let orchestration_raw = r#" {"sendAdmissionCutoverMs":12,"idleCompaction":{"future":1.00},"future":{"n":1,"n":2}} "#;
        for (source_rowid, id) in [
            (-3, None),
            (0, None),
            (9, Some("future:settings".to_owned())),
        ] {
            canvas_tx(&transaction)
                .upsert(&FederationSnapshot {
                    source_rowid,
                    id: id.clone(),
                    record: crate::json_record_codec::ExactJson::from_stored(federation_raw.into())
                        .unwrap(),
                })
                .unwrap();
            canvas_tx(&transaction)
                .upsert(&OrchestrationSnapshot {
                    source_rowid,
                    id,
                    record: crate::metadata_values::OrchestrationSettingsEvidence::from_stored(
                        orchestration_raw.into(),
                    )
                    .unwrap(),
                })
                .unwrap();
        }
        federation_plan()
            .copy_tx(&mut canvas_tx(&transaction))
            .unwrap();
        orchestration_plan()
            .copy_tx(&mut canvas_tx(&transaction))
            .unwrap();
        for (source_rowid, id) in [
            (-3, None),
            (0, None),
            (9, Some("future:settings".to_owned())),
        ] {
            let identity = nullable_text_identity(&id, source_rowid);
            let federation = canvas_read(&transaction)
                .get::<FederationSettings>(&(identity.clone(),))
                .unwrap()
                .unwrap();
            let orchestration = canvas_read(&transaction)
                .get::<OrchestrationSettings>(&(identity,))
                .unwrap()
                .unwrap();
            assert_eq!(federation.federation_record.raw(), federation_raw);
            assert_eq!(
                orchestration.orchestration_record.as_raw(),
                orchestration_raw
            );
        }
    }

    #[test]
    fn sidebar_and_publication_state_keep_permissive_historical_values() {
        use crate::consolidation_codec::nullable_text_identity;
        use crate::metadata_schema::{AgentPublicationState, SidebarOrder};
        let mut db = legacy_source_canvas();
        let transaction = db.transaction().unwrap();
        Metadata::descriptor()
            .install_if_absent(&transaction)
            .unwrap();
        let raw = r#" [1,{"future":{"n":1.00,"n":2}}] "#;
        for (source_rowid, id) in [(-4, None), (0, None), (9, Some(String::new()))] {
            canvas_tx(&transaction)
                .upsert(&SidebarSnapshot {
                    source_rowid,
                    id: id.clone(),
                    record: raw.into(),
                })
                .unwrap();
            canvas_tx(&transaction)
                .upsert(&PublicationStateSnapshot {
                    source_rowid,
                    id,
                    live: 2,
                    in_flight: -3,
                    team_counts: Some("not JSON".into()),
                })
                .unwrap();
        }
        sidebar_plan()
            .copy_tx(&mut canvas_tx(&transaction))
            .unwrap();
        publication_state_plan()
            .copy_tx(&mut canvas_tx(&transaction))
            .unwrap();
        for (source_rowid, id) in [(-4, None), (0, None), (9, Some(String::new()))] {
            let identity = nullable_text_identity(&id, source_rowid);
            let sidebar = canvas_read(&transaction)
                .get::<SidebarOrder>(&(identity.clone(),))
                .unwrap()
                .unwrap();
            let publication = canvas_read(&transaction)
                .get::<AgentPublicationState>(&(identity,))
                .unwrap()
                .unwrap();
            assert_eq!(sidebar.sidebar_record.raw(), raw);
            assert_eq!(publication.publication_live.as_stored(), 2);
            assert_eq!(publication.publication_in_flight.as_stored(), -3);
            assert_eq!(
                publication.publication_team_counts.unwrap().as_raw(),
                "not JSON"
            );
        }
    }

    #[test]
    fn scheduler_summary_and_peer_settings_keep_distinct_counter_and_boolean_values() {
        use crate::metadata_schema::{PeerUpdateSettings, SchedulerAgentChangeState};
        let mut db = legacy_source_canvas();
        let transaction = db.transaction().unwrap();
        Metadata::descriptor()
            .install_if_absent(&transaction)
            .unwrap();
        canvas_tx(&transaction)
            .upsert(&crate::clock_schema::SchedulerAgentChangeState {
                id: 1,
                floor: 2,
                generation: 3,
                entries: 4,
                total: 5,
            })
            .unwrap();
        canvas_tx(&transaction)
            .upsert(&crate::peer_update_settings::PeerUpdateSettings {
                id: 1,
                auto_update: -3,
            })
            .unwrap();
        scheduler_state_plan()
            .copy_tx(&mut canvas_tx(&transaction))
            .unwrap();
        peer_settings_plan()
            .copy_tx(&mut canvas_tx(&transaction))
            .unwrap();
        let state = canvas_read(&transaction)
            .get::<SchedulerAgentChangeState>(&(encode_key(&(1_i64,)),))
            .unwrap()
            .unwrap();
        assert_eq!(
            (
                state.scheduler_floor,
                state.scheduler_generation,
                state.scheduler_entries,
                state.scheduler_total
            ),
            (2, 3, 4, 5)
        );
        let settings = canvas_read(&transaction)
            .get::<PeerUpdateSettings>(&(encode_key(&(1_i64,)),))
            .unwrap()
            .unwrap();
        assert_eq!(settings.auto_update.as_stored(), -3);
    }

    #[test]
    fn remaining_change_streams_keep_null_multiplicity_and_kind_isolation() {
        use crate::change_schema::{Changes, SchedulerAgentChanges, TranscriptRevisions};
        use crate::consolidation_codec::nullable_text_identity;
        let mut db = legacy_source_canvas();
        let transaction = db.transaction().unwrap();
        Changes::descriptor()
            .install_if_absent(&transaction)
            .unwrap();
        for (source_rowid, key) in [(-5, None), (0, None), (8, Some("agent α".to_owned()))] {
            canvas_tx(&transaction)
                .upsert(&SchedulerSnapshot {
                    source_rowid,
                    id: key.clone(),
                    generation: 17,
                })
                .unwrap();
            canvas_tx(&transaction)
                .upsert(&RevisionSnapshot {
                    source_rowid,
                    agent: key,
                    revision: -9,
                })
                .unwrap();
        }
        scheduler_changes_plan()
            .copy_tx(&mut canvas_tx(&transaction))
            .unwrap();
        revisions_plan()
            .copy_tx(&mut canvas_tx(&transaction))
            .unwrap();
        for (source_rowid, key) in [(-5, None), (0, None), (8, Some("agent α".to_owned()))] {
            let identity = nullable_text_identity(&key, source_rowid);
            let scheduler = canvas_read(&transaction)
                .get::<SchedulerAgentChanges>(&(identity.clone(),))
                .unwrap()
                .unwrap();
            let revision = canvas_read(&transaction)
                .get::<TranscriptRevisions>(&(identity,))
                .unwrap()
                .unwrap();
            assert_eq!(scheduler.source_rowid, source_rowid);
            assert_eq!(revision.source_rowid, source_rowid);
            assert_eq!(scheduler.original_key, key);
            assert_eq!(revision.original_key, key);
            assert_eq!(scheduler.generation, 17);
            assert_eq!(revision.revision, -9);
        }
    }

    #[test]
    fn durable_copy_keeps_rowid_order_ties_and_exact_scope_evidence() {
        use crate::change_schema::{Changes, DurableRows};
        use crate::json_record_codec::ExactJson;
        let mut db = legacy_source_canvas();
        let transaction = db.transaction().unwrap();
        Changes::descriptor()
            .install_if_absent(&transaction)
            .unwrap();
        let raw = r#" {"agent":"a","future":{"n":1.00,"n":2}} "#;
        for (source_rowid, id) in [(-2, "first"), (0, "second")] {
            canvas_tx(&transaction)
                .upsert(&DurableSnapshot {
                    source_rowid,
                    tbl: "future:kind".into(),
                    id: id.into(),
                    seq: 42,
                    deleted: 2,
                    at: -4,
                    scope: Some(ExactJson::from_stored(raw.into()).unwrap()),
                })
                .unwrap();
        }
        durable_plan()
            .copy_tx(&mut canvas_tx(&transaction))
            .unwrap();
        for (source_rowid, id) in [(-2, "first"), (0, "second")] {
            let row = canvas_read(&transaction)
                .get::<DurableRows>(&(encode_key(&("future:kind".to_owned(), id.to_owned())),))
                .unwrap()
                .unwrap();
            assert_eq!(row.source_rowid, source_rowid);
            assert_eq!(row.sequence, 42);
            assert_eq!(row.deleted.as_stored(), 2);
            assert_eq!(row.deleted_at, -4);
            assert_eq!(row.scope.unwrap().raw(), raw);
        }
    }

    #[test]
    fn watermark_copy_preserves_without_rowid_keys_and_maximum_applied_value() {
        use crate::metadata_schema::ClientCommandWatermarks;
        let mut db = legacy_source_canvas();
        let transaction = db.transaction().unwrap();
        Metadata::descriptor()
            .install_if_absent(&transaction)
            .unwrap();
        for (client, applied) in [("client α", i64::MAX), ("client z", 0)] {
            canvas_tx(&transaction)
                .upsert(&crate::settings_schema::ClientCommandWatermarks {
                    client: client.into(),
                    applied,
                })
                .unwrap();
        }
        watermarks_plan()
            .copy_tx(&mut canvas_tx(&transaction))
            .unwrap();
        for (client, applied) in [("client α", i64::MAX), ("client z", 0)] {
            let row = canvas_read(&transaction)
                .get::<ClientCommandWatermarks>(&(encode_key(&(client.to_owned(),)),))
                .unwrap()
                .unwrap();
            assert_eq!(row.text_key, client);
            assert_eq!(row.applied, applied);
        }
    }

    #[test]
    fn sync_meta_copy_preserves_null_key_and_exact_historical_text() {
        use crate::consolidation_codec::nullable_text_identity;
        use crate::metadata_schema::SyncEntityMeta;
        let mut db = legacy_source_canvas();
        let transaction = db.transaction().unwrap();
        Metadata::descriptor()
            .install_if_absent(&transaction)
            .unwrap();
        for (source_rowid, key, value) in [
            (-8, None, "not JSON"),
            (-7, Some("future:key".to_owned()), "00042"),
        ] {
            canvas_tx(&transaction)
                .upsert(&MetaSnapshot {
                    source_rowid,
                    key,
                    value: value.into(),
                })
                .unwrap();
        }
        sync_meta_plan()
            .copy_tx(&mut canvas_tx(&transaction))
            .unwrap();
        for (source_rowid, key, value) in [
            (-8, None, "not JSON"),
            (-7, Some("future:key".to_owned()), "00042"),
        ] {
            let row = canvas_read(&transaction)
                .get::<SyncEntityMeta>(&(nullable_text_identity(&key, source_rowid),))
                .unwrap()
                .unwrap();
            assert_eq!(row.source_rowid, source_rowid);
            assert_eq!(row.text_key, key);
            assert_eq!(row.meta_value.as_stored(), value);
        }
    }

    #[test]
    fn nullable_identity_copy_keeps_distinct_null_rows_and_signed_rowids() {
        use crate::consolidation_codec::nullable_text_identity;
        use crate::metadata_schema::SyncIdentity;
        let mut db = legacy_source_canvas();
        let transaction = db.transaction().unwrap();
        Metadata::descriptor()
            .install_if_absent(&transaction)
            .unwrap();
        for (source_rowid, id) in [(-2, None), (0, None), (8, Some("identity α".to_owned()))] {
            canvas_tx(&transaction)
                .upsert(&IdentitySnapshot { source_rowid, id })
                .unwrap();
        }
        use crate::online_migration::{begin, step};
        let plan = FamilyCopy::new(sync_identity_plan());
        begin(&transaction, &plan).unwrap();
        step(&transaction, &plan, &FixedClock).unwrap();
        canvas_tx(&transaction)
            .upsert(&IdentitySnapshot {
                source_rowid: 0,
                id: None,
            })
            .unwrap();
        step(&transaction, &plan, &FixedClock).expect("NULL dirty key lookup");
        crate::online_migration::OnlineMigration::finish(&plan, &transaction)
            .expect("NULL replay retains coverage");
        for (source_rowid, id) in [(-2, None), (0, None), (8, Some("identity α".to_owned()))] {
            let row = canvas_read(&transaction)
                .get::<SyncIdentity>(&(nullable_text_identity(&id, source_rowid),))
                .unwrap()
                .unwrap();
            assert_eq!(row.source_rowid, source_rowid);
            assert_eq!(row.text_key, id);
        }
    }

    #[test]
    fn access_metadata_plans_preserve_alias_and_legacy_boolean_integer() {
        use crate::metadata_schema::{AccessLocalAlias, AccessSettings};
        let mut db = legacy_source_canvas();
        let transaction = db.transaction().unwrap();
        Metadata::descriptor()
            .install_if_absent(&transaction)
            .unwrap();
        canvas_tx(&transaction)
            .upsert(&crate::settings_schema::AccessLocalAlias {
                id: 1,
                alias: "local alias α".into(),
            })
            .unwrap();
        canvas_tx(&transaction)
            .upsert(&crate::settings_schema::AccessSettings {
                id: 1,
                auto_pair: -2,
            })
            .unwrap();
        alias_plan().copy_tx(&mut canvas_tx(&transaction)).unwrap();
        access_plan().copy_tx(&mut canvas_tx(&transaction)).unwrap();
        assert_eq!(
            canvas_read(&transaction)
                .get::<AccessLocalAlias>(&(encode_key(&(1_i64,)),))
                .unwrap()
                .unwrap()
                .alias,
            "local alias α"
        );
        assert_eq!(
            canvas_read(&transaction)
                .get::<AccessSettings>(&(encode_key(&(1_i64,)),))
                .unwrap()
                .unwrap()
                .auto_pair
                .as_stored(),
            -2
        );
    }

    #[test]
    fn integer_counter_plans_keep_their_own_partitions() {
        let mut db = legacy_source_canvas();
        let transaction = db.transaction().unwrap();
        Metadata::descriptor()
            .install_if_absent(&transaction)
            .unwrap();
        canvas_tx(&transaction)
            .upsert(&crate::clock_schema::DurableFloor { id: 1, floor: 19 })
            .unwrap();
        canvas_tx(&transaction)
            .upsert(&crate::clock_schema::AgentRecordGeneration { id: 1, value: 27 })
            .unwrap();
        floor_plan().copy_tx(&mut canvas_tx(&transaction)).unwrap();
        agent_generation_plan()
            .copy_tx(&mut canvas_tx(&transaction))
            .unwrap();
        assert_eq!(
            canvas_read(&transaction)
                .get::<DurableFloor>(&(encode_key(&(1_i64,)),))
                .unwrap()
                .unwrap()
                .durable_floor,
            19
        );
        assert_eq!(
            canvas_read(&transaction)
                .get::<crate::metadata_schema::AgentRecordGeneration>(&(encode_key(&(1_i64,)),))
                .unwrap()
                .unwrap()
                .agent_record_generation,
            27
        );
    }

    #[test]
    fn document_copy_preserves_exact_payload_and_legacy_deleted_integer() {
        use crate::publication_schema::{Publications, SyncDocuments};
        let mut db = legacy_source_canvas();
        let transaction = db.transaction().unwrap();
        Publications::descriptor()
            .install_if_absent(&transaction)
            .unwrap();
        let raw = r#" {"session":"s","device":"d","text":"draft","updated":1,"future":{"n":1.00,"n":2}} "#;
        canvas_tx(&transaction)
            .upsert(&crate::clock_schema::SyncDocuments {
                seq: 900,
                scope: "drafts".into(),
                id: "d".into(),
                payload: raw.into(),
                deleted: 2,
            })
            .unwrap();
        documents_plan()
            .copy_tx(&mut canvas_tx(&transaction))
            .unwrap();
        let row = canvas_read(&transaction)
            .get::<SyncDocuments>(&(encode_key(&(900_i64,)),))
            .unwrap()
            .unwrap();
        assert_eq!(row.document_payload.value().raw(), raw);
        assert_eq!(row.deleted.as_stored(), 2);
        assert_eq!(row.source_rowid, 900);
    }

    #[test]
    fn document_replay_replaces_scope_identity_before_obsolete_sequence_replay() {
        use crate::online_migration::{begin, step};
        use crate::publication_schema::{Publications, SyncDocuments};
        let mut db = legacy_source_canvas();
        let transaction = db.transaction().unwrap();
        Publications::descriptor()
            .install_if_absent(&transaction)
            .unwrap();
        let source = |seq| crate::clock_schema::SyncDocuments {
            seq,
            scope: "future".into(),
            id: "same-id".into(),
            payload: "{}".into(),
            deleted: 0,
        };
        canvas_tx(&transaction).upsert(&source(900)).unwrap();
        let plan = documents_copy();
        begin(&transaction, &plan).unwrap();
        step(&transaction, &plan, &FixedClock).unwrap();
        canvas_tx(&transaction)
            .delete::<crate::clock_schema::SyncDocuments>(&(900,))
            .unwrap();
        canvas_tx(&transaction).upsert(&source(100)).unwrap();
        step(&transaction, &plan, &FixedClock)
            .expect("replacement may replay before obsolete sequence");
        crate::online_migration::OnlineMigration::finish(&plan, &transaction)
            .expect("reconciled coverage counts");
        assert!(
            canvas_read(&transaction)
                .get::<SyncDocuments>(&(encode_key(&(900_i64,)),))
                .unwrap()
                .is_none()
        );
        assert!(
            canvas_read(&transaction)
                .get::<SyncDocuments>(&(encode_key(&(100_i64,)),))
                .unwrap()
                .is_some()
        );
    }

    #[test]
    fn entity_replay_replaces_a_reused_physical_rowid_before_old_key_replay() {
        use crate::online_migration::{begin, step};
        use crate::publication_schema::{Publications, SyncEntities};
        let mut db = legacy_source_canvas();
        let transaction = db.transaction().unwrap();
        Publications::descriptor()
            .install_if_absent(&transaction)
            .unwrap();
        let source = |collection: &str| EntitySource {
            source_rowid: 1,
            collection: collection.into(),
            id: "same-id".into(),
            seq: 42,
            hash: "historical-hash".into(),
            payload: Some("{}".into()),
            deleted: 0,
        };
        canvas_tx(&transaction).upsert(&source("future:z")).unwrap();
        let plan = entities_copy();
        begin(&transaction, &plan).unwrap();
        step(&transaction, &plan, &FixedClock).unwrap();
        canvas_tx(&transaction)
            .delete::<EntitySource>(&("future:z".into(), "same-id".into()))
            .unwrap();
        canvas_tx(&transaction).upsert(&source("future:a")).unwrap();
        step(&transaction, &plan, &FixedClock).expect("new key may replay before old key");
        crate::online_migration::OnlineMigration::finish(&plan, &transaction)
            .expect("reconciled coverage counts");
        assert!(
            canvas_read(&transaction)
                .get::<SyncEntities>(&(encode_key(&("future:z".to_owned(), "same-id".to_owned())),))
                .unwrap()
                .is_none()
        );
        assert!(
            canvas_read(&transaction)
                .get::<SyncEntities>(&(encode_key(&("future:a".to_owned(), "same-id".to_owned())),))
                .unwrap()
                .is_some()
        );
    }

    #[test]
    fn entity_copy_preserves_rowid_ties_and_exact_open_payload() {
        use crate::publication_schema::{Publications, SyncEntities};
        let mut db = legacy_source_canvas();
        let transaction = db.transaction().unwrap();
        Publications::descriptor()
            .install_if_absent(&transaction)
            .unwrap();
        let raw = r#" {"future":{"n":1.00,"n":2}} "#;
        for (rowid, collection) in [(-7, "future:a"), (0, "future:b")] {
            canvas_tx(&transaction)
                .upsert(&EntitySource {
                    source_rowid: rowid,
                    collection: collection.into(),
                    id: "same-id".into(),
                    seq: 42,
                    hash: "historical-hash".into(),
                    payload: Some(raw.into()),
                    deleted: 2,
                })
                .unwrap();
        }
        entities_plan()
            .copy_tx(&mut canvas_tx(&transaction))
            .unwrap();
        for (rowid, collection) in [(-7, "future:a"), (0, "future:b")] {
            let row = canvas_read(&transaction)
                .get::<SyncEntities>(&(encode_key(&(collection.to_owned(), "same-id".to_owned())),))
                .unwrap()
                .unwrap();
            assert_eq!(row.source_rowid, rowid);
            assert_eq!(row.publication_seq, 42);
            assert_eq!(row.entity_payload.value().raw(), Some(raw));
            assert_eq!(row.deleted.as_stored(), 2);
        }
    }

    #[test]
    fn generation_copy_keeps_other_metadata_partitions_and_exact_high() {
        let mut db = legacy_source_canvas();
        let transaction = db.transaction().unwrap();
        Metadata::descriptor()
            .install_if_absent(&transaction)
            .unwrap();
        canvas_tx(&transaction)
            .upsert(&crate::clock_schema::SyncGeneration { id: 1, value: 901 })
            .unwrap();
        canvas_tx(&transaction)
            .partition::<DurableFloor>()
            .put(&DurableFloor {
                identity: encode_key(&(1_i64,)),
                source_rowid: 1,
                integer_key: 1,
                durable_floor: 17,
            })
            .unwrap();
        generation_plan()
            .copy_tx(&mut canvas_tx(&transaction))
            .unwrap();
        let generation = canvas_read(&transaction)
            .get::<SyncGeneration>(&(encode_key(&(1_i64,)),))
            .unwrap()
            .unwrap();
        assert_eq!(generation.sync_generation, 901);
        assert_eq!(generation.source_rowid, 1);
        assert_eq!(
            canvas_read(&transaction)
                .get::<DurableFloor>(&(encode_key(&(1_i64,)),))
                .unwrap()
                .unwrap()
                .durable_floor,
            17
        );
        assert_eq!(
            canvas_read(&transaction)
                .get::<crate::clock_schema::SyncGeneration>(&(1,))
                .unwrap()
                .unwrap()
                .value,
            901,
            "copy preparation retains source writer authority"
        );
    }
}
