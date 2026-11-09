//! Physical-format-preserving access and federation table contracts.
use super::access_payloads::*;
use crate::json_record_codec::{ExactJson, ExactJsonCodec};
use crate::peer_update_settings::Canvas;
use crate::peer_update_settings::canvas_tx;
use studio_db_api::DbTable;

crate::json_record_codec::exact_json_row_writes!(AccessClientsRow { insert => insert, put => upsert });
crate::json_record_codec::exact_json_row_writes!(AccessAutoAttemptsRow { put => upsert });
crate::json_record_codec::exact_json_row_writes!(AccessDiscoveredRow { put => upsert });
crate::json_record_codec::exact_json_row_writes!(FederationIdentityRow { insert => insert });
crate::json_record_codec::exact_json_row_writes!(FederationInvitesRow { insert => insert });

impl AccessAuditRow {
    pub fn append(
        tx: &rusqlite::Transaction<'_>,
        client: &str,
        actor: &str,
        action: &str,
        created: f64,
    ) -> rusqlite::Result<i64> {
        canvas_tx(tx).insert_rowid(&Self {
            seq: 0,
            client: client.into(),
            actor: actor.into(),
            action: action.into(),
            created,
        })
    }
}

impl AccessClientsRow {
    pub fn update(
        tx: &rusqlite::Transaction<'_>,
        id: &str,
        raw: String,
    ) -> crate::StoreResult<usize> {
        let record: ExactJson<AccessClientPayload> = ExactJson::from_stored(raw)?;
        Ok(studio_db_api::db_update!(
            canvas_tx(tx),
            AccessClientsRow,
            set(AccessClientsRow::RECORD = &record),
            filter(AccessClientsRow::ID.eq(&id.to_owned()))
        )?)
    }

    pub fn delete(tx: &rusqlite::Transaction<'_>, id: &str) -> rusqlite::Result<usize> {
        canvas_tx(tx).delete::<Self>(&(id.into(),))
    }
}

impl AccessAutoAttemptsRow {
    pub fn delete(tx: &rusqlite::Transaction<'_>, id: &str) -> rusqlite::Result<usize> {
        canvas_tx(tx).delete::<Self>(&(id.into(),))
    }
}

impl AccessAliasRequestsRow {
    pub fn get(tx: &rusqlite::Transaction<'_>, id: &str) -> rusqlite::Result<Option<Self>> {
        canvas_tx(tx).get::<Self>(&(id.into(),))
    }
    pub fn insert(
        tx: &rusqlite::Transaction<'_>,
        id: &str,
        server: &str,
        alias: &str,
    ) -> rusqlite::Result<()> {
        canvas_tx(tx).insert(&Self {
            id: id.into(),
            server: server.into(),
            alias: alias.into(),
        })
    }
}

impl AccessUnrevokesRow {
    pub fn get(tx: &rusqlite::Transaction<'_>, id: &str) -> rusqlite::Result<Option<Self>> {
        canvas_tx(tx).get::<Self>(&(id.into(),))
    }
    pub fn insert(tx: &rusqlite::Transaction<'_>, id: &str, client: &str) -> rusqlite::Result<()> {
        canvas_tx(tx).insert(&Self {
            id: id.into(),
            client: client.into(),
        })
    }
}

impl AccessInvitesRow {
    pub fn insert(
        tx: &rusqlite::Transaction<'_>,
        id: &str,
        request_id: &str,
        hash: &str,
        raw: String,
    ) -> crate::StoreResult<()> {
        canvas_tx(tx).insert(&Self {
            id: id.into(),
            request_id: request_id.into(),
            hash: hash.into(),
            record: ExactJson::from_stored(raw)?,
        })?;
        Ok(())
    }
}

impl ServerExecRow {
    pub fn claim(
        tx: &rusqlite::Transaction<'_>,
        id: &str,
        owner: &str,
        actor: &str,
        raw: String,
    ) -> crate::StoreResult<()> {
        canvas_tx(tx).insert(&Self {
            id: id.into(),
            owner: owner.into(),
            actor: actor.into(),
            record: ExactJson::from_stored(raw)?,
        })?;
        Ok(())
    }

    pub fn delete(tx: &rusqlite::Transaction<'_>, id: &str) -> rusqlite::Result<usize> {
        canvas_tx(tx).delete::<Self>(&(id.into(),))
    }
}

impl ServerExecAuditRow {
    pub fn insert(&self, tx: &rusqlite::Transaction<'_>) -> rusqlite::Result<()> {
        canvas_tx(tx).insert(self)
    }

    pub fn finish(
        tx: &rusqlite::Transaction<'_>,
        id: &str,
        end: Option<f64>,
        exit_code: Option<i64>,
        signal: Option<i64>,
    ) -> rusqlite::Result<usize> {
        studio_db_api::db_update!(
            canvas_tx(tx),
            ServerExecAuditRow,
            set(
                ServerExecAuditRow::END = &end,
                ServerExecAuditRow::EXIT_CODE = &exit_code,
                ServerExecAuditRow::SIGNAL = &signal
            ),
            filter(ServerExecAuditRow::ID.eq(&id.to_owned()))
        )
    }

    pub fn expire(tx: &rusqlite::Transaction<'_>, cutoff: f64) -> rusqlite::Result<usize> {
        studio_db_api::db_delete!(
            canvas_tx(tx),
            ServerExecAuditRow,
            filter(ServerExecAuditRow::END.lt(&Some(cutoff)))
        )
    }
}

impl ServerExecInputRow {
    pub fn claim(tx: &rusqlite::Transaction<'_>, id: &str, handle: &str) -> rusqlite::Result<()> {
        canvas_tx(tx).insert(&Self {
            id: id.into(),
            handle: handle.into(),
        })
    }

    pub fn delete_handle(tx: &rusqlite::Transaction<'_>, handle: &str) -> rusqlite::Result<usize> {
        studio_db_api::db_delete!(
            canvas_tx(tx),
            ServerExecInputRow,
            filter(ServerExecInputRow::HANDLE.eq(&handle.to_owned()))
        )
    }
}

impl ServerOutputToolsRow {
    pub fn remember(
        tx: &rusqlite::Transaction<'_>,
        id: &str,
        expires: f64,
    ) -> rusqlite::Result<bool> {
        canvas_tx(tx).insert_new(&Self {
            id: id.into(),
            expires,
        })
    }
}

impl ServerInboxRow {
    pub fn claim(
        tx: &rusqlite::Transaction<'_>,
        id: &str,
        signature: &str,
        created_at: i64,
        principal: &str,
        actor: Option<&str>,
        admission: String,
    ) -> crate::StoreResult<()> {
        let state = studio_runtime_schema::persistence::ServerDeliveryState::Running
            .as_str()
            .to_owned();
        canvas_tx(tx).insert(&Self {
            id: id.into(),
            signature: signature.into(),
            state: state.clone(),
            result: None,
            // Preserve the existing REAL-affinity conversion of the caller's integer clock.
            created_at: created_at as f64,
            completed_at: 0.0,
            principal: Some(principal.into()),
            actor: actor.map(str::to_owned),
            admission: Some(ExactJson::from_stored(admission)?),
            receipt_state: state,
        })?;
        Ok(())
    }

    pub fn wait_if_running(tx: &rusqlite::Transaction<'_>, id: &str) -> rusqlite::Result<usize> {
        use studio_runtime_schema::persistence::ServerDeliveryState;
        studio_db_api::db_update!(
            canvas_tx(tx),
            ServerInboxRow,
            set(ServerInboxRow::STATE = &ServerDeliveryState::Waiting.as_str().to_owned()),
            filter(
                ServerInboxRow::ID.eq(&id.to_owned()),
                ServerInboxRow::STATE.eq(&ServerDeliveryState::Running.as_str().to_owned())
            )
        )
    }

    pub fn delete(tx: &rusqlite::Transaction<'_>, id: &str) -> rusqlite::Result<usize> {
        canvas_tx(tx).delete::<Self>(&(id.into(),))
    }
}

#[derive(DbTable)]
#[db(table = "runtime_access_alias_requests", database = Canvas, state_check = crate::state_check::count_legacy_rows::<AccessAliasRequestsRow>, legacy_ddl = "CREATE TABLE runtime_access_alias_requests (\n          id TEXT PRIMARY KEY, server TEXT NOT NULL, alias TEXT NOT NULL)")]
pub struct AccessAliasRequestsRow {
    #[db(key)]
    pub id: String,
    pub server: String,
    pub alias: String,
}

#[derive(DbTable)]
#[db(table = "runtime_access_audit", database = Canvas, state_check = crate::state_check::count_legacy_rows::<AccessAuditRow>, legacy_ddl = "CREATE TABLE runtime_access_audit (\n          seq INTEGER PRIMARY KEY AUTOINCREMENT, client TEXT NOT NULL, actor TEXT NOT NULL,\n          action TEXT NOT NULL, created REAL NOT NULL)")]
pub struct AccessAuditRow {
    #[db(key, generated)]
    pub seq: i64,
    pub client: String,
    pub actor: String,
    pub action: String,
    pub created: f64,
}

#[derive(DbTable)]
#[db(table = "runtime_access_auto_attempts", database = Canvas, state_check = crate::state_check::count_legacy_rows::<AccessAutoAttemptsRow>, legacy_ddl = "CREATE TABLE runtime_access_auto_attempts(id TEXT PRIMARY KEY, record TEXT NOT NULL)")]
pub struct AccessAutoAttemptsRow {
    #[db(key)]
    pub id: String,
    #[db(codec = ExactJsonCodec<AccessAutoAttemptPayload>)]
    pub record: ExactJson<AccessAutoAttemptPayload>,
}

#[derive(DbTable)]
#[db(table = "runtime_access_clients", database = Canvas, state_check = crate::multi_server_state_check::check_access_clients, legacy_ddl = "CREATE TABLE runtime_access_clients (id TEXT PRIMARY KEY, record TEXT NOT NULL)")]
pub struct AccessClientsRow {
    #[db(key)]
    pub id: String,
    #[db(codec = ExactJsonCodec<AccessClientPayload>)]
    pub record: ExactJson<AccessClientPayload>,
}

#[derive(DbTable)]
#[db(table = "runtime_access_discovered", database = Canvas, state_check = crate::multi_server_state_check::check_access_discovered, legacy_ddl = "CREATE TABLE runtime_access_discovered(id TEXT PRIMARY KEY, record TEXT NOT NULL)")]
pub struct AccessDiscoveredRow {
    #[db(key)]
    pub id: String,
    #[db(codec = ExactJsonCodec<AccessClientPayload>)]
    pub record: ExactJson<AccessClientPayload>,
}

#[derive(DbTable)]
#[db(table = "runtime_access_invites", database = Canvas, state_check = crate::multi_server_state_check::check_access_invites, legacy_ddl = "CREATE TABLE runtime_access_invites (\n          id TEXT PRIMARY KEY, request_id TEXT NOT NULL UNIQUE, hash TEXT NOT NULL, record TEXT NOT NULL)")]
pub struct AccessInvitesRow {
    #[db(key)]
    pub id: String,
    pub request_id: String,
    pub hash: String,
    #[db(codec = ExactJsonCodec<AccessInvitePayload>)]
    pub record: ExactJson<AccessInvitePayload>,
}

#[derive(DbTable)]
#[db(table = "runtime_access_name_requests", database = Canvas, state_check = crate::state_check::count_legacy_rows::<AccessNameRequestsRow>, legacy_ddl = "CREATE TABLE runtime_access_name_requests (\n          id TEXT PRIMARY KEY, label TEXT NOT NULL, actor TEXT NOT NULL, applied INTEGER NOT NULL)")]
pub struct AccessNameRequestsRow {
    #[db(key)]
    pub id: String,
    pub label: String,
    pub actor: String,
    pub applied: i64,
}

#[derive(DbTable)]
#[db(table = "runtime_access_name_targets", database = Canvas, state_check = crate::state_check::count_legacy_rows::<AccessNameTargetsRow>, legacy_ddl = "CREATE TABLE runtime_access_name_targets (\n          id TEXT PRIMARY KEY, server TEXT NOT NULL, label TEXT NOT NULL)")]
pub struct AccessNameTargetsRow {
    #[db(key)]
    pub id: String,
    pub server: String,
    pub label: String,
}

#[derive(DbTable)]
#[db(table = "runtime_access_nonces", database = Canvas, state_check = crate::state_check::count_legacy_rows::<AccessNoncesRow>, legacy_ddl = "CREATE TABLE runtime_access_nonces (\n          client TEXT NOT NULL, nonce TEXT NOT NULL, created INTEGER NOT NULL, PRIMARY KEY(client,nonce))", legacy_index = "CREATE INDEX runtime_access_nonce_age ON runtime_access_nonces(created)")]
pub struct AccessNoncesRow {
    #[db(key)]
    pub client: String,
    #[db(key)]
    pub nonce: String,
    pub created: i64,
}

#[derive(DbTable)]
#[db(table = "runtime_access_unrevokes", database = Canvas, state_check = crate::state_check::count_legacy_rows::<AccessUnrevokesRow>, legacy_ddl = "CREATE TABLE runtime_access_unrevokes(id TEXT PRIMARY KEY, client TEXT NOT NULL)")]
pub struct AccessUnrevokesRow {
    #[db(key)]
    pub id: String,
    pub client: String,
}

#[derive(DbTable)]
#[db(table = "runtime_federation_identity", database = Canvas, state_check = crate::state_check::count_legacy_rows::<FederationIdentityRow>, legacy_ddl = "CREATE TABLE runtime_federation_identity (\n          id TEXT PRIMARY KEY, record TEXT NOT NULL)")]
pub struct FederationIdentityRow {
    #[db(key)]
    pub id: String,
    #[db(codec = ExactJsonCodec<FederationIdentityPayload>, private)]
    pub record: ExactJson<FederationIdentityPayload>,
}

#[derive(DbTable)]
#[db(table = "runtime_federation_inbox", database = Canvas, state_check = check_federation_inbox, legacy_ddl = "CREATE TABLE runtime_federation_inbox (\n          peer_id TEXT NOT NULL, message_id TEXT NOT NULL, record TEXT NOT NULL, receipt_ack TEXT GENERATED ALWAYS AS (json_extract(record,'$.ack')) VIRTUAL NOT NULL CHECK(\"receipt_ack\" IN ('pending','sent','confirmed')), receipt_kind TEXT GENERATED ALWAYS AS (json_extract(record,'$.kind')) VIRTUAL  CHECK(\"receipt_kind\" IN ('chat-message','room-invite','room-accept')),\n          PRIMARY KEY(peer_id,message_id))")]
pub struct FederationInboxRow {
    #[db(key)]
    pub peer_id: String,
    #[db(key)]
    pub message_id: String,
    #[db(codec = ExactJsonCodec<FederationInboxPayload>)]
    pub record: ExactJson<FederationInboxPayload>,
    #[db(generated)]
    pub receipt_ack: String,
    #[db(generated)]
    pub receipt_kind: Option<String>,
}

#[derive(DbTable)]
#[db(table = "runtime_federation_invites", database = Canvas, state_check = crate::state_check::count_legacy_rows::<FederationInvitesRow>, legacy_ddl = "CREATE TABLE runtime_federation_invites (\n          id TEXT PRIMARY KEY, record TEXT NOT NULL)")]
pub struct FederationInvitesRow {
    #[db(key)]
    pub id: String,
    #[db(codec = ExactJsonCodec<FederationInvitePayload>)]
    pub record: ExactJson<FederationInvitePayload>,
}

#[derive(DbTable)]
#[db(table = "runtime_federation_peers", database = Canvas, state_check = check_federation_peer, legacy_ddl = "CREATE TABLE runtime_federation_peers (\n          id TEXT PRIMARY KEY, record TEXT NOT NULL)")]
pub struct FederationPeersRow {
    #[db(key)]
    pub id: String,
    #[db(codec = ExactJsonCodec<FederationPeerPayload>)]
    pub record: ExactJson<FederationPeerPayload>,
}

#[derive(DbTable)]
#[db(table = "runtime_federation_rooms", database = Canvas, state_check = check_federation_room, legacy_ddl = "CREATE TABLE runtime_federation_rooms (\n          id TEXT PRIMARY KEY, record TEXT NOT NULL)")]
pub struct FederationRoomsRow {
    #[db(key)]
    pub id: String,
    #[db(codec = ExactJsonCodec<FederationRoomPayload>)]
    pub record: ExactJson<FederationRoomPayload>,
}

#[derive(DbTable)]
#[db(table = "runtime_server_exec", database = Canvas, state_check = crate::state_check::count_legacy_rows::<ServerExecRow>, legacy_ddl = "CREATE TABLE runtime_server_exec (\n            id TEXT PRIMARY KEY, owner TEXT NOT NULL, actor TEXT NOT NULL,\n            record TEXT NOT NULL)")]
pub struct ServerExecRow {
    #[db(key)]
    pub id: String,
    pub owner: String,
    pub actor: String,
    #[db(codec = ExactJsonCodec<ServerExecPayload>)]
    pub record: ExactJson<ServerExecPayload>,
}

#[derive(DbTable)]
#[db(table = "runtime_server_exec_audit", database = Canvas, state_check = crate::state_check::count_legacy_rows::<ServerExecAuditRow>, legacy_ddl = "CREATE TABLE runtime_server_exec_audit (\n            id TEXT PRIMARY KEY, actor TEXT NOT NULL, source_server TEXT NOT NULL,\n            server TEXT NOT NULL, cwd TEXT NOT NULL, argv_hash TEXT NOT NULL,\n            start REAL NOT NULL, end REAL, exit_code INTEGER, signal INTEGER)")]
pub struct ServerExecAuditRow {
    #[db(key)]
    pub id: String,
    pub actor: String,
    pub source_server: String,
    pub server: String,
    pub cwd: String,
    pub argv_hash: String,
    pub start: f64,
    pub end: Option<f64>,
    pub exit_code: Option<i64>,
    pub signal: Option<i64>,
}

#[derive(DbTable)]
#[db(table = "runtime_server_exec_input", database = Canvas, state_check = crate::state_check::count_legacy_rows::<ServerExecInputRow>, legacy_ddl = "CREATE TABLE runtime_server_exec_input (\n            id TEXT PRIMARY KEY, handle TEXT NOT NULL)", legacy_index = "CREATE INDEX runtime_server_exec_input_handle\n            ON runtime_server_exec_input(handle)")]
pub struct ServerExecInputRow {
    #[db(key)]
    pub id: String,
    pub handle: String,
}

#[derive(DbTable)]
#[db(table = "runtime_server_inbox", database = Canvas, state_check = crate::multi_server_state_check::check_server_inbox, legacy_ddl = "CREATE TABLE runtime_server_inbox (\n                    id TEXT PRIMARY KEY, signature TEXT NOT NULL, state TEXT NOT NULL,\n                    result TEXT, created_at REAL NOT NULL DEFAULT 0, completed_at REAL NOT NULL DEFAULT 0, principal TEXT, actor TEXT, admission TEXT, receipt_state TEXT GENERATED ALWAYS AS (state) VIRTUAL NOT NULL CHECK(\"receipt_state\" IN ('queued','running','waiting','complete')))")]
pub struct ServerInboxRow {
    #[db(key)]
    pub id: String,
    pub signature: String,
    pub state: String,
    pub result: Option<String>,
    pub created_at: f64,
    pub completed_at: f64,
    pub principal: Option<String>,
    pub actor: Option<String>,
    #[db(codec = ExactJsonCodec<studio_runtime_schema::persistence::AdmittedRequest>)]
    pub admission: Option<ExactJson<studio_runtime_schema::persistence::AdmittedRequest>>,
    #[db(generated)]
    pub receipt_state: String,
}

#[derive(DbTable)]
#[db(table = "runtime_server_links", database = Canvas, state_check = crate::state_check::count_legacy_rows::<ServerLinksRow>, legacy_ddl = "CREATE TABLE runtime_server_links (\n                    id TEXT PRIMARY KEY, record TEXT NOT NULL)", legacy_index = "CREATE INDEX runtime_server_links_home_parent\n                    ON runtime_server_links(json_extract(record,'$.parent'))\n                    WHERE json_extract(record,'$.side')='home'")]
pub struct ServerLinksRow {
    #[db(key)]
    pub id: String,
    #[db(codec = ExactJsonCodec<ServerLinkPayload>)]
    pub record: ExactJson<ServerLinkPayload>,
}

#[derive(DbTable)]
#[db(table = "runtime_server_output_tools", database = Canvas, state_check = crate::state_check::count_legacy_rows::<ServerOutputToolsRow>, legacy_ddl = "CREATE TABLE runtime_server_output_tools (\n                    id TEXT PRIMARY KEY, expires REAL NOT NULL)", legacy_index = "CREATE INDEX runtime_server_output_tools_expiry\n                    ON runtime_server_output_tools(expires)")]
pub struct ServerOutputToolsRow {
    #[db(key)]
    pub id: String,
    pub expires: f64,
}

#[derive(DbTable)]
#[db(table = "runtime_server_retired", database = Canvas, state_check = crate::state_check::count_legacy_rows::<ServerRetiredRow>, legacy_ddl = "CREATE TABLE runtime_server_retired (\n                    id TEXT NOT NULL, direction TEXT NOT NULL, signature TEXT NOT NULL,\n                    server TEXT, actor TEXT, PRIMARY KEY(id,direction))")]
pub struct ServerRetiredRow {
    #[db(key)]
    pub id: String,
    #[db(key)]
    pub direction: String,
    pub signature: String,
    pub server: Option<String>,
    pub actor: Option<String>,
}

macro_rules! federation_checkers {
    ($($check:ident => $table:ident),+ $(,)?) => {$(
        fn $check(connection: &rusqlite::Connection) -> rusqlite::Result<studio_db_api::StateCheck> {
            crate::state_check::storage_report(crate::repositories::federation_messaging::check_table_state(connection, crate::repositories::federation_messaging::FederationTable::$table))
        }
    )+};
}
federation_checkers! { check_federation_peer => Peer, check_federation_room => Room, check_federation_inbox => Inbox }

/// Historical inbox ALTERs and the grant migration keep their original bootstrap.
pub(crate) fn prepare_features(connection: &rusqlite::Connection) -> rusqlite::Result<()> {
    use studio_db_api::Table;
    crate::canvas_schema::install(
        connection,
        &[
            AccessAliasRequestsRow::descriptor(),
            AccessAuditRow::descriptor(),
            AccessAutoAttemptsRow::descriptor(),
            AccessClientsRow::descriptor(),
            AccessDiscoveredRow::descriptor(),
            AccessInvitesRow::descriptor(),
            AccessNameRequestsRow::descriptor(),
            AccessNameTargetsRow::descriptor(),
            AccessNoncesRow::descriptor(),
            AccessUnrevokesRow::descriptor(),
            FederationIdentityRow::descriptor(),
            FederationInvitesRow::descriptor(),
            FederationPeersRow::descriptor(),
            FederationRoomsRow::descriptor(),
            ServerExecRow::descriptor(),
            ServerExecAuditRow::descriptor(),
            ServerExecInputRow::descriptor(),
            ServerLinksRow::descriptor(),
            ServerOutputToolsRow::descriptor(),
            ServerRetiredRow::descriptor(),
        ],
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use studio_db_api::Table;

    fn family_registrations() -> impl Iterator<Item = studio_db_api::Registration> {
        crate::canvas_schema::registry()
            .into_iter()
            .filter(|entry| {
                let name = entry.descriptor.name;
                (name.starts_with("runtime_access_")
                    || name.starts_with("runtime_federation_")
                    || name.starts_with("runtime_server_")
                    || name == "project_grants")
                    && !matches!(
                        name,
                        "runtime_access_local_alias"
                            | "runtime_access_settings"
                            | "runtime_federation_settings"
                    )
            })
    }

    #[test]
    fn access_federation_new_reports_keep_unvalidated_history_accepted() {
        let mut checked = 0;
        for registration in family_registrations() {
            let descriptor = registration.descriptor;
            if matches!(
                descriptor.name,
                "runtime_access_clients"
                    | "runtime_access_discovered"
                    | "runtime_access_invites"
                    | "runtime_server_inbox"
                    | "runtime_federation_peers"
                    | "runtime_federation_rooms"
                    | "runtime_federation_inbox"
            ) {
                continue;
            }
            let db = rusqlite::Connection::open_in_memory().expect("legacy db");
            db.execute_batch(descriptor.ddl).expect("physical table");
            // Raw SQL: forge historical storage types without entering typed write admission.
            let mut columns = db
                .prepare(
                    "SELECT name,type,pk FROM pragma_table_xinfo(?1) WHERE hidden=0 ORDER BY cid",
                )
                .expect("columns");
            let fields = columns
                .query_map([descriptor.name], |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, i64>(2)?,
                    ))
                })
                .expect("fields")
                .collect::<rusqlite::Result<Vec<_>>>()
                .expect("column types");
            let names = fields
                .iter()
                .map(|(name, _, _)| format!("\"{name}\""))
                .collect::<Vec<_>>()
                .join(",");
            let values = fields
                .iter()
                .map(|(_, kind, key)| {
                    if kind == "INTEGER" && *key > 0 {
                        "1"
                    } else {
                        "x'00'"
                    }
                })
                .collect::<Vec<_>>()
                .join(",");
            db.execute(
                &format!(
                    "INSERT INTO \"{}\"({names}) VALUES({values})",
                    descriptor.name
                ),
                [],
            )
            .expect("historically admitted row");
            let report = (registration.check)(&db).expect("count-only report");
            assert_eq!(report.counts(), (1, 0), "{}", descriptor.name);
            assert!(report.failures.is_empty(), "{}", descriptor.name);
            checked += 1;
        }
        assert_eq!(checked, 16);
    }

    #[derive(serde::Deserialize)]
    struct Catalog {
        #[serde(rename = "canvas.sqlite3")]
        canvas: Objects,
    }
    #[derive(serde::Deserialize)]
    struct Objects {
        sqlite_master: Vec<Entry>,
    }
    #[derive(serde::Deserialize)]
    struct Entry {
        name: String,
        sql: String,
    }

    #[test]
    fn access_federation_descriptors_preserve_every_table_and_index() {
        let catalog: Catalog =
            studio_json::from_str(crate::fresh_state::SNAPSHOT).expect("catalog");
        let db = crate::fresh_state::test_canvas();
        for registration in family_registrations() {
            let descriptor = registration.descriptor;

            if descriptor.name == "project_grants" {
                let legacy = rusqlite::Connection::open_in_memory().expect("legacy grants");
                legacy.execute_batch("CREATE TABLE project_grants(\n            grant_id TEXT NOT NULL,\n            peer TEXT NOT NULL,\n            peer_public_key TEXT NOT NULL,\n            local_project TEXT NOT NULL,\n            remote_project TEXT NOT NULL,\n            capability TEXT NOT NULL,\n            created_at INTEGER NOT NULL,\n            revoked_at INTEGER,\n            PRIMARY KEY(grant_id, capability)\n        ); ALTER TABLE project_grants ADD COLUMN peer_label TEXT NOT NULL DEFAULT '';").expect("pre-descriptor DDL and migration");
                let sql: String = legacy
                    .query_row(
                        "SELECT sql FROM sqlite_master WHERE name='project_grants'",
                        [],
                        |row| row.get(0),
                    )
                    .expect("grant DDL");
                assert_eq!(descriptor.ddl, sql, "grant DDL");
            } else {
                let entry = catalog
                    .canvas
                    .sqlite_master
                    .iter()
                    .find(|entry| entry.name == descriptor.name)
                    .expect("catalog table");
                assert_eq!(descriptor.ddl, entry.sql, "{} DDL", descriptor.name);
            }
            for index in descriptor.indexes {
                let sql: String = db
                    .query_row(
                        "SELECT sql FROM sqlite_master WHERE name=?1",
                        [index.name],
                        |row| row.get(0),
                    )
                    .expect("index");
                assert_eq!(index.sql, sql, "{} index", index.name);
            }
            let report = (registration.check)(&db).expect("unchanged state check");
            assert_eq!(report.rejected_count, 0, "{}", descriptor.name);
        }
    }

    #[test]
    fn access_federation_nonce_and_request_uniqueness_roll_back() {
        let mut db = rusqlite::Connection::open_in_memory().expect("db");
        db.execute_batch(AccessNoncesRow::DDL).expect("nonces");
        db.execute_batch(AccessNameTargetsRow::DDL)
            .expect("targets");
        db.execute_batch(AccessNameRequestsRow::DDL).expect("names");
        let tx = db.transaction().expect("tx");
        let mut typed = crate::peer_update_settings::canvas_tx(&tx);
        let nonce = AccessNoncesRow {
            client: "federation:peer".into(),
            nonce: "n".into(),
            created: 1,
        };
        assert!(typed.insert_new(&nonce).expect("new"));
        assert!(!typed.insert_new(&nonce).expect("replay"));
        assert!(crate::repositories::federation_credentials::FederationCredentialsRepository::reserve_name(&tx, "request", "label", "actor").expect("reserve"));
        assert!(!crate::repositories::federation_credentials::FederationCredentialsRepository::reserve_name(&tx, "request", "other", "actor").expect("conflict"));
        tx.rollback().expect("rollback");
        assert_eq!(
            db.query_row("SELECT count(*) FROM runtime_access_nonces", [], |row| row
                .get::<_, i64>(
                0
            ))
            .expect("count"),
            0
        );
    }

    #[test]
    fn access_federation_room_put_keeps_integral_decimal_sequences() {
        use crate::repositories::federation_messaging::{
            FederationMessagingRepository, FederationTable,
        };
        let mut db = rusqlite::Connection::open_in_memory().expect("db");
        db.execute_batch(FederationRoomsRow::DDL).expect("rooms");
        for number in ["1.0", "1e0"] {
            let raw = r#" {"id":"room","kind":"federated","peerId":"peer","peerLabel":"Peer","name":"Room","status":"approved","localMembers":[],"remoteMembers":[],"localApproved":true,"remoteApproved":true,"shareNames":true,"shareStatus":true,"created":1,"updated":1,"sequence":$number,"remoteSequence":$number} "#.replace("$number", number);
            let value = FederationMessagingRepository::parse(FederationTable::Room, "room", &raw)
                .expect("historical schema accepts integral decimals");
            let expected = studio_json::to_string(&value).expect("legacy serialization");
            let tx = db.transaction().expect("tx");
            FederationMessagingRepository::put(&tx, FederationTable::Room, "room", "peer", &value)
                .expect("room-accept caller can persist the accepted room");
            let stored = || {
                tx.query_row("SELECT record FROM runtime_federation_rooms", [], |row| {
                    row.get::<_, String>(0)
                })
                .expect("stored room")
            };
            assert_eq!(
                stored(),
                expected,
                "caller retains historical serialization"
            );
            tx.execute("UPDATE runtime_federation_rooms SET record=?1", [&raw])
                .expect("historical spelling");
            let mut typed = canvas_tx(&tx);
            let row = typed
                .get::<FederationRoomsRow>(&("room".into(),))
                .expect("typed read")
                .expect("room");
            typed.upsert(&row).expect("exact rewrite");
            assert_eq!(
                stored(),
                raw,
                "typed rewrite keeps numeric spelling and whitespace"
            );
            tx.rollback().expect("rollback");
        }
    }

    #[test]
    fn access_federation_inbox_keeps_receipt_identity_and_exact_evidence() {
        let mut db = rusqlite::Connection::open_in_memory().expect("db");
        db.execute_batch(FederationInboxRow::DDL).expect("inbox");
        let raw = " \n{\"hash\":\"h\",\"ack\":\"pending\",\"created\":1.25,\"requestId\":\"00000000-0000-7000-8000-000000000001\",\"kind\":\"chat-message\",\"evidence\":{\"x\":\"\\u0061\"}}\t";
        db.execute("INSERT INTO runtime_federation_inbox(peer_id,message_id,record) VALUES('peer','message',?1)", [raw]).expect("legacy receipt");
        let tx = db.transaction().expect("tx");
        let mut typed = canvas_tx(&tx);
        let row = typed
            .get::<FederationInboxRow>(&("peer".into(), "message".into()))
            .expect("read")
            .expect("receipt");
        assert_eq!(row.record.raw(), raw);
        assert_eq!(row.receipt_ack, "pending");
        assert_eq!(row.receipt_kind.as_deref(), Some("chat-message"));
        assert!(
            typed.insert(&row).is_err(),
            "peer/message identity is unique"
        );
        typed.upsert(&row).expect("ack projection is never written");
        assert_eq!(
            tx.query_row("SELECT record FROM runtime_federation_inbox", [], |row| row
                .get::<_, String>(0))
                .expect("stored"),
            raw
        );
        tx.rollback().expect("rollback");
        let report = check_federation_inbox(&db).expect("legacy checker");
        assert_eq!(report.counts(), (1, 0));
    }
}
