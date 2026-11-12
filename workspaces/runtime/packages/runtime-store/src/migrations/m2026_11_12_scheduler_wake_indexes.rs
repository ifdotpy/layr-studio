//! Sparse wake candidates reuse existing agent, chat and turn columns.
use crate::{StoreResult, Transaction};

pub(super) const DEFINITION: super::MigrationDefinition = super::MigrationDefinition {
    id: "2026-11-12-scheduler-wake-indexes",
    introduced: super::MigrationDate {
        year: 2026,
        month: 11,
        day: 12,
    },
    target: studio_runtime_schema::studio_errors::MigrationTarget::Canvas,
    supported_source: "Consolidated scheduler wake selection scans idle agents",
    remove_when: "Supported installations and backups all contain sparse wake indexes",
};

pub(super) fn apply(tx: &Transaction<'_>) -> StoreResult<()> {
    crate::SchedulerRepository::prepare_wake_eligibility_indexes(tx)
}
