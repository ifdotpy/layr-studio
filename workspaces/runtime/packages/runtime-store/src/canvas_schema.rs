//! Canvas table registry; each descriptor has one family owner.
use crate::peer_update_settings::{Canvas, PeerUpdateSettings};
use studio_db_api::db_schema;

pub(crate) fn registry() -> [studio_db_api::Registration; 1] {
    db_schema![Canvas;
        PeerUpdateSettings,
    ]
}
