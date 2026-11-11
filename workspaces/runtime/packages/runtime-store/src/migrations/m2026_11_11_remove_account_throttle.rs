//! Retire per-account turn-start settings and their ephemeral agent waits.
//! Source is the supported registry format with optional startThrottle fields.
//! Run under runtime.lock before current registry readers and worker admission.
use crate::{StoreError, StoreResult};
use rusqlite::Transaction;
use serde::Deserialize;
use std::path::Path;
use studio_runtime_schema::studio_errors::{
    MigrationPhase, MigrationRollbackOutcome, MigrationTarget,
};

pub(super) const REGISTRY_DEFINITION: super::MigrationDefinition = super::MigrationDefinition {
    id: "2026-11-11-remove-account-throttle",
    introduced: super::MigrationDate {
        year: 2026,
        month: 11,
        day: 11,
    },
    target: MigrationTarget::AccountRegistry,
    supported_source: "Current account registry with optional startThrottle settings",
    remove_when: "Supported installations and backups no longer contain account throttle settings",
};

/// Frozen source-format bounds, in starts, seconds, and running turns.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct LegacyThrottle {
    burst: f64,
    pause_seconds: f64,
    max_running_turns: f64,
}
impl LegacyThrottle {
    fn valid(&self) -> bool {
        [
            (self.burst, 1.0, 512.0),
            (self.pause_seconds, 0.0, 3600.0),
            (self.max_running_turns, 1.0, 512.0),
        ]
        .into_iter()
        .all(|(value, minimum, maximum)| {
            value.fract() == 0.0 && value >= minimum && value <= maximum
        })
    }
}

fn invalid_registry() -> StoreError {
    StoreError::SchemaMismatch("invalid account registry during throttle removal".into())
}

/// Validate the destination before replacing any old registry bytes.
/// Ordered account and login keys and all receipt fields keep their identities.
pub(super) fn registry_destination(bytes: &[u8]) -> StoreResult<Option<Vec<u8>>> {
    if bytes.len() > crate::account_state::MAX_PERSISTED_DOCUMENT_BYTES
        || !crate::account_state::persisted_document_within_bounds(bytes)
    {
        return Err(invalid_registry());
    }
    let mut value: studio_json::Value =
        studio_json::from_slice(bytes).map_err(|_| invalid_registry())?;
    let accounts = value
        .get_mut("accounts")
        .and_then(studio_json::Value::as_object_mut)
        .ok_or_else(invalid_registry)?;
    let mut changed = false;
    for account in accounts.values_mut() {
        let row = account.as_object_mut().ok_or_else(invalid_registry)?;
        if let Some(throttle) = row.get("startThrottle") {
            let throttle: Option<LegacyThrottle> =
                Option::deserialize(throttle).map_err(|_| invalid_registry())?;
            if throttle.is_some_and(|throttle| !throttle.valid()) {
                return Err(invalid_registry());
            }
            row.remove("startThrottle");
            changed = true;
        }
    }
    if !changed {
        return Ok(None);
    }
    let mut deserializer = studio_json::Deserializer::from_slice(bytes);
    let order = crate::account_registry::RegistryOrder::deserialize(&mut deserializer)
        .map_err(|_| invalid_registry())?;
    let bytes =
        crate::account_registry::encode_registry(&value, &order).map_err(|_| invalid_registry())?;
    crate::account_registry::decode_registry_typed(&bytes).map_err(|_| invalid_registry())?;
    Ok(Some(bytes))
}

pub(super) fn registry(root: &Path) -> StoreResult<()> {
    let path = root.join("accounts/registry.json");
    let bytes = match std::fs::read(&path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error.into()),
    };
    let Some(destination) = registry_destination(&bytes)? else {
        return Ok(());
    };
    crate::account_registry::atomic_replace(&path, &destination).map_err(|error| {
        StoreError::Migration {
            id: REGISTRY_DEFINITION.id,
            target: MigrationTarget::AccountRegistry,
            phase: MigrationPhase::Commit,
            rollback: match error {
                crate::account_registry::ReplaceError::NotReplaced => {
                    MigrationRollbackOutcome::NotRequired
                }
                crate::account_registry::ReplaceError::ReplacedNotDurable => {
                    MigrationRollbackOutcome::Unknown
                }
            },
            source: Box::new(StoreError::SchemaMismatch(
                "account registry atomic replacement failed".into(),
            )),
            rollback_error: None,
        }
    })
}

pub(super) fn apply(tx: &Transaction<'_>) -> StoreResult<()> {
    tx.execute("UPDATE runtime_agents SET record=json_remove(record,'$.startWait') WHERE json_extract(record,'$.startWait.reason') IN ('slot','pause','start_queue')", [])?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use studio_json::json;

    #[tokio::test]
    async fn startup_removes_account_throttle_and_preserves_other_waits() {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path();
        let store = crate::RuntimeStore::open(Some(root)).await.unwrap();
        store.transaction_with(|tx| {
            for reason in ["slot", "pause", "start_queue", "model_catalog", "provider_session", "dependency", "disk"] {
                let record = json!({"id":reason,"rootId":reason,"provider":"codex","accountKey":"default","epoch":3,"compactions":0,"status":"queued","autoWake":true,"startWait":{"reason":reason,"epoch":3}});
                tx.execute("INSERT INTO runtime_agents(id,record) VALUES(?1,?2)", rusqlite::params![reason,record.to_string()])?;
            }
            Ok::<_,StoreError>(())
        }).await.unwrap();
        drop(store);
        // Force the preceding catalog so this resembles a pre-upgrade database.
        let db = rusqlite::Connection::open(root.join("canvas.sqlite3")).unwrap();
        let previous = super::super::catalog_fingerprint(
            &super::super::CATALOG[..super::super::CATALOG.len() - 1],
            studio_runtime_schema::studio_errors::MigrationTarget::Canvas,
        );
        let marker = super::super::catalog_marker(&db, &previous).unwrap();
        db.execute(
            "UPDATE sync_entity_meta SET value=?1 WHERE key='migration_catalog'",
            [&marker],
        )
        .unwrap();
        drop(db);
        std::fs::create_dir_all(root.join("accounts")).unwrap();
        let registry_path = root.join("accounts/registry.json");
        let registry = json!({"version":2,"defaultAccountKey":"default","accounts":{"default":{"id":"default","home":"/fixture","label":"Fixture","source":"CLI","status":"ready","provider":"codex","startThrottle":{"burst":1,"pauseSeconds":3600,"maxRunningTurns":1}},"nullable":{"id":"nullable","home":"/nullable","label":"Nullable","source":"CLI","status":"ready","provider":"claude","startThrottle":null},"absent":{"id":"absent","home":"/absent","label":"Absent","source":"CLI","status":"ready","provider":"codex"}},"logins":{},"nameReceipts":{"request":{"accountKey":"default","label":"Fixture"}}});
        std::fs::write(&registry_path, registry.to_string()).unwrap();
        let expected_registry = {
            let mut expected = registry.clone();
            for account in expected["accounts"].as_object_mut().unwrap().values_mut() {
                account.as_object_mut().unwrap().remove("startThrottle");
            }
            expected
        };
        let store = crate::RuntimeStore::open(Some(root)).await.unwrap();
        let bytes = std::fs::read(&registry_path).unwrap();
        assert_eq!(
            studio_json::from_slice::<studio_json::Value>(&bytes).unwrap(),
            expected_registry
        );
        crate::account_registry::decode_registry_typed(&bytes).unwrap();
        store
            .read_async("test/throttle-migration", |db| {
                for reason in [
                    "slot",
                    "pause",
                    "start_queue",
                    "model_catalog",
                    "provider_session",
                    "dependency",
                    "disk",
                ] {
                    let agent =
                        crate::AgentRepository::get(db, &crate::AgentId::from(reason))?.unwrap();
                    assert_eq!(
                        agent.start_wait.is_none(),
                        matches!(reason, "slot" | "pause" | "start_queue")
                    );
                    assert_eq!(agent.epoch, 3);
                    assert_eq!(agent.auto_wake, Some(true));
                }
                Ok::<_, StoreError>(())
            })
            .await
            .unwrap();
        drop(store);
        let reopened = crate::RuntimeStore::open(Some(root)).await.unwrap();
        assert_eq!(std::fs::read(&registry_path).unwrap(), bytes);
        drop(reopened);
    }

    #[test]
    fn registry_publication_failures_preserve_order_and_durability_evidence() {
        use crate::account_registry::{ReplaceError, atomic_replace_with};
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("accounts/registry.json");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        let source=br#"{"version":2,"defaultAccountKey":"z","accounts":{"z":{"id":"z","home":"/fixture","label":"Z","source":"CLI","status":"ready","provider":"codex","startThrottle":{"burst":1.0,"pauseSeconds":0.0,"maxRunningTurns":1.0}},"a":{"id":"a","home":"/other","label":"A","source":"CLI","status":"ready","provider":"claude"}},"logins":{"r2":{"requestId":"r2","accountKey":"z","status":"pending"},"r1":{"requestId":"r1","accountKey":"a","status":"ready"}}}"#;
        std::fs::write(&path, source).unwrap();
        let destination = registry_destination(source).unwrap().unwrap();
        let (_, order) = crate::account_registry::decode_registry(&destination).unwrap();
        assert_eq!(order.accounts, ["z", "a"]);
        assert_eq!(order.logins, ["r2", "r1"]);
        let failed = || std::io::Error::other("injected publication failure");
        assert!(matches!(
            atomic_replace_with(&path, &destination, |_, _| Err(failed()), |_| Ok(())),
            Err(ReplaceError::NotReplaced)
        ));
        assert_eq!(std::fs::read(&path).unwrap(), source);
        assert!(matches!(
            atomic_replace_with(
                &path,
                &destination,
                |from, to| std::fs::rename(from, to),
                |_| Err(failed())
            ),
            Err(ReplaceError::ReplacedNotDurable)
        ));
        assert_eq!(std::fs::read(&path).unwrap(), destination);
        registry(directory.path()).unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), destination);
    }

    #[test]
    fn failed_wait_update_rolls_back_all_rows() {
        let mut db = rusqlite::Connection::open_in_memory().unwrap();
        db.execute_batch("CREATE TABLE runtime_agents(id TEXT PRIMARY KEY,record TEXT NOT NULL); INSERT INTO runtime_agents VALUES('first','{\"startWait\":{\"reason\":\"slot\"}}'),('second','{\"startWait\":{\"reason\":\"pause\"}}'); CREATE TRIGGER reject_second BEFORE UPDATE ON runtime_agents WHEN NEW.id='second' BEGIN SELECT RAISE(ABORT,'fixture'); END;").unwrap();
        let tx = db.transaction().unwrap();
        assert!(apply(&tx).is_err());
        tx.rollback().unwrap();
        let count:i64=db.query_row("SELECT count(*) FROM runtime_agents WHERE json_type(record,'$.startWait')='object'",[],|row|row.get(0)).unwrap();
        assert_eq!(count, 2);
    }

    #[tokio::test]
    async fn invalid_registry_is_preserved_on_startup_failure() {
        let directory = tempfile::tempdir().unwrap();
        drop(
            crate::RuntimeStore::open(Some(directory.path()))
                .await
                .unwrap(),
        );
        let path = directory.path().join("accounts/registry.json");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        for (version, throttle) in [
            (2, json!(false)),
            (2, json!({"burst":1})),
            (
                2,
                json!({"burst":1,"pauseSeconds":0,"maxRunningTurns":1,"extra":true}),
            ),
            (2, json!({"burst":1.5,"pauseSeconds":0,"maxRunningTurns":1})),
            (2, json!({"burst":513,"pauseSeconds":0,"maxRunningTurns":1})),
            (3, json!({"burst":1,"pauseSeconds":0,"maxRunningTurns":1})),
        ] {
            let registry = json!({"version":version,"defaultAccountKey":"default","accounts":{"default":{"id":"default","home":"/fixture","label":"Fixture","source":"CLI","status":"ready","provider":"codex","startThrottle":throttle}}});
            let bytes = registry.to_string().into_bytes();
            std::fs::write(&path, &bytes).unwrap();
            assert!(
                crate::RuntimeStore::open(Some(directory.path()))
                    .await
                    .is_err()
            );
            assert_eq!(std::fs::read(&path).unwrap(), bytes);
        }
    }
}
