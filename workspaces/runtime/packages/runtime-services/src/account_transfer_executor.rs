//! Retained-session executor for lazy account moves.
//!
//! Ports Python `codex_account_transfer.py:destination_settings`, `move_lazy`,
//! `read_transfer_source`, and `codex_portable_history.py:_native_records`.
//! Provider writes are guarded by the transfer driver's durable intent and exact
//! SessionReceipt; an Unknown outcome is never resent.

use crate::account_session_runtime::SessionOutcome as ReceiptOutcome;
use crate::{
    account_session_runtime::{
        AccountSessionError, AccountSessionPort, ProviderRequest, ProviderResponse,
        RequestCertainty, SessionReceipt, TransferProviderRequest,
    },
    account_transfer::TransferError,
    account_transfer_driver::{
        self as driver, PreparedTransfer, TransferDispatch, TransferExecutor, TransferMethod,
        TransferPreparation, TransferTransport,
    },
    model_catalog::ModelCatalogService,
    portable_history::{self, ExportInput, NativeHistory},
    submit_gates::{
        self as gates, CatalogIdentity, CatalogObservation, ExecutionSettings, ModelCapabilities,
        StoredField,
    },
};
use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
    sync::{
        Arc, LazyLock, Mutex, Weak,
        atomic::{AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};
use studio_json::{Value, json};
use studio_runtime_providers::Method;
use studio_runtime_providers::ProviderKind;
use studio_runtime_store::RuntimeStore;
use studio_runtime_store::path_identity::CanonicalPath;

const READ_DEADLINE: Duration = Duration::from_secs(120);
const MUTATION_DEADLINE: Duration = Duration::from_secs(60);

static NATIVE_IMPORT_SEQUENCE: AtomicU64 = AtomicU64::new(1);
static NATIVE_IMPORT_LOCKS: LazyLock<Mutex<HashMap<PathBuf, Weak<Mutex<()>>>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

fn native_import_lock(target_home: &Path) -> Result<Arc<Mutex<()>>, TransferError> {
    let mut locks = NATIVE_IMPORT_LOCKS
        .lock()
        .map_err(|_| TransferError::Refused("Native import lock is unavailable"))?;
    locks.retain(|_, lock| lock.strong_count() > 0);
    let key = target_home.to_path_buf();
    if let Some(lock) = locks.get(&key).and_then(Weak::upgrade) {
        return Ok(lock);
    }
    let lock = Arc::new(Mutex::new(()));
    locks.insert(key, Arc::downgrade(&lock));
    Ok(lock)
}

fn copy_native_history(
    source_home: &str,
    target_home: &str,
    native_path: &str,
) -> Result<String, TransferError> {
    let source_root = CanonicalPath::existing(source_home)
        .map_err(|_| TransferError::Refused("Source native history is unavailable"))?;
    let source = CanonicalPath::existing(native_path)
        .map_err(|_| TransferError::Refused("Source native history is unavailable"))?;
    let relative = source.strip_prefix(&source_root).map_err(|_| {
        TransferError::Refused("Source native history path escaped its account home")
    })?;
    if !matches!(
        relative
            .components()
            .next()
            .and_then(|part| part.as_os_str().to_str()),
        Some("sessions" | "archived_sessions")
    ) || source.extension().and_then(|value| value.to_str()) != Some("jsonl")
    {
        return Err(TransferError::Refused(
            "Unsupported native history format; original context is preserved",
        ));
    }
    let bytes = fs::read(&source)
        .map_err(|_| TransferError::Refused("Source native history is not fully persisted"))?;
    let first_end = bytes
        .iter()
        .position(|byte| *byte == b'\n')
        .ok_or(TransferError::Refused(
            "Native history has no complete session metadata",
        ))?;
    let metadata: Value = studio_json::from_slice(&bytes[..first_end])
        .map_err(|_| TransferError::Refused("Native history metadata is invalid"))?;
    let header = crate::analytics_history::SessionHeader::from_value(&metadata);
    if !header
        .as_ref()
        .is_some_and(|header| header.is_session() && header.history_base().is_none())
    {
        return Err(TransferError::Refused(
            "Native history ancestry requires the portable-history importer",
        ));
    }
    if !bytes.ends_with(b"\n") {
        return Err(TransferError::Refused(
            "Source native history is not fully persisted",
        ));
    }
    let target_root = CanonicalPath::existing(target_home)
        .map_err(|_| TransferError::Refused("Destination account home is unavailable"))?;
    let lock = native_import_lock(&target_root)?;
    let _guard = lock
        .lock()
        .map_err(|_| TransferError::Refused("Native import lock is unavailable"))?;
    let destination = target_root.join("sessions").join(".studio-imports").join(
        source
            .file_name()
            .ok_or(TransferError::Refused("Invalid native history filename"))?,
    );
    fs::create_dir_all(
        destination
            .parent()
            .ok_or(TransferError::Refused("Invalid native history destination"))?,
    )
    .map_err(|_| TransferError::Refused("Could not create native history destination"))?;
    if destination.exists() {
        if fs::read(&destination)
            .map_err(|_| TransferError::Refused("Native history destination is unreadable"))?
            != bytes
        {
            return Err(TransferError::Refused(
                "The destination has different native history. No file was overwritten.",
            ));
        }
    } else {
        let temporary = destination.with_extension(format!(
            "studio-transfer-{}-{}.tmp",
            std::process::id(),
            NATIVE_IMPORT_SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        fs::write(&temporary, &bytes)
            .map_err(|_| TransferError::Refused("Could not copy native history"))?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&temporary, fs::Permissions::from_mode(0o600))
                .map_err(|_| TransferError::Refused("Could not secure native history copy"))?;
        }
        match fs::hard_link(&temporary, &destination) {
            Ok(()) => fs::remove_file(&temporary)
                .map_err(|_| TransferError::Refused("Could not publish native history copy"))?,
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                fs::remove_file(&temporary)
                    .map_err(|_| TransferError::Refused("Could not publish native history copy"))?;
                if fs::read(&destination).map_err(|_| {
                    TransferError::Refused("Native history destination is unreadable")
                })? != bytes
                {
                    return Err(TransferError::Refused(
                        "The destination has different native history. No file was overwritten.",
                    ));
                }
            }
            Err(_) => {
                let _ignored = fs::remove_file(&temporary);
                return Err(TransferError::Refused(
                    "Could not publish native history copy",
                ));
            }
        }
    }
    let manifest_path = target_root.join(".studio-account-imports.json");
    let mut manifest: studio_json::Map<String, Value> = if manifest_path.exists() {
        studio_json::from_slice(
            &fs::read(&manifest_path)
                .map_err(|_| TransferError::Refused("Native import manifest is unreadable"))?,
        )
        .map_err(|_| TransferError::Refused("Native import manifest is invalid"))?
    } else {
        studio_json::Map::new()
    };
    let relative_destination = destination.strip_prefix(&target_root).map_err(|_| {
        TransferError::Refused("Native history destination escaped its account home")
    })?;
    let relative_key = relative_destination.to_string_lossy().replace('\\', "/");
    manifest.insert(
        relative_key,
        json!({
            "threadId":header.as_ref().and_then(crate::analytics_history::SessionHeader::raw_id).cloned().unwrap_or(Value::Null),
            "sha256":studio_runtime_store::sha256_hex(&bytes),
            "sourceHome":source_root,
        }),
    );
    let temporary_manifest = manifest_path.with_extension(format!(
        "studio-transfer-{}-{}.tmp",
        std::process::id(),
        NATIVE_IMPORT_SEQUENCE.fetch_add(1, Ordering::Relaxed)
    ));
    fs::write(
        &temporary_manifest,
        studio_json::to_vec(&Value::Object(manifest))
            .map_err(|_| TransferError::Refused("Could not encode native import manifest"))?,
    )
    .map_err(|_| TransferError::Refused("Could not publish native import manifest"))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&temporary_manifest, fs::Permissions::from_mode(0o600))
            .map_err(|_| TransferError::Refused("Could not secure native import manifest"))?;
    }
    fs::rename(&temporary_manifest, &manifest_path)
        .map_err(|_| TransferError::Refused("Could not publish native import manifest"))?;
    Ok(destination.to_string_lossy().into_owned())
}

pub struct AccountTransferExecutor {
    pub store: Arc<RuntimeStore>,
    pub session: Arc<dyn AccountSessionPort>,
    pub catalog: ModelCatalogService,
    pub preparation: crate::account_worker::preparation::PreparationResources,
    pub now: f64,
}

/// Params for `claude/moveImport`: the source session export and its native history file.
/// The target bridge repeats the saved prompt and tool proof, so a changed setting refuses
/// before any native write. The model is checked here because the bridge keeps the source model.
fn claude_import_params(
    export: &Value,
    cwd: &Value,
    model: &Value,
) -> Result<Value, TransferError> {
    let path = export
        .get("path")
        .and_then(Value::as_str)
        .filter(|path| !path.is_empty())
        .ok_or(TransferError::Refused(
            "Claude move export returned no history path",
        ))?;
    let session = export
        .get("session")
        .filter(|session| session.is_object())
        .ok_or(TransferError::Refused(
            "Claude move export returned no session",
        ))?;
    if session.get("model") != Some(model) {
        return Err(TransferError::Refused(
            "The destination Claude model differs from the source session",
        ));
    }
    Ok(json!({"session": session, "path": path, "cwd": cwd}))
}

async fn run_transfer_blocking<T: Send + 'static>(
    operation: impl FnOnce() -> Result<T, TransferError> + Send + 'static,
) -> Result<T, TransferError> {
    studio_runtime_process::blocking_pool::spawn_blocking(
        "runtime-services/account_transfer_executor",
        operation,
    )
    .await
    .map_err(|_| TransferError::Refused("Account transfer preparation worker stopped"))?
}

struct ProviderReadContext<'a> {
    store: &'a RuntimeStore,
    session: &'a dyn AccountSessionPort,
    transfer: &'a str,
    agent: &'a str,
    account: &'a str,
    now: f64,
}

struct ProviderReadCall {
    actor: Option<crate::account_worker::retained::RequestActor>,
    transfer_source_thread: Option<String>,
    operation: String,
    method: Method,
    params: Value,
    deadline: Duration,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct SourcePage {
    data: Vec<serde::de::IgnoredAny>,
    next_cursor: Option<serde::de::IgnoredAny>,
}

#[derive(serde::Deserialize)]
#[serde(untagged)]
enum HistoryCursor {
    Text(String),
    Other(serde::de::IgnoredAny),
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct HistoryPage {
    #[serde(rename = "data")]
    _data: Vec<serde::de::IgnoredAny>,
    next_cursor: Option<HistoryCursor>,
}

fn source_page_empty(value: &Value, invalid: &'static str) -> Result<bool, TransferError> {
    let page: SourcePage =
        serde::Deserialize::deserialize(value).map_err(|_| TransferError::Refused(invalid))?;
    Ok(page.data.is_empty() && page.next_cursor.is_none())
}

#[cfg(test)]
mod reconnect_read_tests {
    use super::*;
    use crate::recovery_history::ConnectionScope;
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct ReconnectedSession(AtomicUsize);
    impl AccountSessionPort for ReconnectedSession {
        fn current_scope(&self, account: &str) -> Option<ConnectionScope> {
            Some(ConnectionScope {
                account: account.into(),
                connection: "new-connection".into(),
                generation: 8,
            })
        }
        fn provider_request<'a>(
            &'a self,
            _request: ProviderRequest,
        ) -> crate::account_session_runtime::AccountSessionFuture<'a, SessionReceipt> {
            self.0.fetch_add(1, Ordering::SeqCst);
            Box::pin(async { Err(AccountSessionError::Refused("no native write is permitted")) })
        }
        fn transfer_provider_request<'a>(
            &'a self,
            _request: TransferProviderRequest,
        ) -> crate::account_session_runtime::AccountSessionFuture<'a, SessionReceipt> {
            self.0.fetch_add(1, Ordering::SeqCst);
            Box::pin(async { Err(AccountSessionError::Refused("no native write is permitted")) })
        }
    }

    async fn fixture(change: &'static str) -> (tempfile::TempDir, RuntimeStore, Value) {
        let root = tempfile::tempdir().expect("reconnect state");
        let store = RuntimeStore::open(Some(root.path())).await.expect("store");
        let old_scope = json!({"account":"source","connection":"old-connection","generation":7});
        let mut saved = json!({"receipt":{"operation":"read","scope":old_scope,
            "certainty":"written","ticket":{"generation":7,"id":"old-request","method":"thread/read"}},
            "certainty":"written","result":{"kind":"reply","value":{"thread":{"id":"source-thread"}}}});
        match change {
            "receipt-scope" => saved["receipt"]["scope"]["connection"] = json!("wrong-connection"),
            "ticket-generation" => saved["receipt"]["ticket"]["generation"] = json!(8),
            "ticket-method" => saved["receipt"]["ticket"]["method"] = json!("thread/start"),
            "ticket-id" => saved["receipt"]["ticket"]["id"] = Value::Null,
            "unknown" | "not-sent" => {
                let certainty = if change == "unknown" {
                    "unknown"
                } else {
                    "not_sent"
                };
                saved["certainty"] = json!(certainty);
                saved["receipt"]["certainty"] = json!(certainty);
            }
            "rejected" => saved["result"]["kind"] = json!("rejected"),
            _ => {}
        }
        let expected = saved.clone();
        store.transaction(move |tx| {
            let agent = studio_runtime_store::AgentRecord::from_value(json!({"compactions":0,"provider":studio_runtime_schema::ProviderKind::Codex,"id":"member","rootId":"member","epoch":3,
                "status":"completed","isLead":true,"autoWake":false,"accountKey":"target","accountTransferId":"move"}))?;
            studio_runtime_store::AgentRepository::put(tx, &agent)?;
            let operation = json!({"id":"move","leadId":"member","targetAccountKey":"target",
                "status":"pending","scope":"team","created":1.0,"members":{"member":{
                    "phase":"reading","sourceAccountKey":"source","sourceThreadId":"source-thread",
                    "providerReadIntents":{"read":{"operation":"read","account":"source","scope":old_scope,
                        "method":"thread/read","params":{"threadId":"source-thread","includeTurns":false}}},
                    "providerReadReceipts":{"read":saved}}},"requests":{}});
            if change == "ticket-id" {
                // Simulate a corrupt retained receipt that cannot pass typed writes.
                tx.execute("INSERT INTO runtime_account_transfers(id,record) VALUES('move',?1)", [operation.to_string()])?;
                Ok(())
            } else {
                studio_runtime_store::AccountTransferRepository::put(tx, "move", &operation.to_string())
            }
        }).await.expect("written read");
        (root, store, expected)
    }

    async fn read(
        store: &RuntimeStore,
        session: &ReconnectedSession,
        changed_content: bool,
    ) -> Result<(SessionReceipt, Value), TransferError> {
        session_reply(&ProviderReadContext { store, session, transfer:"move", agent:"member", account:"source", now:2.0 },
            ProviderReadCall { actor:None, transfer_source_thread:None, operation:"read".into(), method:Method::ThreadRead,
                params:json!({"threadId":if changed_content { "another-thread" } else { "source-thread" },"includeTurns":false}), deadline:Duration::from_secs(1) }).await
    }

    #[tokio::test]
    async fn written_read_replays_its_original_receipt_after_reconnect_without_native_writes() {
        let (_root, store, expected) = fixture("").await;
        let session = ReconnectedSession(AtomicUsize::new(0));
        let before = store
            .read_async("before replay", |tx| {
                driver::load(tx, "move").map(|s| s.record().clone())
            })
            .await
            .expect("operation");
        let (receipt, value) = read(&store, &session, false)
            .await
            .expect("replay the durable reply");
        assert_eq!(
            studio_json::to_value(&receipt.scope).expect("scope"),
            expected["receipt"]["scope"]
        );
        assert_eq!(
            receipt.ticket().expect("original ticket").id,
            studio_runtime_providers::RequestId::String("old-request".into())
        );
        assert_eq!(value, expected["result"]["value"]);
        assert!(receipt.replayed);
        assert_eq!(session.0.load(Ordering::SeqCst), 0);
        let after = store
            .read_async("after replay", |tx| {
                driver::load(tx, "move").map(|s| s.record().clone())
            })
            .await
            .expect("operation");
        assert_eq!(
            studio_json::to_vec(&after).unwrap(),
            studio_json::to_vec(&before).unwrap()
        );
    }

    #[tokio::test]
    async fn reconnect_read_refuses_changed_content_or_unproven_receipts_without_native_writes() {
        for change in [
            "content",
            "receipt-scope",
            "ticket-generation",
            "ticket-method",
            "ticket-id",
            "unknown",
            "not-sent",
            "rejected",
        ] {
            let (_root, store, _) = fixture(change).await;
            let session = ReconnectedSession(AtomicUsize::new(0));
            assert!(
                read(&store, &session, change == "content").await.is_err(),
                "{change}"
            );
            assert_eq!(session.0.load(Ordering::SeqCst), 0, "{change}");
        }
    }

    #[tokio::test]
    async fn written_source_unsubscribe_replays_only_its_exact_receipt_after_reconnect() {
        let (_root, store, _) = fixture("").await;
        const OPERATION: &str = "transfer:move:member:unsubscribe";
        store
            .transaction(|tx| {
                let mut operation =
                    studio_runtime_store::AccountTransferRepository::get(tx, "move")?
                        .expect("transfer");
                let member = &mut operation["members"]["member"];
                let mut intent = member["providerReadIntents"]["read"].take();
                intent["operation"] = json!(OPERATION);
                intent["method"] = json!("thread/unsubscribe");
                intent["params"] = json!({"threadId":"source-thread"});
                member["providerReadIntents"] = json!({(OPERATION):intent});
                let mut result = member["providerReadReceipts"]["read"].take();
                result["receipt"]["operation"] = json!(OPERATION);
                result["receipt"]["ticket"]["method"] = json!("thread/unsubscribe");
                result["result"]["value"] = json!({});
                member["providerReadReceipts"] = json!({(OPERATION):result});
                studio_runtime_store::AccountTransferRepository::put(
                    tx,
                    "move",
                    &operation.to_string(),
                )
            })
            .await
            .expect("written source unsubscribe");
        let session = ReconnectedSession(AtomicUsize::new(0));
        let before = store
            .read_async("before unsubscribe replay", |tx| {
                driver::load(tx, "move").map(|s| s.record().clone())
            })
            .await
            .expect("operation");
        let (receipt, value) = session_reply(
            &ProviderReadContext {
                store: &store,
                session: &session,
                transfer: "move",
                agent: "member",
                account: "source",
                now: 2.0,
            },
            ProviderReadCall {
                actor: None,
                transfer_source_thread: Some("source-thread".into()),
                operation: OPERATION.into(),
                method: Method::ThreadUnsubscribe,
                params: json!({"threadId":"source-thread"}),
                deadline: Duration::from_secs(1),
            },
        )
        .await
        .expect("replay without another unsubscribe");
        assert_eq!(receipt.operation, OPERATION);
        assert_eq!(receipt.scope.connection, "old-connection");
        assert_eq!(
            receipt.ticket().expect("ticket").method,
            Method::ThreadUnsubscribe
        );
        assert!(receipt.replayed);
        assert_eq!(value, json!({}));
        assert_eq!(session.0.load(Ordering::SeqCst), 0);
        let after = store
            .read_async("after unsubscribe replay", |tx| {
                driver::load(tx, "move").map(|s| s.record().clone())
            })
            .await
            .expect("operation");
        assert_eq!(
            studio_json::to_vec(&after).unwrap(),
            studio_json::to_vec(&before).unwrap()
        );
        store.transaction(|tx| {
            let mut operation = studio_runtime_store::AccountTransferRepository::get(tx, "move")?.unwrap();
            operation["members"]["member"]["providerReadReceipts"][OPERATION]["result"]["value"] = Value::Null;
            studio_runtime_store::AccountTransferRepository::put(tx, "move", &operation.to_string())
        }).await.unwrap();
        let (_, null_reply) = session_reply(
            &ProviderReadContext {
                store: &store,
                session: &session,
                transfer: "move",
                agent: "member",
                account: "source",
                now: 2.0,
            },
            ProviderReadCall {
                actor: None,
                transfer_source_thread: Some("source-thread".into()),
                operation: OPERATION.into(),
                method: Method::ThreadUnsubscribe,
                params: json!({"threadId":"source-thread"}),
                deadline: Duration::from_secs(1),
            },
        )
        .await
        .expect("null reply replays");
        assert_eq!(null_reply, Value::Null);
        assert_eq!(session.0.load(Ordering::SeqCst), 0);
        for field in ["sourceAccountKey", "sourceThreadId"] {
            let mut changed = before.clone();
            let member = changed.members.get_mut("member").unwrap();
            match field {
                "sourceAccountKey" => {
                    member.source_account_key = "another-source".to_owned().try_into().unwrap()
                }
                "sourceThreadId" => {
                    member.source_thread_id =
                        studio_runtime_schema::queue_command::Field::Value("another-source".into())
                }
                _ => unreachable!("fixture field"),
            }
            store
                .transaction(move |tx| {
                    studio_runtime_store::AccountTransferRepository::put(
                        tx,
                        "move",
                        &studio_json::to_string(&changed).unwrap(),
                    )
                })
                .await
                .expect("changed source ownership");
            assert!(
                session_reply(
                    &ProviderReadContext {
                        store: &store,
                        session: &session,
                        transfer: "move",
                        agent: "member",
                        account: "source",
                        now: 3.0
                    },
                    ProviderReadCall {
                        actor: None,
                        transfer_source_thread: Some("source-thread".into()),
                        operation: OPERATION.into(),
                        method: Method::ThreadUnsubscribe,
                        params: json!({"threadId":"source-thread"}),
                        deadline: Duration::from_secs(1)
                    }
                )
                .await
                .is_err(),
                "{field}"
            );
            assert_eq!(session.0.load(Ordering::SeqCst), 0);
        }
    }

    #[tokio::test]
    async fn written_replay_never_authorizes_other_native_mutations() {
        for method in [
            Method::ThreadStart,
            Method::ThreadFork,
            Method::ThreadUnsubscribe,
        ] {
            let (_root, store, _) = fixture("").await;
            let session = ReconnectedSession(AtomicUsize::new(0));
            assert!(
                session_reply(
                    &ProviderReadContext {
                        store: &store,
                        session: &session,
                        transfer: "move",
                        agent: "member",
                        account: "source",
                        now: 2.0
                    },
                    ProviderReadCall {
                        actor: None,
                        transfer_source_thread: None,
                        operation: "read".into(),
                        method,
                        params: json!({"threadId":"source-thread"}),
                        deadline: Duration::from_secs(1)
                    }
                )
                .await
                .is_err()
            );
            assert_eq!(session.0.load(Ordering::SeqCst), 0);
        }
    }
}

async fn session_reply(
    context: &ProviderReadContext<'_>,
    call: ProviderReadCall,
) -> Result<(SessionReceipt, Value), TransferError> {
    let scope = context
        .session
        .current_scope(context.account)
        .ok_or(TransferError::Requires("provider-session/account-worker"))?;
    if scope.account != context.account {
        return Err(TransferError::Refused(
            "Provider read account scope changed",
        ));
    }
    let method_name = call.method.as_str();
    let replay = driver::provider_read_intent(
        context.store,
        driver::ProviderReadIntent {
            transfer: context.transfer,
            agent: context.agent,
            operation: &call.operation,
            account: context.account,
            scope: scope.clone(),
            method: method_name,
            params: call.params.clone(),
        },
        context.now,
    )
    .await?;
    if let Some(saved) = replay {
        use studio_runtime_store::transfer_record::ResponseKind;
        let result = saved
            .result
            .value()
            .ok_or(TransferError::Refused("Invalid saved provider receipt"))?;
        if matches!(result.kind(), ResponseKind::Rejected) {
            eprintln!(
                "account transfer read {} replays a provider rejection",
                call.operation
            );
            return Err(TransferError::Refused(
                "The provider rejected a transfer read",
            ));
        }
        let value: Value = match result {
            studio_runtime_store::transfer_record::ProviderResult::Inline {
                value: None, ..
            } => Value::Null,
            studio_runtime_store::transfer_record::ProviderResult::Inline {
                value: Some(value),
                ..
            } => studio_json::from_str(value.as_str())
                .map_err(|_| TransferError::Refused("Invalid saved provider receipt"))?,
            studio_runtime_store::transfer_record::ProviderResult::Blob { .. } => {
                return Err(TransferError::Refused("Invalid saved provider receipt"));
            }
        };
        let stored_ticket = saved
            .receipt
            .ticket
            .value()
            .ok_or(TransferError::Refused("Invalid saved provider ticket"))?;
        let ticket = studio_runtime_providers::RequestTicket {
            generation: stored_ticket.generation,
            id: stored_ticket.id.clone(),
            method: Method::from_wire(stored_ticket.method.as_str())
                .ok_or(TransferError::Refused("Invalid saved provider ticket"))?,
        };
        return Ok((
            SessionReceipt {
                scope: saved.receipt.scope,
                operation: saved.receipt.operation,
                replayed: true,
                outcome: ReceiptOutcome::Written {
                    ticket,
                    response: ProviderResponse::Reply(value.clone()),
                },
            },
            value,
        ));
    }
    let admission = if driver::source_call_needs_admission(call.method.as_str()) {
        driver::provider_admission(
            context.store,
            context.transfer,
            context.agent,
            &call.operation,
        )
        .await?
    } else {
        driver::ProviderAdmission {
            root_request_id: String::new(),
            admitted_request: studio_runtime_schema::queue_command::Field::Null,
        }
    };
    let mut last_receipt = None;
    for attempt in 0..2 {
        let receipt = (if let Some(thread_id) = &call.transfer_source_thread {
            context
                .session
                .transfer_provider_request(TransferProviderRequest {
                    root_request_id: admission.root_request_id.clone(),
                    admitted_request: admission.admitted_request.value().cloned(),
                    transfer_id: context.transfer.into(),
                    member: context.agent.into(),
                    source_account: context.account.into(),
                    thread_id: thread_id.clone(),
                    operation: call.operation.clone(),
                    expected_scope: scope.clone(),
                    method: call.method,
                    params: call.params.clone(),
                    deadline: Instant::now() + call.deadline,
                })
                .await
        } else {
            context
                .session
                .provider_request(ProviderRequest {
                    root_request_id: admission.root_request_id.clone(),
                    admitted_request: admission.admitted_request.value().cloned(),
                    account: context.account.into(),
                    operation: call.operation.clone(),
                    actor: call.actor.clone(),
                    deleted_actor_session: None,
                    expected_scope: Some(scope.clone()),
                    method: call.method,
                    params: call.params.clone(),
                    deadline: Instant::now() + call.deadline,
                })
                .await
        })
        .map_err(TransferError::from)?;
        let certainty = receipt.certainty();
        let receipt = driver::provider_read_receipt(
            context.store,
            driver::ProviderReadReceipt {
                transfer: context.transfer,
                agent: context.agent,
                operation: &call.operation,
                receipt,
            },
            context.now,
        )
        .await?;
        if certainty == RequestCertainty::NotSent && attempt == 0 {
            last_receipt = Some(receipt);
            continue;
        }
        if receipt.operation != call.operation
            || receipt.scope != scope
            || receipt.certainty() != RequestCertainty::Written
        {
            eprintln!(
                "account transfer read {} not written: certainty {:?}, not-sent reason {:?}, same scope {}",
                call.operation,
                receipt.certainty(),
                receipt.not_sent_reason(),
                receipt.scope == scope
            );
            return Err(TransferError::Refused(
                "The retained read receipt is not Written on the expected connection",
            ));
        }
        let response = match receipt.response().cloned() {
            Some(ProviderResponse::Reply(value)) => value,
            Some(ProviderResponse::Rejected(rejection)) => {
                eprintln!(
                    "account transfer read {} rejected by the provider: {rejection:?}",
                    call.operation
                );
                return Err(TransferError::Refused(
                    "The provider rejected a transfer read",
                ));
            }
            None => {
                return Err(TransferError::Refused(
                    "The provider rejected a transfer read",
                ));
            }
        };
        return Ok((receipt, response));
    }
    let receipt = last_receipt.ok_or(TransferError::Refused("Provider read was not sent"))?;
    if receipt.certainty() == RequestCertainty::NotSent {
        return Err(TransferError::Refused(
            "Provider read was not sent after retry",
        ));
    }
    Err(TransferError::Refused(
        "The provider read outcome is unknown",
    ))
}

struct SourceTransferCall<'a> {
    thread: &'a str,
    suffix: &'a str,
    method: Method,
    params: Value,
}

async fn source_transfer_call(
    context: &ProviderReadContext<'_>,
    call: SourceTransferCall<'_>,
) -> Result<Value, TransferError> {
    let (_, response) = session_reply(
        context,
        ProviderReadCall {
            actor: None,
            transfer_source_thread: Some(call.thread.into()),
            operation: format!(
                "transfer:{}:{}:{}",
                context.transfer, context.agent, call.suffix
            ),
            method: call.method,
            params: call.params,
            deadline: READ_DEADLINE,
        },
    )
    .await?;
    Ok(response)
}

async fn read_catalog(
    context: &ProviderReadContext<'_>,
    provider: ProviderKind,
    home: &str,
) -> Result<CatalogObservation, TransferError> {
    let mut cursor: Option<String> = None;
    let mut seen = std::collections::HashSet::new();
    let mut rows = Vec::new();
    for page in 0..16 {
        let mut params =
            json!({"limit":studio_runtime_schema::shared_values::MODEL_CATALOG_ITEMS_PER_PAGE});
        if let Some(value) = &cursor {
            params["cursor"] = json!(value);
        }
        let operation = format!(
            "account-transfer:{}:{}:model-list:{page}",
            context.transfer, context.agent
        );
        let (_, response) = session_reply(
            context,
            ProviderReadCall {
                actor: None,
                transfer_source_thread: None,
                operation,
                method: Method::ModelList,
                params,
                deadline: READ_DEADLINE,
            },
        )
        .await?;
        let page = studio_runtime_schema::model_catalog::ModelCatalog::parse(response)
            .map_err(|_| TransferError::Refused("Invalid destination model catalog"))?;
        rows.extend(page.data);
        match page.next_cursor.flatten() {
            None => {
                let models = rows.iter().map(ModelCapabilities::from).collect();
                return Ok(CatalogObservation {
                    identity: CatalogIdentity {
                        account_key: context.account.into(),
                        home: home.into(),
                        provider,
                        generation: 0,
                    },
                    expires_at: Instant::now() + Duration::from_secs(30),
                    models,
                });
            }
            Some(value) if !value.is_empty() && seen.insert(value.clone()) => {
                cursor = Some(value.clone())
            }
            _ => return Err(TransferError::Refused("Invalid destination model cursor")),
        }
    }
    Err(TransferError::Refused(
        "Destination catalog pagination limit exceeded",
    ))
}

fn destination_settings(
    agent: &studio_runtime_store::AgentRecord,
    _source_account: &str,
    source_provider: ProviderKind,
    target_provider: ProviderKind,
    catalog: &CatalogObservation,
) -> Result<studio_runtime_store::transfer_record::DestinationSettings, TransferError> {
    use crate::account_transfer::{known_transfer_object, known_transfer_value, worker_defaults};
    use studio_runtime_schema::queue_command::Field;
    use studio_runtime_store::transfer_record::{DestinationSettings, TransferSettings};
    let changed = source_provider != target_provider;
    let visible = catalog
        .models
        .iter()
        .filter(|model| !model.hidden)
        .collect::<Vec<_>>();
    let current = agent.model.as_deref().and_then(|name| {
        visible
            .iter()
            .find(|model| model.model == name || model.resolved_model.as_deref() == Some(name))
    });
    let chosen = if changed || current.is_none() {
        visible.iter().find(|model| model.is_default).or_else(|| {
            let fallback = crate::provider_adapter::default_model(target_provider);
            visible.iter().find(|model| model.model == fallback)
        })
    } else {
        current
    };
    let model = chosen.ok_or(TransferError::Refused(
        "The destination account has no available default model",
    ))?;
    let selected_model = if !changed && model.resolved_model.as_deref() == agent.model.as_deref() {
        agent.model.clone().unwrap_or_default()
    } else {
        model.model.clone()
    };
    let settings = ExecutionSettings {
        model: StoredField::Value(selected_model.clone()),
        effort: if changed {
            None
        } else {
            agent.effort.value().cloned()
        },
        native_effort: StoredField::Missing,
        fast_mode: Some(!changed && agent.fast_mode == Some(true) && model.priority),
        daybreak_enabled: Some(!changed && agent.daybreak_enabled == Some(true)),
    };
    let (effort, native_effort) =
        gates::validate_execution(&settings, catalog, changed || settings.effort.is_none())
            .map_err(|_| TransferError::Requires("account-transfer/destination-settings"))?;
    let account_key = catalog
        .identity
        .account_key
        .clone()
        .try_into()
        .map_err(|_| TransferError::Refused("Invalid destination account"))?;
    let mut defaults = worker_defaults(agent)?.value().cloned().unwrap_or_default();
    if changed {
        defaults.model = Field::Null;
        defaults.effort = Field::Value("medium".into());
        defaults.fast_mode = Field::Value(false);
        defaults.daybreak_enabled = Field::Value(false);
        defaults.cyber_access_program = Field::Value("standard".into());
    }
    let daybreak = !changed && agent.daybreak_enabled == Some(true);
    let cyber_program = match target_provider {
        ProviderKind::Claude => {
            if daybreak {
                return Err(TransferError::Requires(
                    "account-transfer/destination-settings",
                ));
            }
            "standard".to_owned()
        }
        ProviderKind::Codex => {
            let grants = catalog
                .models
                .iter()
                .find(|row| row.model == selected_model && !row.hidden)
                .and_then(|row| row.access_programs.as_ref());
            if daybreak {
                grants
                    .and_then(|programs| {
                        ["daybreakBlue", "daybreakRed"]
                            .into_iter()
                            .find(|p| programs.iter().any(|v| v == p))
                    })
                    .ok_or(TransferError::Requires(
                        "account-transfer/destination-settings",
                    ))?
                    .to_owned()
            } else if grants
                .is_some_and(|programs| !programs.iter().any(|value| value == "standard"))
            {
                return Err(TransferError::Requires(
                    "account-transfer/destination-settings",
                ));
            } else {
                "standard".to_owned()
            }
        }
    };
    let mut result = DestinationSettings {
        settings: TransferSettings {
            provider: Field::Value(target_provider),
            model: Field::Value(selected_model),
            effort: effort.map_or(Field::Null, Field::Value),
            native_effort: native_effort.map_or(Field::Null, Field::Value),
            fast_mode: Field::Value(settings.fast_mode.unwrap_or(false)),
            daybreak_enabled: Field::Value(settings.daybreak_enabled.unwrap_or(false)),
            cyber_access_program: Field::Value(cyber_program),
            worker_defaults: Field::Value(defaults),
            ..TransferSettings::default()
        },
        pending_settings: if changed { Field::Null } else { Field::Missing },
        pending_settings_account_key: if changed { Field::Null } else { Field::Missing },
    };
    if !changed
        && let Some(mut pending) =
            known_transfer_object(&agent.pending_settings, "pendingSettings")?
                .value()
                .cloned()
    {
        let pending_account = known_transfer_value(
            &agent.pending_settings_account_key,
            "pendingSettingsAccountKey",
        )?;
        if pending_account
            .value()
            .is_some_and(|key| key.as_str() != catalog.identity.account_key)
        {
            return Err(TransferError::Refused(
                "Queued settings belong to another account. Save them again before transfer",
            ));
        }
        let pending_exec = ExecutionSettings {
            model: StoredField::Value(pending.model.value().cloned().unwrap_or_default()),
            effort: pending.effort.value().cloned(),
            native_effort: StoredField::Missing,
            fast_mode: Some(pending.fast_mode.value().copied().unwrap_or(false)),
            daybreak_enabled: Some(pending.daybreak_enabled.value().copied().unwrap_or(false)),
        };
        let (pending_effort, pending_native) =
            gates::validate_execution(&pending_exec, catalog, false)
                .map_err(|_| TransferError::Requires("account-transfer/destination-settings"))?;
        pending.effort = pending_effort.map_or(Field::Null, Field::Value);
        pending.native_effort = pending_native.map_or(Field::Null, Field::Value);
        if pending.cyber_access_program.is_missing() {
            pending.cyber_access_program = Field::Value("standard".into());
        }
        result.pending_settings = Field::Value(*pending);
        result.pending_settings_account_key = Field::Value(account_key);
    }
    Ok(result)
}

impl TransferExecutor for AccountTransferExecutor {
    fn available(&self) -> Result<(), TransferError> {
        Ok(())
    }

    fn prepare<'a>(
        &'a self,
        input: &'a TransferPreparation,
    ) -> driver::TransferFuture<'a, Result<PreparedTransfer, TransferError>> {
        Box::pin(async move {
            #[cfg(test)]
            if let Some(error) = self.session.transfer_preparation_error() {
                return Err(TransferError::Store(error));
            }
            let operation = input.operation();
            let agent = input.agent();
            let agent_id = agent.id.as_ref();
            let member = operation
                .members
                .get(agent_id)
                .ok_or(TransferError::Refused("Unknown transfer member"))?;
            let epoch = agent.epoch;
            let transfer_id = operation.id.as_str();
            let source_account = member.source_account_key.as_str();
            let target_account = operation.target_account_key.as_str();
            let source_provider = member
                .provider
                .value()
                .copied()
                .ok_or(TransferError::Refused("Invalid transfer provider"))?;
            let target_provider = operation
                .target_provider
                .value()
                .copied()
                .ok_or(TransferError::Refused("Invalid transfer provider"))?;
            if let Some(accounts) = &self.preparation.account_driver {
                // The registry snapshot is already validated and performs no auth refresh.
                // Transfer preparation must not depend on an earlier HTTP model read.
                let snapshot = accounts.snapshot().map_err(|_| {
                    TransferError::Refused(
                        "The account registry is unavailable for transfer preparation",
                    )
                })?;
                self.catalog.retain_public_accounts(&snapshot.accounts);
            }
            let (home, registered_provider) =
                self.catalog
                    .account_profile(target_account)
                    .ok_or(TransferError::Requires(
                        "account-transfer/destination-settings",
                    ))?;
            if registered_provider != target_provider {
                return Err(TransferError::Refused(
                    "Destination provider changed during transfer",
                ));
            }
            let catalog_context = ProviderReadContext {
                store: self.store.as_ref(),
                session: &*self.session,
                transfer: transfer_id,
                agent: agent_id,
                account: target_account,
                now: self.now,
            };
            let catalog = read_catalog(&catalog_context, target_provider, &home).await?;
            let settings = destination_settings(
                agent,
                source_account,
                source_provider,
                target_provider,
                &catalog,
            )?;

            let mut portable = None;
            let mut history_text = None;
            let mut transcript_source = None;
            let mut copied_history_path = None;
            let mut claude_export = None;
            let source_thread = member.source_thread_id.value().map(String::as_str);
            if let Some(thread_id) = source_thread {
                // No other agent may use the source account any more, so its
                // worker can be stopped. Start it before the source reads.
                if self.session.current_scope(source_account).is_none() {
                    self.session
                        .ensure_session(source_account)
                        .await
                        .map_err(TransferError::from)?;
                }
                let source_context = ProviderReadContext {
                    store: self.store.as_ref(),
                    session: &*self.session,
                    transfer: transfer_id,
                    agent: agent_id,
                    account: source_account,
                    now: self.now,
                };
                let operation_id = format!("account-transfer:{transfer_id}:{agent_id}:thread-read");
                let (_, read) = session_reply(
                    &source_context,
                    ProviderReadCall {
                        actor: None,
                        transfer_source_thread: None,
                        operation: operation_id,
                        method: Method::ThreadRead,
                        params: json!({"threadId":thread_id,"includeTurns":false}),
                        deadline: READ_DEADLINE,
                    },
                )
                .await?;
                let thread = read
                    .get("thread")
                    .filter(|thread| thread["id"] == thread_id)
                    .ok_or(TransferError::Refused("Source thread identity changed"))?;
                if !matches!(
                    thread["status"]["type"].as_str(),
                    Some("idle" | "notLoaded" | "systemError")
                ) {
                    return Err(TransferError::Refused("Source thread is not idle"));
                }
                let queue = source_transfer_call(
                    &source_context,
                    SourceTransferCall {
                        thread: thread_id,
                        suffix: "queue-before",
                        method: Method::ThreadQueueList,
                        params: json!({"threadId":thread_id}),
                    },
                )
                .await?;
                if !source_page_empty(&queue, "Invalid source native queue page")? {
                    return Err(TransferError::Refused(
                        "Source native thread has queued input; saved input remains queued",
                    ));
                }
                if matches!(
                    thread["status"]["type"].as_str(),
                    Some("idle" | "systemError")
                ) {
                    let background = source_transfer_call(
                        &source_context,
                        SourceTransferCall {
                            thread: thread_id,
                            suffix: "background-before",
                            method: Method::ThreadBackgroundTerminalsList,
                            params: json!({"threadId":thread_id,"limit":studio_runtime_schema::shared_values::NATIVE_TERMINAL_ITEMS_PER_PAGE}),
                        },
                    )
                    .await?;
                    // A running background command stays with the source: the thread
                    // stays attached so the command finishes there, and the chat moves on.
                    if source_page_empty(&background, "Invalid source background command page")? {
                        source_transfer_call(
                            &source_context,
                            SourceTransferCall {
                                thread: thread_id,
                                suffix: "unsubscribe",
                                method: Method::ThreadUnsubscribe,
                                params: json!({"threadId":thread_id}),
                            },
                        )
                        .await?;
                    } else {
                        eprintln!(
                            "account transfer of thread {thread_id} leaves a background command running on the source"
                        );
                    }
                }
                let queue = source_transfer_call(
                    &source_context,
                    SourceTransferCall {
                        thread: thread_id,
                        suffix: "queue-after",
                        method: Method::ThreadQueueList,
                        params: json!({"threadId":thread_id}),
                    },
                )
                .await?;
                if !source_page_empty(&queue, "Invalid source native queue page")? {
                    return Err(TransferError::Refused(
                        "Source native thread has queued input; saved input remains queued",
                    ));
                }
                let portable_required = source_provider != target_provider
                    || source_provider == ProviderKind::Claude
                    || target_provider == ProviderKind::Claude;
                if !portable_required {
                    let native_path = thread.get("path").and_then(Value::as_str).ok_or(
                        TransferError::Refused("Codex returned no saved context path"),
                    )?;
                    let source_home = self
                        .catalog
                        .account_profile(source_account)
                        .map(|(home, _)| home)
                        .ok_or(TransferError::Requires(
                            "account-transfer/native-history-copy",
                        ))?;
                    let target_home = home.clone();
                    let native_path = native_path.to_owned();
                    copied_history_path = Some(
                        run_transfer_blocking(move || {
                            copy_native_history(&source_home, &target_home, &native_path)
                        })
                        .await?,
                    );
                }
                // A Claude move copies the native file, so it reads no turn pages.
                let claude_move = source_provider == ProviderKind::Claude
                    && target_provider == ProviderKind::Claude;
                claude_export = if claude_move {
                    Some(
                        source_transfer_call(
                            &source_context,
                            SourceTransferCall {
                                thread: thread_id,
                                suffix: "move-export",
                                method: Method::ClaudeMoveExport,
                                params: json!({"threadId":thread_id}),
                            },
                        )
                        .await?,
                    )
                } else {
                    None
                };
                let pages = if claude_move { 0 } else { 1024 };
                let mut records = vec![json!({"kind":"native_thread","thread":thread})];
                let mut cursor: Option<String> = None;
                let mut cursors = std::collections::HashSet::new();
                for page in 0..pages {
                    let mut params = json!({"threadId":thread_id,"limit":20,"sortDirection":"desc","itemsView":"full"});
                    if let Some(value) = &cursor {
                        params["cursor"] = json!(value);
                    }
                    let (_, page_value) = session_reply(
                        &source_context,
                        ProviderReadCall {
                            actor: None,
                            transfer_source_thread: None,
                            operation: format!(
                                "account-transfer:{transfer_id}:{agent_id}:turns:{page}"
                            ),
                            method: Method::ThreadTurnsList,
                            params: params.clone(),
                            deadline: READ_DEADLINE,
                        },
                    )
                    .await?;
                    let decoded: HistoryPage = serde::Deserialize::deserialize(&page_value)
                        .map_err(|_| TransferError::Refused("Invalid source history page"))?;
                    records.push(
                        json!({"kind":"native_turn_page","request":params,"page":page_value}),
                    );
                    match decoded.next_cursor {
                        None => break,
                        Some(HistoryCursor::Text(value))
                            if !value.is_empty() && cursors.insert(value.clone()) =>
                        {
                            cursor = Some(value)
                        }
                        _ => {
                            return Err(TransferError::Refused(
                                "Repeated or invalid source history cursor",
                            ));
                        }
                    }
                }
                if portable_required && !claude_move {
                    let mut archive_agent = agent.value();
                    if let Some(source_state) = archive_agent
                        .get("lazyAccountTransfer")
                        .and_then(|lazy| lazy.get("sourceState"))
                        .and_then(Value::as_object)
                        .cloned()
                    {
                        for (key, value) in source_state {
                            archive_agent[key] = value;
                        }
                    }
                    archive_agent["accountKey"] = json!(source_account);
                    archive_agent["threadId"] = json!(thread_id);
                    let descriptor_settings = portable_history::source_settings_snapshot(
                        &archive_agent,
                    )
                    .map_err(|_| TransferError::Refused("Source portable history export failed"))?;
                    let descriptor_generation = self
                        .session
                        .current_scope(source_account)
                        .ok_or(TransferError::Requires("provider-session/account-worker"))?
                        .generation;
                    let native = NativeHistory {
                        account_key: source_account.to_owned(),
                        thread_id: thread_id.to_owned(),
                        epoch,
                        settings: descriptor_settings.clone(),
                        connection_generation: descriptor_generation,
                        records: Box::new(records.into_iter().map(Ok)),
                        checked_empty_proof: None,
                    };
                    let descriptor = portable_history::export_history(
                        self.store.as_ref(),
                        ExportInput {
                            agent_id: agent_id.to_owned(),
                            account_key: source_account.to_owned(),
                            epoch,
                            thread_id: Some(thread_id.to_owned()),
                            source_settings: descriptor_settings,
                            source_connection_generation: Some(descriptor_generation),
                            transfer_id: transfer_id.to_owned(),
                            native_history: Some(native),
                        },
                    )
                    .await
                    .map_err(|_| TransferError::Refused("Source portable history export failed"))?;
                    let root = self.store.root().to_path_buf();
                    let (descriptor, validated_history) = run_transfer_blocking(move || {
                        let history = portable_history::history_context(&root, &descriptor)
                            .map_err(|_| {
                                TransferError::Refused("Source portable history validation failed")
                            })?;
                        Ok((descriptor, history))
                    })
                    .await?;
                    history_text = Some(validated_history);
                    transcript_source = Some(descriptor.clone());
                    portable = Some(descriptor.to_value().map_err(|_| {
                        TransferError::Refused("Invalid portable archive descriptor")
                    })?);
                }
            }
            // The retained settings read above does not populate the separate
            // admission cache. Refresh its exact destination identity before
            // native preparation reads and rechecks that cache generation.
            match self
                .preparation
                .model_catalog
                .wait_for_admission_catalog(target_account, &home, target_provider)
                .await
            {
                crate::model_catalog::AdmissionCatalogRead::Fresh { .. } => {}
                crate::model_catalog::AdmissionCatalogRead::Pending => {
                    return Err(TransferError::Requires(
                        crate::model_catalog::MODEL_CATALOG_DEPENDENCY,
                    ));
                }
                crate::model_catalog::AdmissionCatalogRead::Unavailable => {
                    return Err(TransferError::Refused(
                        crate::model_catalog::MODEL_CATALOG_DEPENDENCY,
                    ));
                }
            }
            let scope = self
                .session
                .current_scope(target_account)
                .ok_or(TransferError::Requires("provider-session/account-worker"))?;
            let attempt_id = format!("account-transfer:{transfer_id}:{agent_id}");
            let generation = scope.generation;
            let prepared = Box::pin(
                crate::account_worker::preparation::prepare_account_transfer_native_thread(
                    Arc::clone(&self.store),
                    &self.preparation,
                    agent_id,
                    transfer_id,
                    &attempt_id,
                    generation,
                ),
            )
            .await
            .map_err(|_| TransferError::Refused("Destination thread preparation failed"))?;
            let mut params = prepared.params;
            params["model"] = json!(settings.settings.model.value());
            if let Some(config) = params.get_mut("config").and_then(Value::as_object_mut) {
                if let Some(effort) = settings.settings.native_effort.value() {
                    config.insert("model_reasoning_effort".into(), json!(effort));
                }
                config.insert(
                    "features.fast_mode".into(),
                    json!(settings.settings.fast_mode.value() == Some(&true)),
                );
            }
            params["serviceTier"] = json!(if settings.settings.fast_mode.value() == Some(&true) {
                "priority"
            } else {
                "default"
            });
            if let Some(text) = history_text {
                let mut instructions = text;
                if let Some(source) = transcript_source {
                    // The model reads the full history from a file in the destination folder.
                    let project = PathBuf::from(
                        params["cwd"]
                            .as_str()
                            .ok_or(TransferError::Refused("Destination folder is missing"))?,
                    );
                    let root = self.store.root().to_path_buf();
                    let transcript = run_transfer_blocking(move || {
                        portable_history::write_readable_transcript(&root, &source, &project)
                            .map_err(|_| {
                                TransferError::Refused("Source transcript file write failed")
                            })
                    })
                    .await?;
                    instructions.push_str(&format!(
                        "\n\nThe complete earlier conversation is in this read-only file: {}. Read it when earlier context is needed.",
                        transcript.display()
                    ));
                }
                params["developerInstructions"] = json!(instructions);
            }
            if let Some(path) = copied_history_path {
                params["path"] = json!(path);
            }
            if let Some(history) = &portable {
                params["portableHistory"] = history.clone();
            }
            let method = if claude_export.is_some() {
                TransferMethod::ClaudeImport
            } else if source_thread.is_some() && portable.is_none() {
                TransferMethod::Fork
            } else {
                TransferMethod::Start
            };
            if let Some(export) = &claude_export {
                params = claude_import_params(
                    export,
                    &params["cwd"],
                    &json!(settings.settings.model.value()),
                )?;
            }
            if method == TransferMethod::Fork {
                if let Some(object) = params.as_object_mut() {
                    drop(object.remove("dynamicTools"));
                }
                params["threadId"] = json!(source_thread);
                params["excludeTurns"] = json!(true);
                params["deferGoalContinuation"] = json!(true);
            }
            driver::prepared_member(
                self.store.as_ref(),
                transfer_id,
                agent_id,
                settings,
                portable,
                self.now,
            )
            .await?;
            let scope = self
                .session
                .current_scope(target_account)
                .ok_or(TransferError::Requires("provider-session/account-worker"))?;
            let operation_id = format!(
                "account-transfer:{transfer_id}:{agent_id}:{}",
                method.wire()
            );
            Ok(PreparedTransfer {
                ticket: crate::account_transfer::Ticket {
                    operation: transfer_id.into(),
                    agent: agent_id.into(),
                    epoch,
                    account: target_account
                        .to_owned()
                        .try_into()
                        .map_err(|_| TransferError::Refused("Invalid destination account"))?,
                    connection: scope.connection.clone(),
                    request: operation_id,
                },
                scope,
                method,
                params,
            })
        })
    }
}

impl TransferTransport for AccountTransferExecutor {
    fn send<'a>(
        &'a self,
        ticket: &'a crate::account_transfer::Ticket,
        scope: &'a crate::account_session_runtime::ConnectionScope,
        method: &'a str,
        params: &'a Value,
    ) -> driver::TransferFuture<'a, Result<TransferDispatch, TransferError>> {
        Box::pin(async move {
            let parsed = Method::from_wire(method);
            let Some(method) = parsed else {
                return Err(TransferError::Refused("Invalid transfer method"));
            };
            let admission = driver::provider_admission(
                &self.store,
                &ticket.operation,
                &ticket.agent,
                &ticket.request,
            )
            .await?;
            let request = ProviderRequest {
                root_request_id: admission.root_request_id,
                admitted_request: admission.admitted_request.value().cloned(),
                account: ticket.account.as_str().to_owned(),
                operation: ticket.request.clone(),
                actor: Some(crate::account_worker::retained::RequestActor {
                    agent: ticket.agent.clone(),
                    epoch: ticket.epoch,
                }),
                deleted_actor_session: None,
                expected_scope: Some(scope.clone()),
                method,
                params: params.clone(),
                deadline: Instant::now() + MUTATION_DEADLINE,
            };
            let session_receipt = self.session.provider_request(request).await;
            match session_receipt {
                Ok(receipt) => Ok(TransferDispatch::Receipt(receipt)),
                Err(AccountSessionError::Expired) => Err(TransferError::Expired),
                Err(AccountSessionError::Requires(name)) => Err(TransferError::Requires(name)),
                Err(AccountSessionError::Refused(reason)) => Err(TransferError::Refused(reason)),
                Err(
                    AccountSessionError::InvalidResponse(_)
                    | AccountSessionError::Persistence
                    | AccountSessionError::Unknown,
                ) => Ok(TransferDispatch::Unknown {
                    operation: ticket.request.clone(),
                    scope: scope.clone(),
                }),
            }
        })
    }
}

pub async fn before_start(
    store: Arc<RuntimeStore>,
    session: Option<Arc<dyn AccountSessionPort>>,
    catalog: &ModelCatalogService,
    preparation: &crate::account_worker::preparation::PreparationResources,
    agent_id: &str,
    now: f64,
) -> Result<Value, TransferError> {
    let mut agent = transfer_member(&store, agent_id).await?;
    if agent.account_transfer_id.value().is_some()
        && agent.lazy_account_transfer.value().is_none()
        && driver::release_waiting(&store, agent_id, now).await?
    {
        agent = transfer_member(&store, agent_id).await?;
    }
    let lazy = agent.lazy_account_transfer.value();
    if let Some(lazy) = lazy {
        let operation = lazy
            .pending()
            .ok_or(TransferError::Refused(
                "Current transfer data is unreadable",
            ))?
            .id
            .as_str();
        let owner = operation.to_owned();
        let member = agent_id.to_owned();
        let reading = store
            .transaction_with_error(move |tx| driver::owns_reading_member(tx, &owner, &member))
            .await?;
        if reading {
            // The executor already owns this read. Ordinary preparation keeps
            // its snapshot's AccountTransfer hold and cannot repeat the move.
            return Ok(agent.value());
        }
        let session = session.ok_or(TransferError::Requires("provider-session/account-worker"))?;
        let executor = AccountTransferExecutor {
            store: Arc::clone(&store),
            session,
            catalog: catalog.clone(),
            preparation: preparation.clone(),
            now,
        };
        let result =
            driver::execute_member(store.as_ref(), &executor, operation, agent_id, now).await?;
        if result == driver::DispatchOutcome::ReceiptSaved {
            driver::adopt_receipt(store.as_ref(), operation, agent_id, now).await?;
        }
    } else {
        let agent = if driver::active_summary_id(&agent).is_some() {
            driver::drop_orphan_summary(store.as_ref(), agent_id)
                .await?
                .unwrap_or(agent)
        } else {
            agent
        };
        crate::account_transfer::before_start(&agent)?;
    }
    Ok(transfer_member(&store, agent_id).await?.value())
}

async fn transfer_member(
    store: &RuntimeStore,
    agent_id: &str,
) -> Result<studio_runtime_store::AgentRecord, TransferError> {
    let agent_id = studio_runtime_store::AgentId::from(agent_id);
    store
        .read_async("runtime-services/account_transfer_executor", move |tx| {
            studio_runtime_store::AgentRepository::get(tx, &agent_id)?
                .ok_or(TransferError::Refused("Unknown transfer member"))
        })
        .await
}

#[cfg(test)]
mod destination_settings_tests {
    use super::*;

    #[test]
    fn same_provider_preserves_and_normalizes_queued_settings() {
        let agent = studio_json::from_str(r#"{"id":"agent","rootId":"agent","epoch":0,"status":"idle","compactions":0,"accountKey":"source","model":"model-a","fastMode":true,"workerDefaults":{"model":"child"},"pendingSettings":{"model":"model-a","fastMode":true},"pendingSettingsAccountKey":"target"}"#).unwrap();
        let catalog = CatalogObservation {
            identity: CatalogIdentity {
                account_key: "target".into(),
                home: "/target".into(),
                provider: ProviderKind::Codex,
                generation: 1,
            },
            expires_at: Instant::now() + Duration::from_secs(30),
            models: vec![ModelCapabilities {
                model: "model-a".into(),
                resolved_model: None,
                is_default: true,
                hidden: false,
                access_programs: None,
                supported_efforts: vec!["medium".into()],
                default_effort: Some("medium".into()),
                priority: true,
            }],
        };
        let result = destination_settings(
            &agent,
            "source",
            ProviderKind::Codex,
            ProviderKind::Codex,
            &catalog,
        )
        .unwrap();
        assert_eq!(
            result.settings.model.value().map(String::as_str),
            Some("model-a")
        );
        assert_eq!(result.settings.fast_mode.value(), Some(&true));
        assert_eq!(
            result
                .settings
                .worker_defaults
                .value()
                .unwrap()
                .model
                .value()
                .map(String::as_str),
            Some("child")
        );
        let pending = result.pending_settings.value().unwrap();
        assert_eq!(pending.model.value().map(String::as_str), Some("model-a"));
        assert_eq!(
            pending.cyber_access_program.value().map(String::as_str),
            Some("standard")
        );
        assert_eq!(
            result
                .pending_settings_account_key
                .value()
                .unwrap()
                .as_str(),
            "target"
        );
    }

    fn claude_target_catalog() -> CatalogObservation {
        CatalogObservation {
            identity: CatalogIdentity {
                account_key: "target".into(),
                home: "/target".into(),
                provider: ProviderKind::Claude,
                generation: 1,
            },
            expires_at: Instant::now() + Duration::from_secs(30),
            models: vec![ModelCapabilities {
                model: "model-a".into(),
                resolved_model: None,
                is_default: true,
                hidden: false,
                access_programs: None,
                supported_efforts: vec!["medium".into()],
                default_effort: Some("medium".into()),
                priority: false,
            }],
        }
    }

    #[test]
    fn transfer_keeps_worker_defaults_naming_another_account() {
        let agent = studio_json::from_str(r#"{"id":"agent","rootId":"agent","epoch":0,"status":"idle","compactions":0,"accountKey":"source","model":"model-a","workerDefaults":{"accountKey":"codex-login","model":"gpt-6.1-sol"}}"#).unwrap();
        let result = destination_settings(
            &agent,
            "source",
            ProviderKind::Claude,
            ProviderKind::Claude,
            &claude_target_catalog(),
        )
        .unwrap();
        let defaults = result.settings.worker_defaults.value().unwrap();
        assert_eq!(
            defaults.model.value().map(String::as_str),
            Some("gpt-6.1-sol")
        );
    }

    #[test]
    fn transfer_preserves_worker_model_on_same_provider() {
        let agent = studio_json::from_str(r#"{"id":"agent","rootId":"agent","epoch":0,"status":"idle","compactions":0,"accountKey":"source","model":"model-a","workerDefaults":{"model":"model-a"}}"#).unwrap();
        let result = destination_settings(
            &agent,
            "source",
            ProviderKind::Claude,
            ProviderKind::Claude,
            &claude_target_catalog(),
        )
        .unwrap();
        let defaults = result.settings.worker_defaults.value().unwrap();
        assert_eq!(defaults.model.value().map(String::as_str), Some("model-a"));
    }

    #[test]
    fn provider_change_settings_can_be_prepared() {
        let operation = json!({"id":"transfer","leadId":"agent","targetAccountKey":"target",
            "status":"pending","created":1.0,"scope":"team",
            "members":{"agent":{"phase":"lazy","sourceAccountKey":"source","lazy":true}}});
        let agent = json!({"compactions":0,"provider":"codex","id":"agent","rootId":"agent",
            "epoch":7,"status":"idle","accountKey":"target","accountTransferId":"transfer",
            "lazyAccountTransfer":{"id":"transfer","sourceAccountKey":"source"},"model":"source-model"});
        let snapshot = crate::account_transfer::Snapshot::read(
            &operation.to_string(),
            [("agent".into(), agent.to_string())],
        )
        .unwrap();
        let decision = snapshot.begin_read("agent").unwrap();
        let reading = crate::account_transfer::Snapshot::read(
            &decision.encoded().unwrap(),
            decision
                .agents
                .into_iter()
                .map(|(id, agent)| (id, studio_json::to_string(&agent).unwrap())),
        )
        .unwrap();
        let catalog = CatalogObservation {
            identity: CatalogIdentity {
                account_key: "target".into(),
                home: std::env::temp_dir()
                    .join("target")
                    .to_string_lossy()
                    .into_owned(),
                provider: ProviderKind::Claude,
                generation: 1,
            },
            expires_at: Instant::now() + Duration::from_secs(30),
            models: vec![ModelCapabilities {
                model: "destination".into(),
                resolved_model: None,
                is_default: true,
                hidden: false,
                access_programs: None,
                supported_efforts: vec!["medium".into()],
                default_effort: Some("medium".into()),
                priority: false,
            }],
        };
        let settings = destination_settings(
            reading.agent("agent").unwrap(),
            "source",
            ProviderKind::Codex,
            ProviderKind::Claude,
            &catalog,
        )
        .unwrap();
        reading
            .prepared_member("agent", settings, None)
            .expect("provider change settings are accepted");
    }

    #[test]
    fn missing_default_and_fallback_model_refuses_instead_of_selecting_first() {
        let catalog = CatalogObservation {
            identity: CatalogIdentity {
                account_key: "target".into(),
                home: "/tmp/target".into(),
                provider: ProviderKind::Codex,
                generation: 1,
            },
            expires_at: Instant::now() + Duration::from_secs(30),
            models: vec![ModelCapabilities {
                model: "visible-but-not-default".into(),
                resolved_model: None,
                is_default: false,
                hidden: false,
                access_programs: None,
                supported_efforts: vec!["medium".into()],
                default_effort: Some("medium".into()),
                priority: false,
            }],
        };
        let result = destination_settings(
            &studio_json::from_str(r#"{"id":"agent","rootId":"agent","epoch":0,"status":"idle","compactions":0,"accountKey":"source","model":"source-model"}"#).unwrap(),
            "source",
            ProviderKind::Claude,
            ProviderKind::Codex,
            &catalog,
        );
        assert!(matches!(
            result,
            Err(TransferError::Refused(
                "The destination account has no available default model"
            ))
        ));
    }

    #[test]
    fn native_history_copy_refuses_present_null_ancestry_before_publish() {
        let root = tempfile::tempdir().expect("fixture");
        let source = root.path().join("source");
        let target = root.path().join("target");
        let history = source.join("sessions").join("thread.jsonl");
        fs::create_dir_all(history.parent().expect("parent")).expect("source");
        fs::create_dir_all(&target).expect("target");
        fs::write(
            &history,
            b"{\"type\":\"session_meta\",\"payload\":{\"history_base\":null}}\n",
        )
        .expect("history");
        let result = copy_native_history(
            source.to_str().unwrap(),
            target.to_str().unwrap(),
            history.to_str().unwrap(),
        );
        assert!(matches!(
            result,
            Err(TransferError::Refused(
                "Native history ancestry requires the portable-history importer"
            ))
        ));
        assert!(!target.join("sessions").exists());
    }

    #[test]
    fn native_history_manifest_preserves_raw_header_id_and_source_bytes() {
        let root = tempfile::tempdir().unwrap();
        let source = root.path().join("source");
        let target = root.path().join("target");
        fs::create_dir_all(source.join("sessions")).unwrap();
        fs::create_dir_all(&target).unwrap();
        for (name, header, expected) in [
            (
                "missing",
                r#"{"type":"session_meta","payload":{}}"#,
                Value::Null,
            ),
            (
                "null",
                r#"{"type":"session_meta","payload":{"id":null}}"#,
                Value::Null,
            ),
            (
                "number",
                r#"{"type":"session_meta","payload":{"id":42}}"#,
                json!(42),
            ),
            (
                "boolean",
                r#"{"type":"session_meta","payload":{"id":false}}"#,
                json!(false),
            ),
            (
                "array",
                r#"{"type":"session_meta","payload":{"id":[1,null]}}"#,
                json!([1, null]),
            ),
            (
                "object",
                r#"{"type":"session_meta","payload":{"id":{"opaque":true}}}"#,
                json!({"opaque":true}),
            ),
            (
                "scalar-payload",
                r#"{"type":"session_meta","payload":false}"#,
                Value::Null,
            ),
            (
                "duplicate",
                r#"{"type":"session_meta","payload":{"id":"old","id":{"last":2}}}"#,
                json!({"last":2}),
            ),
        ] {
            let history = source.join("sessions").join(format!("{name}.jsonl"));
            let bytes = format!("{header}\n{{\"type\":\"turn\"}}\n").into_bytes();
            fs::write(&history, &bytes).unwrap();
            let copied = copy_native_history(
                source.to_str().unwrap(),
                target.to_str().unwrap(),
                history.to_str().unwrap(),
            )
            .unwrap();
            assert_eq!(fs::read(&copied).unwrap(), bytes);
            assert_eq!(
                copy_native_history(
                    source.to_str().unwrap(),
                    target.to_str().unwrap(),
                    history.to_str().unwrap()
                )
                .unwrap(),
                copied
            );
            let manifest: Value = studio_json::from_slice(
                &fs::read(target.join(".studio-account-imports.json")).unwrap(),
            )
            .unwrap();
            assert_eq!(
                manifest[format!("sessions/.studio-imports/{name}.jsonl")]["threadId"],
                expected
            );
        }
        let invalid = source.join("sessions/invalid.jsonl");
        fs::write(&invalid, b"{bad}\n").unwrap();
        assert!(matches!(
            copy_native_history(
                source.to_str().unwrap(),
                target.to_str().unwrap(),
                invalid.to_str().unwrap()
            ),
            Err(TransferError::Refused("Native history metadata is invalid"))
        ));
        assert!(
            !target
                .join("sessions/.studio-imports/invalid.jsonl")
                .exists()
        );
    }

    #[test]
    fn native_history_file_digest_is_stable() {
        let root = tempfile::tempdir().unwrap();
        let source = root.path().join("source");
        let target = root.path().join("target");
        fs::create_dir_all(source.join("sessions")).unwrap();
        fs::create_dir_all(&target).unwrap();
        let history = source.join("sessions/golden.jsonl");
        fs::write(&history, b"{\"type\":\"session_meta\",\"payload\":{\"id\":\"source-thread\"}}\n{\"type\":\"turn\"}\n").unwrap();
        copy_native_history(
            source.to_str().unwrap(),
            target.to_str().unwrap(),
            history.to_str().unwrap(),
        )
        .unwrap();
        let manifest: Value = studio_json::from_slice(
            &fs::read(target.join(".studio-account-imports.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(
            manifest["sessions/.studio-imports/golden.jsonl"]["sha256"],
            "62e246317762ee0bc6d9090a820ce23f01c74a1aab542d3fb1034928e31edc4d"
        );
    }

    #[test]
    fn native_history_copy_reuses_the_published_file_after_interruption() {
        let root = tempfile::tempdir().expect("test fixture operation succeeds");
        let source = root.path().join("source");
        let target = root.path().join("target");
        let history = source.join("sessions").join("source-thread.jsonl");
        fs::create_dir_all(history.parent().expect("session directory"))
            .expect("test fixture operation succeeds");
        fs::create_dir_all(&target).expect("test fixture operation succeeds");
        fs::write(
            &history,
            b"{\"type\":\"session_meta\",\"payload\":{\"id\":\"source-thread\"}}\n{\"type\":\"turn\"}\n",
        )
        .expect("test fixture operation succeeds");

        let first = copy_native_history(
            source.to_str().expect("source path"),
            target.to_str().expect("target path"),
            history.to_str().expect("history path"),
        )
        .expect("history copy succeeds");
        // before_start can be re-entered after process restart. The copy step must
        // recognize the exact existing bytes and never publish a duplicate.
        let second = copy_native_history(
            source.to_str().expect("source path"),
            target.to_str().expect("target path"),
            history.to_str().expect("history path"),
        )
        .expect("replayed history copy succeeds");

        assert_eq!(first, second);
        assert_eq!(
            fs::read(&first).expect("copied history is readable"),
            fs::read(&history).expect("source history is readable")
        );
        let manifest: Value = studio_json::from_slice(
            &fs::read(target.join(".studio-account-imports.json")).expect("manifest exists"),
        )
        .expect("manifest is valid");
        assert_eq!(manifest.as_object().map(studio_json::Map::len), Some(1));
        assert_eq!(
            manifest["sessions/.studio-imports/source-thread.jsonl"]["threadId"],
            "source-thread"
        );
    }

    #[test]
    fn concurrent_native_history_imports_preserve_both_manifest_entries() {
        let root = tempfile::tempdir().expect("isolated import fixture");
        let source = root.path().join("source");
        let target = root.path().join("target");
        fs::create_dir_all(source.join("sessions")).expect("source sessions exist");
        fs::create_dir_all(&target).expect("target account exists");
        let first = source.join("sessions/first.jsonl");
        let second = source.join("sessions/second.jsonl");
        fs::write(
            &first,
            b"{\"type\":\"session_meta\",\"payload\":{\"id\":\"first-thread\"}}\n{\"type\":\"turn\",\"id\":\"first\"}\n",
        )
        .expect("first history is written");
        fs::write(
            &second,
            b"{\"type\":\"session_meta\",\"payload\":{\"id\":\"second-thread\"}}\n{\"type\":\"turn\",\"id\":\"second\"}\n",
        )
        .expect("second history is written");

        let barrier = Arc::new(std::sync::Barrier::new(3));
        let run = |history: PathBuf| {
            let barrier = Arc::clone(&barrier);
            let source = source.to_string_lossy().into_owned();
            let target = target.to_string_lossy().into_owned();
            std::thread::spawn(move || {
                barrier.wait();
                copy_native_history(&source, &target, &history.to_string_lossy())
            })
        };
        let first_copy = run(first.clone());
        let second_copy = run(second.clone());
        barrier.wait();
        first_copy
            .join()
            .expect("first import thread joins")
            .expect("first import succeeds");
        second_copy
            .join()
            .expect("second import thread joins")
            .expect("second import succeeds");

        let manifest: Value = studio_json::from_slice(
            &fs::read(target.join(".studio-account-imports.json")).expect("manifest exists"),
        )
        .expect("manifest is valid");
        assert_eq!(manifest.as_object().map(studio_json::Map::len), Some(2));
        assert_eq!(
            fs::read(target.join("sessions/.studio-imports/first.jsonl"))
                .expect("first copy exists"),
            fs::read(first).expect("first source is readable")
        );
        assert_eq!(
            fs::read(target.join("sessions/.studio-imports/second.jsonl"))
                .expect("second copy exists"),
            fs::read(second).expect("second source is readable")
        );
    }

    #[test]
    fn concurrent_conflicting_history_filenames_never_overwrite_the_first_copy() {
        let root = tempfile::tempdir().expect("isolated import fixture");
        let first_source = root.path().join("first-source");
        let second_source = root.path().join("second-source");
        let target = root.path().join("target");
        fs::create_dir_all(first_source.join("sessions")).expect("first source exists");
        fs::create_dir_all(second_source.join("sessions")).expect("second source exists");
        fs::create_dir_all(&target).expect("target account exists");
        let first = first_source.join("sessions/shared.jsonl");
        let second = second_source.join("sessions/shared.jsonl");
        fs::write(
            &first,
            b"{\"type\":\"session_meta\",\"payload\":{\"id\":\"first-thread\"}}\n{\"type\":\"turn\",\"id\":\"first\"}\n",
        )
        .expect("first history is written");
        fs::write(
            &second,
            b"{\"type\":\"session_meta\",\"payload\":{\"id\":\"second-thread\"}}\n{\"type\":\"turn\",\"id\":\"second\"}\n",
        )
        .expect("second history is written");
        let barrier = Arc::new(std::sync::Barrier::new(3));
        let run = |source: PathBuf, history: PathBuf| {
            let barrier = Arc::clone(&barrier);
            let source = source.to_string_lossy().into_owned();
            let target = target.to_string_lossy().into_owned();
            std::thread::spawn(move || {
                barrier.wait();
                copy_native_history(&source, &target, &history.to_string_lossy())
            })
        };
        let first_copy = run(first_source, first.clone());
        let second_copy = run(second_source, second.clone());
        barrier.wait();
        let results = [
            first_copy.join().expect("first import thread joins"),
            second_copy.join().expect("second import thread joins"),
        ];
        assert_eq!(results.iter().filter(|result| result.is_ok()).count(), 1);
        let destination = fs::read(target.join("sessions/.studio-imports/shared.jsonl"))
            .expect("one complete history remains published");
        assert!(
            destination == fs::read(first).expect("first source readable")
                || destination == fs::read(second).expect("second source readable")
        );
        let manifest: Value = studio_json::from_slice(
            &fs::read(target.join(".studio-account-imports.json")).expect("manifest exists"),
        )
        .expect("manifest is valid");
        assert_eq!(manifest.as_object().map(studio_json::Map::len), Some(1));
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod retained_executor_tests {
    use super::*;
    use crate::account_session_runtime::SessionOutcome as ReceiptOutcome;
    use crate::account_session_runtime::{
        AccountSessionError, ConnectionScope, ProviderRequest, SessionReceipt,
    };
    use std::sync::Mutex;
    use studio_json::json;
    use studio_runtime_providers::{RequestId, RequestTicket};
    use studio_runtime_store::{AgentId, AgentRecord, AgentRepository};

    struct GuardSession {
        store: Arc<RuntimeStore>,
        transfer_id: String,
        requests: Mutex<Vec<String>>,
        queue_has_input: bool,
        /// The source thread runs a background command during the move.
        background: bool,
        native_path: Option<String>,
        /// An account with no live session until `ensure_session` starts it.
        dormant: Mutex<Option<String>>,
    }

    impl GuardSession {
        fn scope(account: &str) -> ConnectionScope {
            ConnectionScope {
                account: account.into(),
                connection: format!("{account}-connection"),
                generation: 7,
            }
        }

        fn receipt(
            &self,
            account: &str,
            operation: String,
            method: Method,
            value: Value,
        ) -> SessionReceipt {
            SessionReceipt {
                scope: Self::scope(account),
                operation,
                replayed: false,
                outcome: ReceiptOutcome::Written {
                    ticket: RequestTicket {
                        generation: 7,
                        id: RequestId::Number(1),
                        method,
                    },
                    response: ProviderResponse::Reply(value),
                },
            }
        }
    }

    impl AccountSessionPort for GuardSession {
        fn ensure_session<'a>(
            &'a self,
            account: &'a str,
        ) -> crate::account_session_runtime::AccountSessionFuture<'a, ConnectionScope> {
            self.requests
                .lock()
                .expect("test fixture operation succeeds")
                .push(format!("ensure:{account}"));
            let mut dormant = self
                .dormant
                .lock()
                .expect("test fixture operation succeeds");
            if dormant.as_deref() == Some(account) {
                *dormant = None;
            }
            Box::pin(async move { Ok(Self::scope(account)) })
        }

        fn current_scope(&self, account: &str) -> Option<ConnectionScope> {
            (self
                .dormant
                .lock()
                .expect("test fixture operation succeeds")
                .as_deref()
                != Some(account))
            .then(|| Self::scope(account))
        }

        #[expect(
            clippy::wildcard_enum_match_arm,
            reason = "Decision lives in codex_disposition; this narrow scope handles only its declared methods"
        )]
        fn provider_request<'a>(
            &'a self,
            request: ProviderRequest,
        ) -> crate::account_session_runtime::AccountSessionFuture<'a, SessionReceipt> {
            Box::pin(async move {
                if request.operation.starts_with("model-list:") {
                    assert_eq!(request.account, "target");
                    assert_eq!(request.expected_scope, Some(Self::scope("target")));
                    assert_eq!(request.method, Method::ModelList);
                    assert_eq!(
                        request.params,
                        json!({"limit":studio_runtime_schema::shared_values::MODEL_CATALOG_ITEMS_PER_PAGE,
                            "includeHidden":true})
                    );
                }
                self.requests
                    .lock()
                    .expect("test fixture operation succeeds")
                    .push(request.method.as_str().into());
                let value = match request.method {
                    Method::ModelList => json!({"data":[{"model":"gpt-6-astra","isDefault":true,
                    "supportedReasoningEfforts":[{"reasoningEffort":"medium"}]}]}),
                    Method::ThreadRead => {
                        json!({"thread":{"id":"source-thread","status":{"type":"idle"},
                    "path":self.native_path}})
                    }
                    Method::ThreadTurnsList | Method::ThreadTurnsItemsList => {
                        json!({"data":[],"nextCursor":null})
                    }
                    Method::ThreadFork => {
                        let mut receipt = self.receipt(
                            &request.account,
                            request.operation,
                            request.method,
                            json!({"thread":{"id":"target-thread"}}),
                        );
                        receipt.outcome = ReceiptOutcome::Unknown {
                            ticket: receipt.ticket().expect("written fixture ticket").clone(),
                        };
                        return Ok(receipt);
                    }
                    _ => {
                        return Err(AccountSessionError::Refused("unexpected provider request"));
                    }
                };
                Ok(self.receipt(&request.account, request.operation, request.method, value))
            })
        }

        #[expect(
            clippy::wildcard_enum_match_arm,
            reason = "Decision lives in codex_disposition; this narrow scope handles only its declared methods"
        )]
        fn transfer_provider_request<'a>(
            &'a self,
            request: TransferProviderRequest,
        ) -> crate::account_session_runtime::AccountSessionFuture<'a, SessionReceipt> {
            Box::pin(async move {
                assert_eq!(request.transfer_id, self.transfer_id);
                assert_eq!(request.member, "lead");
                assert_eq!(request.source_account, "source");
                assert_eq!(request.thread_id, "source-thread");
                crate::account_worker::retained::validate(
                    &crate::account_worker::retained::SessionRequest {
                        root_request_id: request.root_request_id.clone(),
                        admitted_request: request.admitted_request.clone(),
                        scope: request.expected_scope.clone(),
                        expected_scope: Some(request.expected_scope.clone()),
                        operation: request.operation.clone(),
                        actor: None,
                        transfer: Some(crate::account_worker::retained::TransferAuthorization {
                            transfer_id: request.transfer_id.clone(),
                            member: request.member.clone(),
                            source_account: request.source_account.clone(),
                            thread_id: request.thread_id.clone(),
                        }),
                        deleted_actor_session: None,
                        method: request.method,
                        params: request.params.clone(),
                        deadline: request.deadline,
                    },
                )
                .map_err(|_| {
                    AccountSessionError::Refused(
                        "The real retained validator refused the transfer preflight",
                    )
                })?;
                assert!(
                    request
                        .operation
                        .starts_with(&format!("transfer:{}:lead:", self.transfer_id))
                );
                let raw: String = self
                    .store
                    .test_reader()
                    .expect("test fixture operation succeeds")
                    .query_row(
                        "SELECT record FROM runtime_account_transfers WHERE id=?1",
                        [&self.transfer_id],
                        |row| row.get(0),
                    )
                    .expect("test fixture operation succeeds");
                let operation: Value =
                    studio_json::from_str(&raw).expect("test fixture operation succeeds");
                assert!(
                    operation["members"]["lead"]["providerReadIntents"]
                        .get(&request.operation)
                        .is_some()
                );
                self.requests
                    .lock()
                    .expect("test fixture operation succeeds")
                    .push(request.method.as_str().into());
                let value = match request.method {
                    Method::ThreadQueueList => {
                        json!({"data":if self.queue_has_input {json!([{"id":"queued-input"}])} else {json!([])},"nextCursor":null})
                    }
                    Method::ThreadBackgroundTerminalsList => {
                        json!({"data":if self.background {json!([{"id":"bg-1","status":"running"}])} else {json!([])},"nextCursor":null})
                    }
                    Method::ThreadUnsubscribe => json!({}),
                    _ => {
                        return Err(AccountSessionError::Refused(
                            "unexpected source transfer request",
                        ));
                    }
                };
                Ok(self.receipt(
                    &request.source_account,
                    request.operation,
                    request.method,
                    value,
                ))
            })
        }
    }

    /// A lead that moved to this server while its subagent transfer was pending
    /// on the source server (igor-mbp, 2026-10-18): the record has the summary,
    /// this store has no transfer row.
    async fn store_with_orphan_summary(root: &Path) -> Arc<RuntimeStore> {
        let store = Arc::new(
            RuntimeStore::open(Some(root))
                .await
                .expect("test fixture operation succeeds"),
        );
        let lead = AgentRecord::from_value(
            json!({"compactions":0,"id":"lead","rootId":"lead","epoch":0,
            "status":"queued","isLead":true,"autoWake":true,"accountKey":"source",
            "provider":"codex","model":"gpt-6-astra","cwd":"/tmp/work","threadId":"source-thread",
            "accountTransfer":{"id":"abde1958-090d-4a79-8516-3780b6469e81","status":"pending",
                "scope":"subagents","targetAccountKey":"target","updated":1791278955.704289,
                "finishHistory":true,"total":3,"completed":2,"moved":1,"movingNow":0,
                "nativeHistoryPending":1,"canFinishHistory":false,"interrupted":[],
                "leftOnSource":[],"blocked":[],"waitingCount":0,"needsAttention":true,
                "canRetry":true,"waiting":"Waiting for background work or a tool response"}}),
        )
        .expect("test fixture operation succeeds");
        store
            .transaction_with_error(move |tx| {
                AgentRepository::put(tx, &lead)?;
                Ok::<_, studio_runtime_store::StoreError>(())
            })
            .await
            .expect("test fixture operation succeeds");
        store
    }

    fn stored_lead(store: &RuntimeStore) -> Value {
        AgentRepository::get(
            &store
                .test_reader()
                .expect("test fixture operation succeeds"),
            &AgentId::from("lead"),
        )
        .expect("test fixture operation succeeds")
        .expect("test fixture operation succeeds")
        .value()
    }

    #[tokio::test]
    async fn summary_without_its_transfer_row_does_not_hold_turn_start() {
        let root = tempfile::tempdir().expect("test fixture operation succeeds");
        let store = store_with_orphan_summary(root.path()).await;
        let catalog = ModelCatalogService::new();
        let resources = crate::account_worker::preparation::PreparationResources {
            instructions: String::new(),
            thread_config: Default::default(),
            tools: vec![],
            legacy_tools: vec![],
            skills_root: PathBuf::from("/tmp/skills"),
            role_guidance: Default::default(),
            model_catalog: catalog.clone(),
            account_home: "/tmp/source".into(),
            shared_home: String::new(),
            filesystem_inputs: None,
            claude_account_pin: studio_json::Value::Null,
            account_driver: None,
            account_session: None,
        };

        let agent = before_start(Arc::clone(&store), None, &catalog, &resources, "lead", 2.0)
            .await
            .expect("an orphan summary is not a receipt hold");

        assert!(agent.get("accountTransfer").is_none());
        assert!(stored_lead(&store).get("accountTransfer").is_none());
    }

    #[tokio::test]
    async fn summary_without_its_transfer_row_does_not_block_a_new_transfer() {
        let root = tempfile::tempdir().expect("test fixture operation succeeds");
        let store = store_with_orphan_summary(root.path()).await;
        let transfer_id = "5a300f72-4ea6-4d30-8a0e-9b2c13c42281";

        crate::account_transfer_driver::request(
            &store,
            transfer_id,
            "lead",
            "target",
            "team",
            "codex",
            3.0,
        )
        .await
        .expect("a new transfer replaces an orphan summary");

        assert_eq!(stored_lead(&store)["accountTransfer"]["id"], transfer_id);
    }

    /// The source account of a moving chat often has no live worker: no other
    /// agent uses it. The source reads start its session first.
    #[tokio::test]
    async fn source_reads_start_a_dormant_source_session() {
        assert_dormant_source_reads(false).await;
    }

    #[tokio::test]
    async fn startup_transfer_reads_registered_profiles_before_any_http_catalog_request() {
        assert_dormant_source_reads(true).await;
    }

    async fn assert_dormant_source_reads(register_from_driver: bool) {
        let root = tempfile::tempdir().expect("test fixture operation succeeds");
        let store = Arc::new(
            RuntimeStore::open(Some(root.path()))
                .await
                .expect("test fixture operation succeeds"),
        );
        let lead = AgentRecord::from_value(
            json!({"compactions":0,"id":"lead","rootId":"lead","epoch":1,
            "status":"idle","isLead":true,"accountKey":"source","provider":"codex",
            "model":"gpt-6-astra","cwd":"/tmp/work","threadId":"source-thread"}),
        )
        .expect("test fixture operation succeeds");
        store
            .transaction_with_error(move |tx| {
                AgentRepository::put(tx, &lead)?;
                Ok::<_, studio_runtime_store::StoreError>(())
            })
            .await
            .expect("test fixture operation succeeds");
        let transfer_id = "5a300f72-4ea6-4d30-8a0e-9b2c13c4a201";
        crate::account_transfer_driver::request(
            &store,
            transfer_id,
            "lead",
            "target",
            "team",
            "codex",
            1.0,
        )
        .await
        .expect("test fixture operation succeeds");
        let session = Arc::new(GuardSession {
            store: store.clone(),
            transfer_id: transfer_id.into(),
            requests: Mutex::new(Vec::new()),
            queue_has_input: true,
            background: false,
            native_path: None,
            dormant: Mutex::new(Some("source".into())),
        });
        let catalog = ModelCatalogService::new();
        let account_driver = if register_from_driver {
            let registry_path = root.path().join("fixture-accounts.json");
            let registry = json!({"version":studio_runtime_schema::shared_values::ACCOUNT_REGISTRY_FORMAT_VERSION,"defaultAccountKey":"source","accounts":{
                "source":{"id":"source","home":root.path().join("source-home"),
                    "provider":"codex","label":"Source","source":"Codex CLI","status":"signedOut"},
                "target":{"id":"target","home":root.path().join("target-home"),
                    "provider":"codex","label":"Target","source":"Codex CLI","status":"signedOut"}
                },"logins":{},"deleteReceipts":{}});
            fs::write(
                &registry_path,
                studio_json::to_vec(&registry).expect("fixture registry"),
            )
            .expect("save fixture account metadata");
            let driver = crate::account_driver::AccountDriver::open(&registry_path)
                .expect("open fixture account metadata without authentication reads");
            assert!(catalog.account_profile("target").is_none());
            Some(Arc::new(driver))
        } else {
            catalog.retain_accounts(&[
                json!({"id":"source","home":"/tmp/source","provider":"codex"}),
                json!({"id":"target","home":"/tmp/target","provider":"codex"}),
            ]);
            None
        };
        let resources = crate::account_worker::preparation::PreparationResources {
            instructions: String::new(),
            thread_config: Default::default(),
            tools: vec![],
            legacy_tools: vec![],
            skills_root: PathBuf::from("/tmp/skills"),
            role_guidance: Default::default(),
            model_catalog: catalog.clone(),
            account_home: "/tmp/target".into(),
            shared_home: String::new(),
            filesystem_inputs: None,
            claude_account_pin: studio_json::Value::Null,
            account_driver,
            account_session: Some(session.clone()),
        };

        let result = before_start(
            Arc::clone(&store),
            Some(session.clone()),
            &catalog,
            &resources,
            "lead",
            2.0,
        )
        .await;

        // The queued source input refuses the move only after the source reads ran.
        assert!(
            matches!(
                &result,
                Err(TransferError::Refused(
                    "Source native thread has queued input; saved input remains queued"
                ))
            ),
            "transfer preparation must reach source queue admission: {result:?}"
        );
        if register_from_driver {
            assert_eq!(
                catalog.account_profile("target"),
                Some((
                    root.path()
                        .join("target-home")
                        .to_string_lossy()
                        .into_owned(),
                    ProviderKind::Codex
                ))
            );
        }
        assert_eq!(
            *session
                .requests
                .lock()
                .expect("test fixture operation succeeds"),
            [
                "model/list",
                "ensure:source",
                "thread/read",
                "thread/queue/list"
            ]
        );
    }

    #[tokio::test]
    async fn before_start_executor_checks_and_durably_authorizes_source_queue() {
        let root = tempfile::tempdir().expect("test fixture operation succeeds");
        let store = Arc::new(
            RuntimeStore::open(Some(root.path()))
                .await
                .expect("test fixture operation succeeds"),
        );
        let lead = AgentRecord::from_value(
            json!({"compactions":0,"id":"lead","rootId":"lead","epoch":1,
            "status":"idle","isLead":true,"accountKey":"source","provider":"codex",
            "model":"gpt-6-astra","cwd":"/tmp/work","threadId":"source-thread"}),
        )
        .expect("test fixture operation succeeds");
        store
            .transaction_with_error(move |tx| {
                AgentRepository::put(tx, &lead)?;
                Ok::<_, studio_runtime_store::StoreError>(())
            })
            .await
            .expect("test fixture operation succeeds");
        let transfer_id = "5a300f72-4ea6-4d30-8a0e-9b2c13c42281";
        crate::account_transfer_driver::request(
            &store,
            transfer_id,
            "lead",
            "target",
            "team",
            "codex",
            1.0,
        )
        .await
        .expect("test fixture operation succeeds");
        let session = Arc::new(GuardSession {
            store: store.clone(),
            transfer_id: transfer_id.into(),
            requests: Mutex::new(Vec::new()),
            queue_has_input: true,
            background: false,
            native_path: None,
            dormant: Mutex::new(None),
        });
        let catalog = ModelCatalogService::new();
        catalog.retain_accounts(&[
            json!({"id":"source","home":"/tmp/source","provider":"codex"}),
            json!({"id":"target","home":"/tmp/target","provider":"codex"}),
        ]);
        let resources = crate::account_worker::preparation::PreparationResources {
            instructions: String::new(),
            thread_config: Default::default(),
            tools: vec![],
            legacy_tools: vec![],
            skills_root: PathBuf::from("/tmp/skills"),
            role_guidance: Default::default(),
            model_catalog: catalog.clone(),
            account_home: "/tmp/target".into(),
            shared_home: String::new(),
            filesystem_inputs: None,
            claude_account_pin: studio_json::Value::Null,
            account_driver: None,
            account_session: Some(session.clone()),
        };

        let result = before_start(
            Arc::clone(&store),
            Some(session.clone()),
            &catalog,
            &resources,
            "lead",
            2.0,
        )
        .await;

        assert!(matches!(
            result,
            Err(TransferError::Refused(
                "Source native thread has queued input; saved input remains queued"
            ))
        ));
        assert_eq!(
            *session
                .requests
                .lock()
                .expect("test fixture operation succeeds"),
            ["model/list", "thread/read", "thread/queue/list"]
        );
        let operation: String = store
            .test_reader()
            .expect("test fixture operation succeeds")
            .query_row(
                "SELECT record FROM runtime_account_transfers WHERE id=?1",
                [transfer_id],
                |row| row.get(0),
            )
            .expect("test fixture operation succeeds");
        let operation: Value =
            studio_json::from_str(&operation).expect("test fixture operation succeeds");
        let intent = operation["members"]["lead"]["providerReadIntents"]
            .as_object()
            .expect("test fixture operation succeeds");
        assert_eq!(intent.len(), 3);
        assert!(
            AgentRepository::get(
                &store
                    .test_reader()
                    .expect("test fixture operation succeeds"),
                &AgentId::from("lead")
            )
            .expect("test fixture operation succeeds")
            .is_some()
        );
    }

    #[tokio::test]
    async fn before_start_copies_history_once_and_replays_unknown_fork_after_restart() {
        Box::pin(assert_history_transfer_without_duplicate_fork(false)).await;
    }

    #[tokio::test]
    async fn a_background_command_does_not_block_a_transfer() {
        Box::pin(assert_history_transfer(false, true)).await;
    }

    #[tokio::test]
    async fn cold_catalog_transfer_prepares_history_without_http_model_read() {
        Box::pin(assert_history_transfer_without_duplicate_fork(true)).await;
    }

    async fn assert_history_transfer_without_duplicate_fork(cold_catalog: bool) {
        Box::pin(assert_history_transfer(cold_catalog, false)).await;
    }

    async fn assert_history_transfer(cold_catalog: bool, background: bool) {
        let root = tempfile::tempdir().expect("test fixture operation succeeds");
        let fixture = root.path().join("fixture");
        let state_dir = root.path().join("state");
        let source_home = fixture.join("source");
        let target_home = fixture.join("target");
        let cwd = fixture.join("project");
        let native_path = source_home.join("sessions/source-thread.jsonl");
        fs::create_dir_all(native_path.parent().expect("source sessions directory"))
            .expect("test fixture operation succeeds");
        fs::create_dir_all(&target_home).expect("test fixture operation succeeds");
        fs::create_dir_all(&cwd).expect("test fixture operation succeeds");
        fs::create_dir_all(fixture.join("skills")).expect("test fixture operation succeeds");
        fs::create_dir_all(fixture.join("server")).expect("test fixture operation succeeds");
        fs::create_dir_all(&state_dir).expect("test fixture operation succeeds");
        fs::write(
            &native_path,
            b"{\"type\":\"session_meta\",\"payload\":{\"id\":\"source-thread\"}}\n{\"type\":\"turn\"}\n",
        )
        .expect("test fixture operation succeeds");

        let store = Arc::new(
            RuntimeStore::open(Some(&state_dir))
                .await
                .expect("test fixture operation succeeds"),
        );
        let lead = AgentRecord::from_value(
            json!({"compactions":0,"id":"lead","rootId":"lead","epoch":1,
            "status":"idle","isLead":true,"accountKey":"source","provider":"codex",
            "model":"gpt-6-astra","cwd":cwd,"threadId":"source-thread"}),
        )
        .expect("test fixture operation succeeds");
        store
            .transaction_with_error(move |tx| {
                AgentRepository::put(tx, &lead)?;
                Ok::<_, studio_runtime_store::StoreError>(())
            })
            .await
            .expect("test fixture operation succeeds");
        let transfer_id = "5a300f72-4ea6-4d30-8a0e-9b2c13c42285";
        crate::account_transfer_driver::request(
            &store,
            transfer_id,
            "lead",
            "target",
            "team",
            "codex",
            1.0,
        )
        .await
        .expect("durable transfer request succeeds");

        let catalog = ModelCatalogService::new();
        catalog.retain_accounts(&[
            json!({"id":"source","home":source_home,"provider":"codex"}),
            json!({"id":"target","home":target_home,"provider":"codex"}),
        ]);
        if !cold_catalog {
            catalog.seed_test_admission_catalog(
                "target",
                target_home.to_str().expect("target home"),
                ProviderKind::Codex,
                json!({"data":[{"model":"gpt-6-astra","isDefault":true,
                "supportedReasoningEfforts":[{"reasoningEffort":"medium"}]}],"nextCursor":null}),
            );
        }
        let session = Arc::new(GuardSession {
            store: store.clone(),
            transfer_id: transfer_id.into(),
            requests: Mutex::new(Vec::new()),
            queue_has_input: false,
            background,
            native_path: Some(native_path.to_string_lossy().into_owned()),
            dormant: Mutex::new(None),
        });
        catalog.set_account_session(Some(session.clone()));
        let resources = crate::account_worker::preparation::PreparationResources {
            instructions: String::new(),
            thread_config: Default::default(),
            tools: vec![],
            legacy_tools: vec![],
            skills_root: fixture.join("skills"),
            role_guidance: [(
                PathBuf::from("codex-orchestrator/references/legacy-move-role.md"),
                "Keep the source context available during the transfer.".into(),
            )]
            .into(),
            model_catalog: catalog.clone(),
            account_home: target_home.to_string_lossy().into_owned(),
            shared_home: String::new(),
            filesystem_inputs: None,
            claude_account_pin: Value::Null,
            account_driver: None,
            account_session: Some(session.clone()),
        };
        if !cold_catalog {
            crate::account_transfer_driver::begin_read(&store, transfer_id, "lead", 1.5)
                .await
                .expect("the durable member enters Reading before preparation");
            let attempt_id = format!("account-transfer:{transfer_id}:lead");
            let generic = crate::account_worker::preparation::prepare_native_thread(
                Arc::clone(&store),
                &resources,
                "lead",
                &attempt_id,
                7,
            )
            .await
            .expect_err("ordinary preparation keeps the transfer admission hold");
            assert_eq!(generic.to_string(), "thread-preparation/AccountTransfer");
            assert!(
                crate::account_worker::preparation::prepare_account_transfer_native_thread(
                    Arc::clone(&store),
                    &resources,
                    "lead",
                    "a different transfer id",
                    &attempt_id,
                    7,
                )
                .await
                .is_err(),
                "a different owner cannot bypass the transfer hold"
            );
            let owned = crate::account_worker::preparation::prepare_account_transfer_native_thread(
                Arc::clone(&store),
                &resources,
                "lead",
                transfer_id,
                &attempt_id,
                7,
            )
            .await
            .expect("the exact Reading transfer owner may prepare its destination");
            assert!(owned.params.is_object());

            store
                .transaction_with_error(|tx| {
                    let mut row = AgentRepository::get(tx, &AgentId::from("lead"))?.unwrap();
                    row.context_repair =
                        studio_runtime_schema::queue_command::Field::Value(Box::new(
                            studio_runtime_store::ContextRepair::test_fixture("preparing"),
                        ));
                    AgentRepository::put(tx, &row)
                })
                .await
                .expect("context repair blocker is installed");
            let context_repair =
                crate::account_worker::preparation::prepare_account_transfer_native_thread(
                    Arc::clone(&store),
                    &resources,
                    "lead",
                    transfer_id,
                    &attempt_id,
                    7,
                )
                .await
                .expect_err("the transfer owner cannot bypass context repair");
            assert_eq!(context_repair.to_string(), "preparation/ownership-changed");
            store
                .transaction_with_error(|tx| {
                    let mut row = AgentRepository::get(tx, &AgentId::from("lead"))?.unwrap();
                    row.context_repair = studio_runtime_schema::queue_command::Field::Missing;
                    AgentRepository::put(tx, &row)
                })
                .await
                .expect("context repair blocker is removed");

            store
                .transaction_with_error(|tx| {
                    let mut row = AgentRepository::get(tx, &AgentId::from("lead"))?.unwrap();
                    row.native_release = Some(studio_runtime_store::NativeRelease {
                        admitted_request: None,
                        scope: None,
                        extra: [
                            (
                                "phase".into(),
                                studio_json::to_raw_value(&"unknown").unwrap(),
                            ),
                            (
                                "resetPending".into(),
                                studio_json::to_raw_value(&true).unwrap(),
                            ),
                        ]
                        .into(),
                    });
                    AgentRepository::put(tx, &row)
                })
                .await
                .expect("native release blocker is installed");
            let native_release =
                crate::account_worker::preparation::prepare_account_transfer_native_thread(
                    Arc::clone(&store),
                    &resources,
                    "lead",
                    transfer_id,
                    &attempt_id,
                    7,
                )
                .await
                .expect_err("the transfer owner cannot bypass native release");
            assert_eq!(
                native_release.to_string(),
                "thread-preparation/NativeRelease"
            );
            store
                .transaction_with_error(|tx| {
                    let mut row = AgentRepository::get(tx, &AgentId::from("lead"))?.unwrap();
                    row.native_release = None;
                    AgentRepository::put(tx, &row)
                })
                .await
                .expect("native release blocker is removed");
            crate::account_transfer_driver::recover(&store, 1.75)
                .await
                .expect("startup recovery returns the member to its retryable phase");
        }
        if cold_catalog {
            assert!(matches!(
                catalog.read_for_admission("target", &resources.account_home, ProviderKind::Codex),
                crate::model_catalog::AdmissionCatalogRead::Pending
            ));
        }
        let first = before_start(
            Arc::clone(&store),
            Some(session.clone()),
            &catalog,
            &resources,
            "lead",
            2.0,
        )
        .await
        .expect("worker before-start prepares the transfer");
        let detached = session
            .requests
            .lock()
            .expect("the provider request log is available")
            .iter()
            .any(|method| method == "thread/unsubscribe");
        // A background command keeps running on the source: its thread stays attached.
        assert_eq!(detached, !background);
        if cold_catalog {
            assert_eq!(
                session
                    .requests
                    .lock()
                    .expect("the provider request log is available")
                    .iter()
                    .filter(|method| method.as_str() == "model/list")
                    .count(),
                2,
                "the transfer read and the admission owner's refresh are distinct"
            );
            match catalog.read_for_admission("target", &resources.account_home, ProviderKind::Codex)
            {
                crate::model_catalog::AdmissionCatalogRead::Fresh {
                    account_key,
                    home,
                    provider,
                    generation,
                    ..
                } => {
                    assert_eq!(account_key, "target");
                    assert_eq!(home, target_home.to_string_lossy());
                    assert_eq!(provider, ProviderKind::Codex);
                    assert!(generation > 0);
                }
                other @ crate::model_catalog::AdmissionCatalogRead::Pending
                | other @ crate::model_catalog::AdmissionCatalogRead::Unavailable => {
                    panic!("the exact destination admission cache must be fresh: {other:?}")
                }
            }
        }
        let imported = target_home.join("sessions/.studio-imports/source-thread.jsonl");
        assert!(imported.is_file());
        #[cfg(unix)]
        let imported_inode = {
            use std::os::unix::fs::MetadataExt;
            fs::metadata(&imported)
                .expect("imported history metadata exists")
                .ino()
        };
        assert_eq!(first["accountKey"], "target");
        assert_eq!(
            session
                .requests
                .lock()
                .expect("test fixture operation succeeds")
                .iter()
                .filter(|method| method.as_str() == "thread/fork")
                .count(),
            1
        );
        let saved: String = store
            .test_reader()
            .expect("test fixture operation succeeds")
            .query_row(
                "SELECT record FROM runtime_account_transfers WHERE id=?1",
                [transfer_id],
                |row| row.get(0),
            )
            .expect("transfer outcome is durable");
        let saved: Value = studio_json::from_str(&saved).expect("saved transfer is valid");
        assert_eq!(saved["members"]["lead"]["phase"], "unknown");
        assert_eq!(
            saved["members"]["lead"]["providerReceipts"][0]["certainty"],
            "unknown"
        );
        drop(resources);
        drop(session);
        drop(catalog);
        drop(store);

        let recovered_store = Arc::new(
            RuntimeStore::open(Some(&state_dir))
                .await
                .expect("store reopens after worker restart"),
        );
        let recovered_catalog = ModelCatalogService::new();
        recovered_catalog.retain_accounts(&[
            json!({"id":"source","home":source_home,"provider":"codex"}),
            json!({"id":"target","home":target_home,"provider":"codex"}),
        ]);
        if !cold_catalog {
            recovered_catalog.seed_test_admission_catalog(
                "target",
                target_home.to_str().expect("target home"),
                ProviderKind::Codex,
                json!({"data":[{"model":"gpt-6-astra","isDefault":true,
                "supportedReasoningEfforts":[{"reasoningEffort":"medium"}]}],"nextCursor":null}),
            );
        }
        let recovered_session = Arc::new(GuardSession {
            store: recovered_store.clone(),
            transfer_id: transfer_id.into(),
            requests: Mutex::new(Vec::new()),
            queue_has_input: false,
            background: false,
            native_path: Some(native_path.to_string_lossy().into_owned()),
            dormant: Mutex::new(None),
        });
        let recovered_resources = crate::account_worker::preparation::PreparationResources {
            instructions: String::new(),
            thread_config: Default::default(),
            tools: vec![],
            legacy_tools: vec![],
            skills_root: fixture.join("skills"),
            role_guidance: [(
                PathBuf::from("codex-orchestrator/references/legacy-move-role.md"),
                "Keep the source context available during the transfer.".into(),
            )]
            .into(),
            model_catalog: recovered_catalog.clone(),
            account_home: target_home.to_string_lossy().into_owned(),
            shared_home: String::new(),
            filesystem_inputs: None,
            claude_account_pin: Value::Null,
            account_driver: None,
            account_session: Some(recovered_session.clone()),
        };
        assert!(
            before_start(
                Arc::clone(&recovered_store),
                Some(recovered_session.clone()),
                &recovered_catalog,
                &recovered_resources,
                "lead",
                3.0,
            )
            .await
            .is_err(),
            "unknown provider outcome must refuse retry after restart"
        );
        if cold_catalog {
            assert!(
                recovered_session
                    .requests
                    .lock()
                    .expect("the provider request log is available")
                    .is_empty(),
                "a cold-cache restart must not refresh or resend an unknown transfer"
            );
        }
        assert_eq!(
            recovered_session
                .requests
                .lock()
                .expect("test fixture operation succeeds")
                .iter()
                .filter(|method| method.as_str() == "thread/fork")
                .count(),
            0,
            "an unknown provider outcome is never sent again after restart"
        );
        assert_eq!(
            fs::read_dir(target_home.join("sessions/.studio-imports"))
                .expect("history import directory exists")
                .count(),
            1
        );
        assert_eq!(
            fs::read(imported).expect("imported history remains readable"),
            fs::read(native_path).expect("source history remains readable")
        );
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            assert_eq!(
                fs::metadata(target_home.join("sessions/.studio-imports/source-thread.jsonl"))
                    .expect("imported history metadata exists")
                    .ino(),
                imported_inode,
                "the interrupted copy was not replaced after restart"
            );
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod claude_move_import_tests {
    use super::*;
    use studio_json::json;

    fn export() -> Value {
        json!({
            "path": "/source/projects/-proj/n1.jsonl",
            "session": {"id": "t1", "nativeId": "n1", "model": "default", "turns": [],
                "moveProof": {"snapshotHash": "h"}}
        })
    }

    #[test]
    fn claude_import_keeps_the_exported_session_and_path() {
        let params =
            claude_import_params(&export(), &json!("/target/proj"), &json!("default")).unwrap();
        assert_eq!(params["path"], "/source/projects/-proj/n1.jsonl");
        assert_eq!(params["cwd"], "/target/proj");
        assert_eq!(params["session"]["id"], "t1");
        assert_eq!(params["session"]["nativeId"], "n1");
        assert_eq!(params["session"]["moveProof"]["snapshotHash"], "h");
        assert!(params.get("portableHistory").is_none());
        assert!(params.get("developerInstructions").is_none());
    }

    #[test]
    fn claude_import_refuses_a_missing_path_or_another_model() {
        let mut missing = export();
        missing.as_object_mut().unwrap().remove("path");
        assert!(claude_import_params(&missing, &json!("/t"), &json!("default")).is_err());
        assert!(claude_import_params(&export(), &json!("/t"), &json!("other-model")).is_err());
    }

    #[test]
    fn claude_export_gets_a_durable_request_identity() {
        // The worker stores a move export, so an empty root identity is refused.
        use crate::account_transfer_driver::source_call_needs_admission;
        assert!(source_call_needs_admission("claude/moveExport"));
        assert!(source_call_needs_admission("thread/unsubscribe"));
        assert!(!source_call_needs_admission("thread/queue/list"));
    }

    #[test]
    fn claude_export_is_a_source_read_only_for_its_thread() {
        let params = json!({"threadId": "t1"});
        assert!(crate::account_transfer::valid_source_preflight_params(
            "claude/moveExport",
            "t1",
            &params
        ));
        assert!(!crate::account_transfer::valid_source_preflight_params(
            "claude/moveExport",
            "t2",
            &params
        ));
    }
}
