//! Canvas table registry; each descriptor has one family owner.
use crate::peer_update_settings::{Canvas, PeerUpdateSettings};
use crate::work_history::{WorkDecisionsStorageRow, WorkReleasesStorageRow, WorkResultsStorageRow};
use studio_db_api::db_schema;

pub(crate) fn registry() -> [studio_db_api::Registration; 4] {
    db_schema![Canvas;
        PeerUpdateSettings,
        WorkDecisionsStorageRow,
        WorkReleasesStorageRow,
        WorkResultsStorageRow,
    ]
}
