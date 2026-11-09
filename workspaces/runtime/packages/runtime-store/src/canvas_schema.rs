//! Canvas table registry; each descriptor has one family owner.
use crate::peer_update_settings::{Canvas, PeerUpdateSettings};
use crate::work_history::{WorkDecisionsStorageRow, WorkReleasesStorageRow, WorkResultsStorageRow};
use studio_db_api::db_schema;

/// Install only the explicitly requested legacy owners, preserving existing rows.
pub(crate) fn install(
    connection: &rusqlite::Connection,
    descriptors: &[studio_db_api::Descriptor],
) -> rusqlite::Result<()> {
    for descriptor in descriptors {
        descriptor.install_if_absent(connection)?;
    }
    Ok(())
}

pub(crate) fn registry() -> [studio_db_api::Registration; 40] {
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
        crate::voice_storage::VoiceApprovalRow,
        crate::voice_storage::VoiceDeliveryRow,
        crate::voice_storage::VoiceRecordRow,
        crate::voice_storage::VoiceSessionRow,
        crate::voice_storage::VoiceSpeechRequestRow,
        WorkDecisionsStorageRow,
        WorkReleasesStorageRow,
        WorkResultsStorageRow,
        crate::project_grants::GrantRow,
        crate::access_tables::AccessAliasRequestsRow,
        crate::access_tables::AccessAuditRow,
        crate::access_tables::AccessAutoAttemptsRow,
        crate::access_tables::AccessClientsRow,
        crate::access_tables::AccessDiscoveredRow,
        crate::access_tables::AccessInvitesRow,
        crate::access_tables::AccessNameRequestsRow,
        crate::access_tables::AccessNameTargetsRow,
        crate::access_tables::AccessNoncesRow,
        crate::access_tables::AccessUnrevokesRow,
        crate::access_tables::FederationIdentityRow,
        crate::access_tables::FederationInboxRow,
        crate::access_tables::FederationInvitesRow,
        crate::access_tables::FederationPeersRow,
        crate::access_tables::FederationRoomsRow,
        crate::access_tables::ServerExecRow,
        crate::access_tables::ServerExecAuditRow,
        crate::access_tables::ServerExecInputRow,
        crate::access_tables::ServerInboxRow,
        crate::access_tables::ServerLinksRow,
        crate::access_tables::ServerOutputToolsRow,
        crate::access_tables::ServerRetiredRow,
    ]
}
