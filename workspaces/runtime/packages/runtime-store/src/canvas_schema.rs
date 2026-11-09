//! Canvas table registry; each descriptor has one family owner.
use crate::peer_update_settings::{Canvas, PeerUpdateSettings};
use crate::work_history::{WorkDecisionsStorageRow, WorkReleasesStorageRow, WorkResultsStorageRow};
use studio_db_api::db_schema;

pub(crate) fn registry() -> [studio_db_api::Registration; 12] {
    db_schema![Canvas;
        crate::interaction_tables::ApprovalsRow,
        crate::interaction_tables::GraphEdgesRow,
        crate::interaction_tables::GroupsRow,
        crate::interaction_tables::MessagesRow,
        crate::interaction_tables::ChatMessagesRow,
        crate::interaction_tables::ComplaintRow,
        PeerUpdateSettings,
        crate::interaction_tables::RequestRow,
        crate::interaction_tables::RoomRow,
        WorkDecisionsStorageRow,
        WorkReleasesStorageRow,
        WorkResultsStorageRow,
    ]
}
