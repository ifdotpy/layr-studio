//! Permanent source-kind codes for the three F1 Canvas destinations.
//! These codes identify source partitions; they never define textual query order.
use studio_db_api::DbEnum;

#[derive(Clone, Copy, Debug, PartialEq, Eq, DbEnum)]
#[repr(i64)]
pub enum CanvasMetadataKind {
    ClientCommandWatermarks = 1,
    DurableFloor = 2,
    AccessLocalAlias = 3,
    AccessSettings = 4,
    AgentRecordGeneration = 5,
    FederationSettings = 6,
    OrchestrationSettings = 7,
    PayloadMigrations = 8,
    SchedulerAgentChangeState = 9,
    SearchRollout = 10,
    SidebarOrder = 11,
    StoredTextBatches = 12,
    SyncEntityMeta = 13,
    SyncGeneration = 14,
    SyncIdentity = 15,
    TaskWindowState = 16,
    AgentPublicationState = 17,
    PeerUpdateSettings = 18,
    OnlineMigrationDirty = 19,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, DbEnum)]
#[repr(i64)]
pub enum CanvasChangesKind {
    DurableRows = 1,
    SchedulerAgentChanges = 2,
    TranscriptRevisions = 3,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, DbEnum)]
#[repr(i64)]
pub enum CanvasPublicationsKind {
    SyncDocuments = 1,
    SyncEntities = 2,
}
