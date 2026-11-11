//! Binary composition of retained account sessions. The supervisor is an
//! independently installed service; backend shutdown never takes its children.
use crate::account_registry::{
    AccountLaunch, AccountLauncher, AccountRegistry, AccountRegistryConfig, AccountSessionBroker,
    LaunchFuture,
};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::Duration,
};
use studio_runtime_http::HttpState;
use studio_runtime_process::{
    BackendId, BackendIdentity, Generation, OperationId, ProviderHandle,
    child_priority::ChildPriority,
    provider_direct::DirectProviderTaskFactory,
    provider_seam::{OpenOptions, ProviderTaskFactory, SessionClock, Shutdown},
    provider_task::SupervisorProviderTaskFactory,
};
use studio_runtime_services::{
    account_driver::{AccountDriver, AccountStatus, ProviderAuthReader},
    account_worker::{WorkerError, preparation::PreparationResources},
    native_runtime::{NativeExecutableLease, NativeRuntime},
    provider_adapter::{HostLaunch, ProviderKind, ProviderLaunch, adapter},
    provider_session_task::SessionTaskConfig,
    thread_prepare::{CatalogSnapshot, CatalogSource},
};
use studio_runtime_store::RuntimeStore;
use tokio::task::JoinHandle;
use tokio::time::timeout;

const REQUEST_TIMEOUT_MS: u64 = 60_000;
const HOST_RUNTIME_SHUTDOWN_WAIT: Duration = Duration::from_secs(5);
// The supervisor detach must finish, and report, inside this bound.
const _: () = assert!(
    studio_runtime_process::provider_task::SUPERVISOR_DETACH_TIMEOUT.as_millis()
        < HOST_RUNTIME_SHUTDOWN_WAIT.as_millis()
);
const CLAUDE_BRIDGE_RELATIVE: &str = "workspaces/providers/apps/claude-bridge";
/// Installed root shared by host-side runtime components.
#[derive(Clone, Debug)]
pub struct InstallRoot(PathBuf);

impl InstallRoot {
    pub fn workspace_root(&self) -> &Path {
        &self.0
    }
    pub fn claude_bridge_dir(&self) -> PathBuf {
        let packaged = self.0.join("scripts/claude_bridge");
        if packaged.join("bridge.mjs").is_file() {
            packaged
        } else {
            self.0.join(CLAUDE_BRIDGE_RELATIVE)
        }
    }
}

/// Resolve the package root from the executable, or the source build in debug mode.
pub fn resolve_install_root() -> Result<InstallRoot, WorkerError> {
    resolve_install_root_from_executable(std::env::current_exe().ok().as_deref())
}

fn resolve_install_root_from_executable(
    executable: Option<&Path>,
) -> Result<InstallRoot, WorkerError> {
    let located = || {
        WorkerError::PreparationRefused(
            "runtime-server/install-root: expected the Claude bridge directory (scripts/claude_bridge when installed, workspaces/providers/apps/claude-bridge in a source checkout) beside the executable or under the source checkout".into(),
        )
    };
    let is_root = |path: &Path| {
        let root = InstallRoot(path.to_path_buf());
        root.claude_bridge_dir().join("bridge.mjs").is_file()
    };

    if let Some(executable) = executable {
        for ancestor in executable.ancestors() {
            let candidate = if ancestor.file_name().is_some_and(|name| name == "bin") {
                ancestor.parent().unwrap_or(ancestor)
            } else {
                ancestor
            };
            if is_root(candidate) {
                return Ok(InstallRoot(candidate.to_path_buf()));
            }
        }
    }

    #[cfg(debug_assertions)]
    if let Some(manifest) = std::env::var_os("CARGO_MANIFEST_DIR") {
        let source_root = PathBuf::from(manifest).join("../../../..");
        let source_root =
            studio_runtime_store::path_identity::CanonicalPath::existing(&source_root)
                .map_err(|_| located())?;
        if is_root(&source_root) {
            return Ok(InstallRoot(source_root.to_path_buf()));
        }
    }
    Err(located())
}

pub struct HostRuntime {
    shutdown: Shutdown,
    task: JoinHandle<Result<(), WorkerError>>,
    preparation_resources: Option<Arc<PreparationResources>>,
}

struct RegistryRuntimeOptions {
    continuation_wake: Arc<tokio::sync::Notify>,
    monitor_dispatch_wake: Arc<tokio::sync::Notify>,
    session_broker: AccountSessionBroker,
    preparation_resources: Option<Arc<PreparationResources>>,
    config: AccountRegistryConfig,
    managed_tool_runtime:
        Option<Arc<studio_runtime_services::provider_callbacks::ManagedToolRuntime>>,
}

impl HostRuntime {
    #[allow(clippy::too_many_arguments)]
    pub async fn start_with_monitor_wake_and_native_runtime(
        state: &mut HttpState,
        store: Arc<RuntimeStore>,
        accounts: Arc<AccountDriver>,
        native_path: Option<String>,
        monitor_dispatch_wake: Arc<tokio::sync::Notify>,
        session_broker: AccountSessionBroker,
        native_runtime: Option<Arc<NativeRuntime>>,
        session_binaries: Arc<Mutex<BTreeMap<String, crate::provider_ports::SessionExecutables>>>,
        config: AccountRegistryConfig,
        managed_tool_runtime: Option<
            Arc<studio_runtime_services::provider_callbacks::ManagedToolRuntime>,
        >,
        // The base Codex home (`resolve_codex_home`); `None` gives host Codex
        // threads no native Chrome browser.
        shared_codex_home: Option<PathBuf>,
    ) -> Result<Self, WorkerError> {
        let mut nonce = [0_u8; 16];
        getrandom::fill(&mut nonce).map_err(|_| WorkerError::TaskFailed)?;
        let backend_id = data_encoding::HEXLOWER.encode(&nonce);
        let identity = BackendIdentity {
            backend_id: BackendId(backend_id.clone()),
            state_dir: store.root().to_owned(),
        };
        let install_root = resolve_install_root()?;
        let catalog = state.model_catalog_handle();
        let resource_root = install_root.workspace_root().to_owned();
        let mut resources = studio_runtime_process::blocking_pool::spawn_blocking(
            "runtime-server/host_runtime",
            move || PreparationResources::bundled_at(String::new(), catalog, resource_root),
        )
        .await
        .map_err(|_| WorkerError::TaskFailed)??;
        let claude_bridge_dir = install_root.claude_bridge_dir();
        resources.account_driver = Some(accounts.clone());
        if let Some(home) = shared_codex_home {
            resources.shared_home = home.to_string_lossy().into_owned();
        }
        // Limits of an idle account are read only after its turns. Read them
        // again once a window resets after the last read.
        if let Some(session) = state.account_session.clone() {
            let store = store.clone();
            tokio::spawn(async move {
                use studio_runtime_services::account_session_runtime::{
                    LIMITS_RESET_CHECK_SECONDS, limits_reset_passed,
                };
                let period = std::time::Duration::from_secs(LIMITS_RESET_CHECK_SECONDS);
                let mut ticks =
                    tokio::time::interval_at(tokio::time::Instant::now() + period, period);
                ticks.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
                loop {
                    ticks.tick().await;
                    let accounts = store
                        .read_async("limits/reset-check", |tx| {
                            let mut rows = tx.prepare(
                                "SELECT id FROM sync_entities WHERE collection='rateLimit' AND deleted=0",
                            )?;
                            let ids = rows
                                .query_map([], |row| row.get::<_, String>(0))?
                                .collect::<rusqlite::Result<Vec<_>>>()?;
                            Ok::<_, studio_runtime_store::StoreError>(ids)
                        })
                        .await
                        .unwrap_or_default();
                    let now = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .map_or(0.0, |elapsed| elapsed.as_secs_f64());
                    for account in accounts {
                        if session
                            .limits_snapshot(&account)
                            .is_ok_and(|snapshot| limits_reset_passed(&snapshot, now))
                            && let Err(error) = session.refresh_limits(&account).await
                        {
                            eprintln!("limits refresh after reset for {account} failed: {error}");
                        }
                    }
                }
            });
        }
        let resources = resources.with_account_session(state.account_session.clone());
        let preparation_resources = Arc::new(resources.clone());
        if let Some(managed_tool_runtime) = &managed_tool_runtime {
            managed_tool_runtime.set_preparation_resources(Arc::clone(&preparation_resources));
        }
        let callback_runtime = Arc::new(
            studio_runtime_services::callback_runtime::CallbackRuntime::new(
                store.clone(),
                session_broker.callback_reply_port(),
            ),
        );
        *state = install_host_voice_port(state.clone(), store.clone(), callback_runtime.clone())
            .with_tool_catalog(CatalogSnapshot {
                source: CatalogSource::Current,
                definitions: resources.tools.clone(),
                saved_thread_id: None,
                saved_digest: None,
            })
            .with_callback_runtime(callback_runtime);
        let launcher = Arc::new(HostLauncher {
            factory: SupervisorProviderTaskFactory::new(identity).with_account_store(store.clone()),
            supervisor_mode: std::env::var("CODEX_AGENTS_SUPERVISOR_MODE")
                .is_ok_and(|value| value == "1"),
            child_priority: ChildPriority::from_environment(),
            provider_stderr_log: state.app_server_log_handle(),
            accounts,
            store: store.clone(),
            root: store.root().to_owned(),
            native_path,
            native_runtime,
            session_binaries,
            resources: Arc::clone(&preparation_resources),
            claude_bridge_dir,
            publication_sink: Arc::new(
                studio_runtime_services::notification_runtime_effects::RuntimeNotificationPostCommitSink {
                    token_rates: state.token_rate_handle(),
                    resources: state.resource_notifications_handle(),
                },
            ),
            environment: std::env::vars().collect(),
        });
        let mut runtime = Self::with_launcher_broker(
            store,
            launcher,
            state.event_wake_handle(),
            RegistryRuntimeOptions {
                continuation_wake: state.continuation_wake_handle(),
                monitor_dispatch_wake,
                session_broker,
                preparation_resources: Some(Arc::clone(&preparation_resources)),
                config,
                managed_tool_runtime,
            },
        )
        .await?;
        runtime.preparation_resources = Some(preparation_resources);
        Ok(runtime)
    }

    #[doc(hidden)]
    pub async fn with_launcher(
        store: Arc<RuntimeStore>,
        launcher: Arc<dyn AccountLauncher>,
        wake: Arc<tokio::sync::Notify>,
        config: AccountRegistryConfig,
    ) -> Result<Self, WorkerError> {
        Self::with_launcher_broker(
            store,
            launcher,
            wake,
            RegistryRuntimeOptions {
                continuation_wake: Arc::new(tokio::sync::Notify::new()),
                monitor_dispatch_wake: Arc::new(tokio::sync::Notify::new()),
                session_broker: AccountSessionBroker::new(),
                preparation_resources: None,
                config,
                managed_tool_runtime: None,
            },
        )
        .await
    }

    async fn with_launcher_broker(
        store: Arc<RuntimeStore>,
        launcher: Arc<dyn AccountLauncher>,
        wake: Arc<tokio::sync::Notify>,
        options: RegistryRuntimeOptions,
    ) -> Result<Self, WorkerError> {
        let RegistryRuntimeOptions {
            continuation_wake,
            monitor_dispatch_wake,
            session_broker,
            preparation_resources,
            config,
            managed_tool_runtime,
        } = options;
        let registry = AccountRegistry::new_with_broker(
            store,
            launcher,
            monitor_dispatch_wake,
            session_broker,
            config,
            managed_tool_runtime,
        )
        .await?;
        let shutdown = Shutdown::default();
        let task = tokio::spawn(registry.run(wake, continuation_wake, shutdown.clone()));
        Ok(Self {
            shutdown,
            task,
            preparation_resources,
        })
    }

    pub fn preparation_resources(&self) -> Option<Arc<PreparationResources>> {
        self.preparation_resources.clone()
    }

    pub fn stop_intake(&self) {
        self.shutdown.cancel();
    }

    pub fn shutdown_handle(&self) -> Shutdown {
        self.shutdown.clone()
    }

    pub async fn shutdown(self) -> Result<(), WorkerError> {
        self.shutdown.cancel();
        match timeout(HOST_RUNTIME_SHUTDOWN_WAIT, self.task).await {
            Ok(result) => result.map_err(|_| WorkerError::TaskFailed)?,
            Err(_) => {
                eprintln!(
                    "runtime server detached the account registry task after its {:.0}s shutdown bound",
                    HOST_RUNTIME_SHUTDOWN_WAIT.as_secs_f64()
                );
                Err(WorkerError::TaskFailed)
            }
        }
    }
}

/// The HTTP adapter shares the retained account session and committed callback
/// reply lane used by the account worker registry.
///
/// Lifecycle deletion integration hook: in the same transaction that tombstones
/// the agent and deletes its voice rows, call
/// `voice::provider::stage_delete_sessions(tx, agent_id)` before row cleanup.
/// After commit, pass the returned intent's `id` and `sessions` to
/// `voice::provider::end_deleted_sessions` using this installed voice port.
fn install_host_voice_port(
    state: HttpState,
    store: Arc<RuntimeStore>,
    callbacks: Arc<studio_runtime_services::callback_runtime::CallbackRuntime>,
) -> HttpState {
    match state.account_session.clone() {
        Some(account_session) => state.with_voice_request_port(Arc::new(
            studio_runtime_services::voice::account_session::AccountSessionVoicePort::new(
                store,
                account_session,
                callbacks,
            ),
        )),
        None => state,
    }
}

struct HostLauncher {
    factory: SupervisorProviderTaskFactory,
    supervisor_mode: bool,
    child_priority: ChildPriority,
    provider_stderr_log: Option<studio_runtime_process::operational_log::OperationalLog>,
    accounts: Arc<AccountDriver>,
    store: Arc<RuntimeStore>,
    root: PathBuf,
    native_path: Option<String>,
    native_runtime: Option<Arc<NativeRuntime>>,
    session_binaries: Arc<Mutex<BTreeMap<String, crate::provider_ports::SessionExecutables>>>,
    resources: Arc<PreparationResources>,
    claude_bridge_dir: PathBuf,
    publication_sink:
        Arc<dyn studio_runtime_services::notification_runtime_effects::NotificationPostCommitSink>,
    environment: BTreeMap<String, String>,
}

impl AccountLauncher for HostLauncher {
    fn launch(&self, account: String) -> LaunchFuture<'_> {
        Box::pin(async move {
            let accounts = self.accounts.clone();
            let key = account.clone();
            let (kind, home, isolated, profile) =
                studio_runtime_process::blocking_pool::spawn_blocking(
                    "runtime-server/host_runtime",
                    move || selected_account(&accounts, &key),
                )
                .await
                .map_err(|_| WorkerError::TaskFailed)??;
            let mut resources = (*self.resources).clone();
            resources.account_home.clone_from(&home);
            let accounts = self.accounts.clone();
            let key = account.clone();
            resources.claude_account_pin = studio_runtime_process::blocking_pool::spawn_blocking(
                "runtime-server/host_runtime",
                move || accounts.auth_pin(&key),
            )
            .await
            .map_err(|_| WorkerError::TaskFailed)?
            .map_err(|_| WorkerError::PreparationRefused("account-driver/unavailable".into()))?;
            // The selected native bundle is a Codex executable; keep its lease
            // for as long as the retained session can launch from it.
            let lease = match &self.native_runtime {
                Some(runtime) if kind == ProviderKind::Codex => {
                    let status = runtime.read_state().map_err(|_| {
                        WorkerError::PreparationRefused("native-runtime/status-read".into())
                    })?;
                    if status.as_ref().and_then(|state| state.selected()).is_some() {
                        Some(runtime.retain_selected().map_err(|_| {
                            WorkerError::PreparationRefused("native-runtime/selected-lease".into())
                        })?)
                    } else {
                        None
                    }
                }
                _ => None,
            };
            let codex_executable = lease
                .as_ref()
                .and_then(|lease| lease.path().to_str())
                .map(str::to_owned)
                .or_else(|| self.native_path.clone());
            let ProviderLaunch {
                spec: launch,
                executable,
            } = adapter(kind).host_launch(HostLaunch {
                profile: profile.as_ref(),
                home,
                isolated,
                codex_executable: codex_executable.as_deref(),
                claude_bridge_dir: &self.claude_bridge_dir,
                account: &account,
                root: self.root.clone(),
                environment: self.environment.clone(),
            })?;
            let clock = SessionClock::default();
            let handle = ProviderHandle::for_account(&account);
            let open = OpenOptions {
                clock: clock.clone(),
                launch: launch.clone(),
                expected_generation: None,
            };
            let (factory, generation): (Arc<dyn ProviderTaskFactory>, _) = if self.supervisor_mode {
                let prepared = self
                    .factory
                    .prepare(handle.clone(), open, Shutdown::default())
                    .await
                    .map_err(
                        studio_runtime_services::provider_session_task::SessionTaskError::from,
                    )?;
                let generation = prepared.generation();
                (Arc::new(prepared), generation)
            } else {
                let generation = next_direct_generation(&self.store, &account).await?;
                let prepared = DirectProviderTaskFactory::new(
                    self.child_priority,
                    self.provider_stderr_log.clone(),
                )
                .prepare(
                    handle.clone(),
                    OpenOptions {
                        expected_generation: Some(generation),
                        ..open
                    },
                    &Shutdown::default(),
                )
                .map_err(studio_runtime_services::provider_session_task::SessionTaskError::from)?;
                debug_assert_eq!(generation, prepared.generation());
                (Arc::new(prepared), generation)
            };
            let factory = match lease {
                Some(lease) => Arc::new(LeasedProviderTaskFactory {
                    inner: factory,
                    _lease: lease,
                }) as Arc<dyn ProviderTaskFactory>,
                None => factory,
            };
            if let Ok(mut binaries) = self.session_binaries.lock() {
                binaries.insert(
                    account.clone(),
                    crate::provider_ports::SessionExecutables {
                        generation: generation.0,
                        kind,
                        executable,
                    },
                );
            }
            Ok(AccountLaunch {
                factory,
                options: OpenOptions {
                    clock,
                    launch,
                    expected_generation: Some(generation),
                },
                config: SessionTaskConfig {
                    initialize_params: studio_runtime_providers::initialize_params(
                        studio_runtime_providers::ClientName::AccountSession,
                    ),
                    generation,
                    wire: kind.wire(),
                    timeout_ms: REQUEST_TIMEOUT_MS,
                    initialize_operation: OperationId(format!(
                        "account:{account}:initialize:{}",
                        generation.0
                    )),
                    request_id_prefix: None,
                    recovered: vec![],
                },
                resources,
                publication_sink: Arc::clone(&self.publication_sink),
            })
        })
    }
}

/// Keeps the selected immutable native bundle alive as long as the retained
/// account session factory can still launch or serve that provider session.
struct LeasedProviderTaskFactory {
    inner: Arc<dyn ProviderTaskFactory>,
    _lease: NativeExecutableLease,
}

impl ProviderTaskFactory for LeasedProviderTaskFactory {
    fn open(
        &self,
        handle: ProviderHandle,
        options: OpenOptions,
        shutdown: Shutdown,
    ) -> studio_runtime_process::provider_seam::OpenFuture<'_> {
        self.inner.open(handle, options, shutdown)
    }
}

#[cfg(test)]
#[allow(clippy::items_after_test_module, clippy::unwrap_used)]
#[allow(clippy::disallowed_methods)]
mod install_root_tests {
    use super::{AccountDriver, CLAUDE_BRIDGE_RELATIVE, HttpState, InstallRoot};
    use std::{fs, path::PathBuf};
    use studio_runtime_store::RuntimeStore;

    #[test]
    fn bridge_matches_install_layout_in_both_modes() {
        let source_checkout = studio_runtime_store::path_identity::CanonicalPath::existing(
            PathBuf::from(
                std::env::var_os("CARGO_MANIFEST_DIR").expect("cargo sets CARGO_MANIFEST_DIR"),
            )
            .join("../../../.."),
        )
        .unwrap();
        let source_layout = InstallRoot(source_checkout.clone().to_path_buf());
        assert_eq!(
            source_layout.claude_bridge_dir(),
            source_checkout.join(CLAUDE_BRIDGE_RELATIVE)
        );

        let temporary = tempfile::tempdir().unwrap();
        let packaged_root = temporary.path().join("workspace");
        let packaged_scripts = packaged_root.join("scripts");
        let packaged_bridge = packaged_scripts.join("claude_bridge");
        fs::create_dir_all(&packaged_bridge).unwrap();
        fs::write(packaged_bridge.join("bridge.mjs"), "").unwrap();
        let packaged_layout = InstallRoot(packaged_root);
        assert_eq!(
            packaged_layout.claude_bridge_dir(),
            packaged_scripts.join("claude_bridge")
        );
    }

    #[test]
    fn install_root_resolves_packaged_bridge_relative_to_executable() {
        let temporary = tempfile::tempdir().unwrap();
        let root = temporary.path().join("package");
        let bridge = root.join("scripts/claude_bridge");
        fs::create_dir_all(&bridge).unwrap();
        fs::write(bridge.join("bridge.mjs"), "").unwrap();
        let executable = root.join("bin/studio-runtime-server");
        let resolved = super::resolve_install_root_from_executable(Some(&executable)).unwrap();
        assert_eq!(resolved.workspace_root(), root);
    }

    #[test]
    fn install_root_source_fallback_is_debug_only() {
        // No executable location can find a bridge in this case. A release
        // test build must refuse even while its original checkout still exists.
        let resolved = super::resolve_install_root_from_executable(None);
        #[cfg(debug_assertions)]
        assert_eq!(
            resolved.unwrap().workspace_root(),
            studio_runtime_store::path_identity::CanonicalPath::existing(
                PathBuf::from(
                    std::env::var_os("CARGO_MANIFEST_DIR").expect("cargo sets CARGO_MANIFEST_DIR")
                )
                .join("../../../..")
            )
            .unwrap()
        );
        #[cfg(not(debug_assertions))]
        assert!(matches!(
            resolved,
            Err(super::WorkerError::PreparationRefused(message))
                if message.starts_with("runtime-server/install-root:")
        ));
    }

    #[tokio::test]
    async fn host_runtime_serves_session_and_accounts() {
        use axum::{
            body::{Body, to_bytes},
            extract::ConnectInfo,
            http::Request,
        };
        use std::sync::Arc;
        use tower::ServiceExt;

        let root = tempfile::tempdir().expect("isolated runtime state creates");
        let store = Arc::new(
            RuntimeStore::open(Some(root.path()))
                .await
                .expect("runtime store opens"),
        );
        let registry_path = root.path().join("accounts/registry.json");
        fs::create_dir_all(registry_path.parent().expect("registry parent"))
            .expect("account registry directory creates");
        fs::write(
            &registry_path,
            studio_json::json!({"version":studio_runtime_schema::shared_values::ACCOUNT_REGISTRY_FORMAT_VERSION,"defaultAccountKey":"default","accounts":{},"logins":{},"deleteReceipts":{}}).to_string().as_bytes(),
        )
        .expect("empty account registry writes");
        let registry_bytes = fs::read(&registry_path).expect("account registry reads");
        let accounts =
            Arc::new(AccountDriver::open(registry_path).expect("empty account registry opens"));
        let account_policy = Arc::new(
            studio_runtime_services::account_policy::AccountPolicyState::from_startup(
                Some(&registry_bytes),
                std::iter::empty::<(String, studio_json::Value)>(),
            )
            .expect("empty account policy loads"),
        );
        let mut state = HttpState::new(
            "host-runtime-token".into(),
            "host-runtime-workspace".into(),
            root.path().to_string_lossy().into_owned(),
            4620,
            None,
            Some(store.clone()),
            Some(account_policy),
        )
        .with_account_driver(accounts.clone());
        let runtime = super::HostRuntime::start_with_monitor_wake_and_native_runtime(
            &mut state,
            store,
            accounts,
            None,
            Arc::new(tokio::sync::Notify::new()),
            super::AccountSessionBroker::new(),
            None,
            Arc::new(std::sync::Mutex::new(std::collections::BTreeMap::new())),
            super::AccountRegistryConfig::default(),
            None,
            None,
        )
        .await
        .expect("composed host runtime starts");

        let app = studio_runtime_http::router(state).expect("HTTP routes compose");
        async fn get(app: &axum::Router, path: &str) -> (u16, studio_json::Value) {
            let response = app
                .clone()
                .oneshot(
                    Request::builder()
                        .uri(path)
                        .header("host", "127.0.0.1:4620")
                        .extension(ConnectInfo(std::net::SocketAddr::from((
                            [127, 0, 0, 1],
                            12345,
                        ))))
                        .body(Body::empty())
                        .expect("request builds"),
                )
                .await
                .expect("composed HTTP response");
            let status = response.status().as_u16();
            let body = to_bytes(response.into_body(), 4096)
                .await
                .expect("JSON body");
            (
                status,
                studio_json::from_slice(&body).expect("response JSON"),
            )
        }

        let (session_status, session) = get(&app, "/api/session").await;
        assert_eq!(session_status, 200);
        assert_eq!(session["token"], "host-runtime-token");
        let (accounts_status, accounts) = get(&app, "/api/accounts").await;
        assert_eq!(accounts_status, 200);
        assert!(accounts["accounts"].as_array().is_some());

        runtime
            .shutdown()
            .await
            .expect("composed host runtime shuts down");
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod voice_composition_tests {
    use super::install_host_voice_port;
    use axum::{
        body::{Body, to_bytes},
        extract::ConnectInfo,
        http::{Request, StatusCode},
    };
    use std::{
        collections::BTreeMap,
        fs,
        path::Path,
        sync::{
            Arc,
            atomic::{AtomicUsize, Ordering},
        },
    };
    use studio_json::{Value, json};
    use studio_runtime_http::{HttpState, router};
    use studio_runtime_process::{
        EventCursor, Generation, OperationId, ProviderHandle, ProviderLaunchSpec,
        provider_seam::{
            AdapterCommand, AttachmentInfo, AttachmentStatus, CommandReceipt, InitializationStatus,
            JournalEntry, JournalPayload, LANE_CAPACITY, OpenFuture, OpenOptions,
            ProviderAttachment, ProviderTaskFactory, SessionClock, Shutdown, TaskExit,
        },
    };
    use studio_runtime_providers::{Frame, RequestId, ResponseBody};
    use studio_runtime_services::{
        account_driver::AccountDriver,
        account_session_runtime::{
            AccountSessionError, AccountSessionRuntime, ConnectionScope, ProviderRequest,
            RetainedAccountSession, SessionFuture, SessionReceipt as AccountSessionReceipt,
        },
        account_worker::{
            AccountHandle, StoreBridge,
            preparation::PreparationResources,
            retained::{PortError, SessionRequest},
            start_account_worker_with_preparation,
        },
        callback_runtime::{CallbackReplyPort, CallbackRuntime},
        model_catalog::ModelCatalogService,
        provider_callbacks::{CallbackError, Reply},
        provider_session_task::SessionTaskConfig,
    };
    use studio_runtime_store::RuntimeStore;
    use tower::ServiceExt;

    struct WorkerSessionAdapter {
        port: studio_runtime_services::account_worker::retained::RetainedSessionPort,
    }

    impl RetainedAccountSession for WorkerSessionAdapter {
        fn current_scope(&self, account: &str) -> Option<ConnectionScope> {
            let scope = self.port.scope().clone();
            (scope.account == account).then_some(scope)
        }

        fn request(&self, request: ProviderRequest) -> SessionFuture {
            let port = self.port.clone();
            Box::pin(async move {
                let scope = port.scope().clone();
                if request.account != scope.account {
                    return Err(AccountSessionError::Refused(
                        "Provider session identity changed",
                    ));
                }
                let operation = request.operation.clone();
                let retained = port
                    .request(SessionRequest {
                        root_request_id: request.root_request_id,
                        admitted_request: request.admitted_request,
                        scope,
                        expected_scope: request.expected_scope,
                        operation,
                        actor: request.actor,
                        transfer: None,
                        deleted_actor_session: request.deleted_actor_session,
                        method: request.method,
                        params: request.params,
                        deadline: request.deadline,
                    })
                    .await
                    .map_err(|error| match error {
                        PortError::Expired => AccountSessionError::Expired,
                        PortError::Unknown => AccountSessionError::Unknown,
                        PortError::Capacity => AccountSessionError::Persistence,
                        PortError::Identity
                        | PortError::ScopeMismatch
                        | PortError::Invalid
                        | PortError::NotSent(_)
                        | PortError::Deadline => {
                            AccountSessionError::Refused("Provider request was refused")
                        }
                    })?;
                let mut receipt: AccountSessionReceipt = retained.into();
                receipt.replayed = false;
                Ok(receipt)
            })
        }
    }

    struct UnusedCallbackReplyPort;

    impl CallbackReplyPort for UnusedCallbackReplyPort {
        fn has_live_session(
            &self,
            _: &studio_runtime_services::provider_callbacks::CallbackContext,
        ) -> bool {
            false
        }

        fn deliver<'a>(
            &'a self,
            _reply: &'a Reply,
            _deadline: std::time::Duration,
        ) -> studio_runtime_services::callback_runtime::CallbackReplyFuture<'a> {
            Box::pin(async {
                Err(CallbackError::Dependency(
                    "test callback reply lane is unused",
                ))
            })
        }
    }

    #[derive(Default)]
    struct VoiceFactory {
        stop_writes: Arc<AtomicUsize>,
        request_writes: Arc<AtomicUsize>,
        stop_refusals: Arc<AtomicUsize>,
        stop_attempts: Arc<AtomicUsize>,
        stop_signal: Arc<tokio::sync::Notify>,
    }

    async fn seed_voice_agent(store: &RuntimeStore, name: &'static str) {
        use studio_runtime_store::{AgentId, AgentRecord, AgentRepository, AgentStatus};
        store
            .transaction(move |tx| {
                let id = AgentId::from(name);
                AgentRepository::put(
                    tx,
                    &AgentRecord {
                        title_job: None,
                        start_outcome_hold: None,
                        fired_events: Default::default(),
                        native_review_thread: None,
                        vm_launch: None,
                        credential_copy: None,
                        vm_auth_bootstrap: None,
                        native_release: None,
                        id: id.clone(),
                        root_id: id,
                        epoch: 3,
                        status: AgentStatus::Idle,
                        is_lead: Some(true),
                        account_key: "voice-account".into(),
                        thread_id: Some("voice-thread".into()),
                        auto_wake: None,
                        usage_resume_enabled: None,
                        in_flight: None,
                        execution_mode: None,
                        turn_epoch: None,
                        turn_id: None,
                        provider: studio_runtime_schema::queue_command::Field::Value(
                            studio_runtime_schema::ProviderKind::Codex,
                        ),
                        ..AgentRecord::test_defaults()
                    },
                )?;
                Ok(())
            })
            .await
            .expect("seed voice agent");
    }

    async fn seed_ready_voice_session(
        store: Arc<RuntimeStore>,
        session: Arc<AccountSessionRuntime>,
        callbacks: Arc<CallbackRuntime>,
        agent: &'static str,
        voice_session: &str,
    ) {
        let provider = Arc::new(
            studio_runtime_services::voice::account_session::AccountSessionVoicePort::new(
                store.clone(),
                session.clone(),
                callbacks,
            ),
        );
        studio_runtime_services::voice::provider::start(
            store.clone(),
            provider,
            agent.into(),
            voice_session.into(),
            "v=0\r\n".into(),
            1.0,
        )
        .await
        .expect("start voice session");
        tokio::time::timeout(
            super::HOST_RUNTIME_SHUTDOWN_WAIT,
            studio_runtime_services::voice::provider::wait_for_start_setup(agent, voice_session),
        )
        .await
        .expect("voice setup completes");
        let scope =
            studio_runtime_services::account_session_runtime::AccountSessionPort::current_scope(
                session.as_ref(),
                "voice-account",
            )
            .unwrap();
        studio_runtime_services::voice::provider::handle_notification(
            &store, "voice-account", &scope.connection,
            &json!({"method":"thread/realtime/sdp","params":{"threadId":"voice-thread","realtimeSessionId":voice_session,"sdp":"answer"}}), 1.0,
        ).await.expect("voice session ready");
    }

    impl ProviderTaskFactory for VoiceFactory {
        fn open(
            &self,
            handle: ProviderHandle,
            _options: OpenOptions,
            shutdown: Shutdown,
        ) -> OpenFuture<'_> {
            let generation = Generation(42);
            let (commands, mut inbox) = tokio::sync::mpsc::channel(LANE_CAPACITY);
            let (events, journal) = tokio::sync::mpsc::channel(LANE_CAPACITY);
            let (status, receiver) = tokio::sync::watch::channel(AttachmentStatus {
                handle: handle.clone(),
                open_attempted: true,
                info: Some(AttachmentInfo {
                    spawn_started_at: None,
                    initialized_at: None,
                    handle: handle.clone(),
                    generation,
                    pid: 42,
                    acknowledged: EventCursor(0),
                    resumed: false,
                }),
                initialization: InitializationStatus::AwaitingReply,
            });
            let mut stop = shutdown.subscribe();
            let stop_writes = Arc::clone(&self.stop_writes);
            let request_writes = Arc::clone(&self.request_writes);
            let stop_refusals = Arc::clone(&self.stop_refusals);
            let stop_attempts = Arc::clone(&self.stop_attempts);
            let stop_signal = Arc::clone(&self.stop_signal);
            let task = tokio::spawn(async move {
                let mut cursor = 0_u64;
                loop {
                    let command = tokio::select! {
                        _ = stop.changed() => break,
                        command = inbox.recv() => command,
                    };
                    let Some(command) = command else { break };
                    match command {
                        AdapterCommand::Write {
                            write,
                            reply,
                            observed,
                        } => {
                            let message = write.message();
                            let method = message
                                .get("method")
                                .and_then(Value::as_str)
                                .unwrap_or_default();
                            if method == "initialize" {
                                let _ignored =
                                    reply.send(CommandReceipt::CachedInitialize(json!({})));
                                if observed.await.is_err() {
                                    break;
                                }
                                continue;
                            }
                            if method == "thread/realtime/stop" {
                                stop_attempts.fetch_add(1, Ordering::SeqCst);
                                stop_signal.notify_one();
                                if stop_refusals
                                    .try_update(Ordering::SeqCst, Ordering::SeqCst, |n| {
                                        n.checked_sub(1)
                                    })
                                    .is_ok()
                                {
                                    let _ignored = reply.send(CommandReceipt::NotSent);
                                    continue;
                                }
                            }
                            let _ignored = reply.send(CommandReceipt::Written {
                                written_at: None,
                                remote_id: None,
                                duplicate: false,
                            });
                            if observed.await.is_err() {
                                break;
                            }
                            let Some(id) = message
                                .get("id")
                                .cloned()
                                .and_then(|id| studio_json::from_value::<RequestId>(id).ok())
                            else {
                                continue;
                            };
                            if (method == "account/read"
                                && message["params"]["refreshToken"] == true)
                                || method == "thread/realtime/appendSpeech"
                            {
                                request_writes.fetch_add(1, Ordering::SeqCst);
                            }
                            if method == "thread/realtime/stop" {
                                stop_writes.fetch_add(1, Ordering::SeqCst);
                            }
                            let result = match method {
                                "account/read" => json!({"account":{"type":"chatgpt"}}),
                                "thread/loaded/list" => json!({"data":[],"nextCursor":null}),
                                _ => json!({}),
                            };
                            cursor += 1;
                            if events
                                .send(JournalEntry {
                                    boundary_at: None,
                                    received_at_ms: cursor,
                                    cursor: EventCursor(cursor),
                                    generation,
                                    payload: JournalPayload::Frame(Frame::Response {
                                        id,
                                        body: ResponseBody::Result(result),
                                        fields: Default::default(),
                                    }),
                                })
                                .await
                                .is_err()
                            {
                                break;
                            }
                        }
                        AdapterCommand::Ack { cursor, reply } => {
                            let _ignored = reply.send(CommandReceipt::Acknowledged(cursor));
                        }
                    }
                }
                drop(status);
                TaskExit {
                    result: Ok(()),
                    cleanup_error: None,
                    recovery_needed: None,
                    provider_retained: false,
                }
            });
            Box::pin(async move {
                Ok(ProviderAttachment::from_task(
                    commands, journal, receiver, shutdown, task,
                ))
            })
        }
    }

    async fn start_voice_worker(
        directory: &Path,
        store: Arc<RuntimeStore>,
        factory: Arc<VoiceFactory>,
    ) -> (
        AccountHandle,
        Arc<AccountSessionRuntime>,
        Arc<CallbackRuntime>,
    ) {
        let bridge = StoreBridge::new(store.clone()).await.unwrap();
        let resources = PreparationResources {
            instructions: String::new(),
            thread_config: Default::default(),
            tools: Vec::new(),
            legacy_tools: Vec::new(),
            skills_root: directory.to_path_buf(),
            role_guidance: BTreeMap::new(),
            model_catalog: ModelCatalogService::new(),
            account_home: String::new(),
            shared_home: String::new(),
            filesystem_inputs: None,
            claude_account_pin: json!({"provider":studio_runtime_schema::ProviderKind::Codex}),
            account_driver: None,
            account_session: None,
        };
        let worker = start_account_worker_with_preparation(
            bridge,
            "voice-account".into(),
            factory,
            OpenOptions {
                clock: SessionClock::default(),
                launch: ProviderLaunchSpec::new(
                    vec!["fake".into()],
                    BTreeMap::new(),
                    "/tmp".into(),
                ),
                expected_generation: None,
            },
            SessionTaskConfig {
                initialize_params: json!({}),
                generation: Generation(42),
                wire: studio_runtime_providers::ProviderKind::Codex.wire(),
                timeout_ms: 2_000,
                initialize_operation: OperationId("initialize:voice-account".into()),
                request_id_prefix: Some("voice-account".into()),
                recovered: Vec::new(),
            },
            resources,
        )
        .await
        .unwrap();
        let account_root = directory.join("accounts");
        fs::create_dir_all(&account_root).unwrap();
        let registry_path = account_root.join("registry.json");
        if !registry_path.exists() {
            fs::write(
                &registry_path,
                json!({
                    "version":studio_runtime_schema::shared_values::ACCOUNT_REGISTRY_FORMAT_VERSION,"defaultAccountKey":"voice-account",
                    "accounts":{"voice-account":{"provider":studio_runtime_schema::ProviderKind::Codex,"id":"voice-account","home":directory,"label":"voice","source":"test","status":"signedOut"}},
                    "logins":{},"deleteReceipts":{}
                })
                .to_string(),
            )
            .unwrap();
        }
        let driver = Arc::new(AccountDriver::open(registry_path).unwrap());
        let retained: Arc<dyn RetainedAccountSession> = Arc::new(WorkerSessionAdapter {
            port: worker.retained_session(),
        });
        let session = Arc::new(AccountSessionRuntime::new(
            driver,
            Some(retained),
            tokio::runtime::Handle::current(),
            account_root,
            directory.to_path_buf(),
            directory.to_path_buf(),
        ));
        let callbacks = Arc::new(CallbackRuntime::new(
            store,
            Arc::new(UnusedCallbackReplyPort),
        ));
        (worker, session, callbacks)
    }

    #[tokio::test]
    async fn expired_provider_retry_refuses_before_signature_check_without_sending() {
        use studio_runtime_schema::persistence::RequestIdentity;
        use studio_runtime_store::request_results::RequestResults;
        let directory = tempfile::tempdir().unwrap();
        let store = Arc::new(RuntimeStore::open(Some(directory.path())).await.unwrap());
        let old_root = uuid::Uuid::from_u128((1 << 80) | (7 << 76) | (2 << 62)).to_string();
        let operation = old_root.clone();
        let identity = RequestIdentity::new(
            "provider-call",
            &["voice-account", "", ""],
            &old_root,
            &[&operation],
        )
        .unwrap();
        store.transaction(move |tx| {
            let cutover = identity.issued_ms() + 1;
            tx.execute("INSERT INTO runtime_orchestration_settings(id,record) VALUES('global',json_object('expired_before_ms',?1)) ON CONFLICT(id) DO UPDATE SET record=json_set(record,'$.expired_before_ms',?1)", [cutover])?;
            assert_eq!(RequestResults::expired_before_ms(tx)?, cutover);
            tx.execute("INSERT INTO runtime_agents(id,record) VALUES('expired-agent',?1)", [json!({"compactions":0,"id":"expired-agent","rootId":"expired-agent","epoch":3,"status":"idle","isLead":true,"provider":"codex","threadId":"voice-thread","accountKey":"voice-account"}).to_string()])?;
            // A saved row with a different signature must not mask the expiry refusal.
            tx.execute("INSERT INTO request_results(id,signature,result,created) VALUES(?1,'original-import-signature','{}',?2)", rusqlite::params![identity.id(),cutover])?;
            Ok(())
        }).await.unwrap();
        let factory = Arc::new(VoiceFactory {
            stop_writes: Arc::new(AtomicUsize::new(0)),
            request_writes: Arc::new(AtomicUsize::new(0)),
            ..Default::default()
        });
        let (mut worker, session, _callbacks) =
            start_voice_worker(directory.path(), Arc::clone(&store), Arc::clone(&factory)).await;
        let scope =
            studio_runtime_services::account_session_runtime::AccountSessionPort::current_scope(
                session.as_ref(),
                "voice-account",
            )
            .unwrap();
        let sent_before_retry = factory.request_writes.load(Ordering::SeqCst);
        let result =
            studio_runtime_services::account_session_runtime::AccountSessionPort::provider_request(
                session.as_ref(),
                ProviderRequest {
                    root_request_id: old_root,
                    admitted_request: None,
                    account: "voice-account".into(),
                    operation,
                    actor: None,
                    expected_scope: Some(scope),
                    deleted_actor_session: None,
                    method: studio_runtime_providers::Method::AccountLoginStart,
                    params: json!({"type":"chatgptDeviceCode"}),
                    deadline: std::time::Instant::now() + std::time::Duration::from_secs(2),
                },
            )
            .await;
        assert!(
            matches!(result, Err(AccountSessionError::Expired)),
            "{:?}",
            result.as_ref().err()
        );
        assert_eq!(
            factory.request_writes.load(Ordering::SeqCst),
            sent_before_retry
        );
        worker.shutdown().await.unwrap();
    }

    #[tokio::test]
    async fn voice_speech_reserves_and_replays_through_real_worker() {
        let directory = tempfile::tempdir().unwrap();
        let store = Arc::new(RuntimeStore::open(Some(directory.path())).await.unwrap());
        seed_voice_agent(&store, "speech-agent").await;
        let factory = Arc::new(VoiceFactory::default());
        let voice_session = studio_runtime_schema::persistence::RequestIdentity::issue_root();
        let (mut worker, session, callbacks) =
            start_voice_worker(directory.path(), store.clone(), factory.clone()).await;
        seed_ready_voice_session(
            store.clone(),
            session.clone(),
            callbacks.clone(),
            "speech-agent",
            &voice_session,
        )
        .await;
        let record_session = voice_session.clone();
        store.transaction(move |tx| {
            tx.execute("INSERT INTO voice_records(id,agent,session,kind,text,item_id,previous_item_id,payload,created) VALUES('speech-record','speech-agent',?1,'orchestrator','hello','','','{}',1)", [&record_session])?;
            Ok(())
        }).await.unwrap();
        let mut state = HttpState::new(
            "token".into(),
            "workspace".into(),
            directory.path().display().to_string(),
            4620,
            None,
            Some(store.clone()),
            None,
        );
        state.account_session = Some(session);
        let provider = install_host_voice_port(state, store.clone(), callbacks)
            .voice_request_port_handle()
            .expect("voice provider");
        let before = factory.request_writes.load(Ordering::SeqCst);
        for _ in 0..2 {
            studio_runtime_services::voice::provider::speech(
                store.clone(),
                provider.clone(),
                "speech-agent".into(),
                voice_session.clone(),
                "speech-record".into(),
                None,
            )
            .await
            .expect("speech receipt");
        }
        assert_eq!(factory.request_writes.load(Ordering::SeqCst), before + 1);
        worker.shutdown().await.unwrap();
    }

    #[tokio::test]
    async fn stale_unsent_voice_stop_reaches_real_worker_after_lifecycle_stop() {
        use studio_runtime_services::{
            agent_management::Action, lifecycle_effects::ProductionLifecyclePorts,
        };
        let directory = tempfile::tempdir().unwrap();
        let store = Arc::new(RuntimeStore::open(Some(directory.path())).await.unwrap());
        seed_voice_agent(&store, "stale-agent").await;
        let factory = Arc::new(VoiceFactory::default());
        let voice_session = studio_runtime_schema::persistence::RequestIdentity::issue_root();
        factory.stop_refusals.store(1, Ordering::SeqCst);
        let (mut worker, session, callbacks) =
            start_voice_worker(directory.path(), store.clone(), factory.clone()).await;
        seed_ready_voice_session(
            store.clone(),
            session.clone(),
            callbacks.clone(),
            "stale-agent",
            &voice_session,
        )
        .await;
        let provider: Arc<dyn studio_runtime_services::voice::runtime::VoiceRequestPort> = Arc::new(
            studio_runtime_services::voice::account_session::AccountSessionVoicePort::new(
                store.clone(),
                session.clone(),
                callbacks,
            ),
        );
        let result = studio_runtime_services::voice::provider::end(
            store.clone(),
            provider.clone(),
            "stale-agent".into(),
            voice_session.clone(),
            1.0,
        )
        .await;
        assert!(
            matches!(
                result,
                Err(studio_runtime_services::voice::provider::VoiceProviderError::NotSent)
            ),
            "{result:?}, attempts={}",
            factory.stop_attempts.load(Ordering::SeqCst)
        );
        assert_eq!(factory.stop_attempts.load(Ordering::SeqCst), 1);
        let ports = Arc::new(ProductionLifecyclePorts::new(
            store.clone(),
            Arc::new(tokio::sync::Notify::new()),
            Arc::new(tokio::sync::Notify::new()),
            session.clone(),
            Some(provider),
        ));
        ports
            .execute(
                &Action::Pause {
                    target: "stale-agent".into(),
                    descendants: false,
                    reason: None,
                    sender: None,
                    sender_epoch: None,
                },
                2.0,
            )
            .await
            .unwrap();
        let scope =
            studio_runtime_services::account_session_runtime::AccountSessionPort::current_scope(
                session.as_ref(),
                "voice-account",
            )
            .expect("live provider scope");
        let old =
            studio_runtime_services::account_session_runtime::AccountSessionPort::provider_request(
                session.as_ref(),
                ProviderRequest {
                    root_request_id:
                        studio_runtime_schema::persistence::RequestIdentity::issue_root(),
                    admitted_request: None,
                    account: "voice-account".into(),
                    operation: format!("voice:stop:{voice_session}"),
                    actor: Some(
                        studio_runtime_schema::provider_call_contract::RequestActor {
                            agent: "stale-agent".into(),
                            epoch: 3,
                        },
                    ),
                    expected_scope: Some(scope),
                    deleted_actor_session: None,
                    method: studio_runtime_providers::Method::ThreadRealtimeStop,
                    params: json!({"threadId":"voice-thread"}),
                    deadline: std::time::Instant::now() + std::time::Duration::from_secs(2),
                },
            )
            .await;
        assert!(
            match old {
                Ok(receipt) => receipt.certainty()
                    == studio_runtime_services::account_session_runtime::RequestCertainty::NotSent,
                Err(AccountSessionError::Refused("Provider request was refused")) => true,
                Err(_) => false,
            },
            "old epoch cannot dispatch"
        );
        tokio::time::timeout(std::time::Duration::from_secs(10), async {
            loop {
                let notified = factory.stop_signal.notified();
                if factory.stop_attempts.load(Ordering::SeqCst) == 2 {
                    break;
                }
                notified.await;
            }
        })
        .await
        .expect("current-epoch lifecycle stop reaches the retained provider");
        assert_eq!(
            factory.stop_attempts.load(Ordering::SeqCst),
            2,
            "old epoch never reaches provider"
        );
        worker.shutdown().await.unwrap();
    }

    #[tokio::test]
    async fn unknown_voice_stop_replays_with_real_worker_without_resending() {
        use studio_runtime_schema::persistence::{CapturedScope, ProviderScope};
        use studio_runtime_store::{VoiceEffect, VoiceEffectAuthority, VoiceEffectDisposition};
        let directory = tempfile::tempdir().unwrap();
        let store = Arc::new(RuntimeStore::open(Some(directory.path())).await.unwrap());
        seed_voice_agent(&store, "legacy-agent").await;
        let voice_session = studio_runtime_schema::persistence::RequestIdentity::issue_root();
        let session_for_seed = voice_session.clone();
        store.transaction(move |tx| {
            let scope = ProviderScope { generation: 42, connection: "legacy-connection".into() };
            let effect = VoiceEffect {
                session_id: session_for_seed.clone(), agent: "legacy-agent".into(), epoch: 3,
                account: "voice-account".into(), thread: "voice-thread".into(),
                disposition: VoiceEffectDisposition::Active,
                authority: VoiceEffectAuthority::Held { scope: CapturedScope::Known(scope.clone()) },
            };
            tx.execute("INSERT INTO voice_sessions(id,agent,created,native_thread,account_key,connection_id,provider_scope,state) VALUES(?1,'legacy-agent',1,'voice-thread','voice-account','legacy-connection',?2,'ready')", rusqlite::params![session_for_seed, studio_json::to_string(&scope)?])?;
            tx.execute("INSERT INTO runtime_outbox(id,kind,owner_kind,owner_id,signature,status,payload,created) VALUES('held-stop','voice_stop','agent','legacy-agent','lifecycle.voice-stop.v1','unknown',?1,1)", [studio_json::to_string(&effect)?])?;
            Ok(())
        }).await.unwrap();
        let factory = Arc::new(VoiceFactory::default());
        let (mut worker, session, callbacks) =
            start_voice_worker(directory.path(), store.clone(), factory.clone()).await;
        let provider = Arc::new(
            studio_runtime_services::voice::account_session::AccountSessionVoicePort::new(
                store.clone(),
                session,
                callbacks,
            ),
        );
        for now in [2.0, 3.0] {
            let replay = studio_runtime_services::voice::provider::end(
                store.clone(),
                provider.clone(),
                "legacy-agent".into(),
                voice_session.clone(),
                now,
            )
            .await
            .expect("replay uncertain stop");
            assert_eq!(replay.get("state").and_then(Value::as_str), Some("ready"));
        }
        assert_eq!(factory.stop_attempts.load(Ordering::SeqCst), 0);
        assert_eq!(factory.stop_writes.load(Ordering::SeqCst), 0);
        worker.shutdown().await.unwrap();
    }

    #[tokio::test]
    async fn composed_runtime_server_routes_voice_through_the_shared_account_session() {
        let directory = tempfile::tempdir().unwrap();
        let store = Arc::new(RuntimeStore::open(Some(directory.path())).await.unwrap());
        let agent_record = json!({"compactions":0,"provider":studio_runtime_schema::ProviderKind::Codex,
            "id":"voice-composed-agent", "rootId":"voice-composed-agent", "epoch":3,
            "status":"idle", "isLead":true, "threadId":"voice-thread", "accountKey":"voice-account"
        })
        .to_string();
        store
            .transaction(move |tx| {
                tx.execute(
                    "INSERT INTO runtime_agents(id,record) VALUES(?1,?2)",
                    ("voice-composed-agent", agent_record.as_str()),
                )?;
                Ok(())
            })
            .await
            .unwrap();

        let factory = Arc::new(VoiceFactory {
            stop_writes: Arc::new(AtomicUsize::new(0)),
            request_writes: Arc::new(AtomicUsize::new(0)),
            ..Default::default()
        });
        let (mut worker, session, callbacks) =
            start_voice_worker(directory.path(), store.clone(), Arc::clone(&factory)).await;
        let session_id = studio_runtime_schema::persistence::RequestIdentity::issue_root();
        let mut state = HttpState::new(
            "token".into(),
            "workspace".into(),
            directory.path().display().to_string(),
            4620,
            None,
            Some(store.clone()),
            None,
        );
        state.account_session = Some(session);
        let app = router(install_host_voice_port(
            state,
            Arc::clone(&store),
            callbacks,
        ))
        .unwrap();
        let mut request = Request::builder()
            .method("POST")
            .uri("/api/voice/start")
            .header("host", "127.0.0.1:4620")
            .header("x-canvas-token", "token")
            .header("content-type", "application/json")
            .body(Body::from(
                json!({"agent":"voice-composed-agent","session_id":session_id,"sdp":"v=0\r\n"})
                    .to_string(),
            ))
            .unwrap();
        request.extensions_mut().insert(ConnectInfo(
            "127.0.0.1:32100".parse::<std::net::SocketAddr>().unwrap(),
        ));
        let response = app.oneshot(request).await.unwrap();
        let status = response.status();
        let body = to_bytes(response.into_body(), 100_000).await.unwrap();
        assert_eq!(status, StatusCode::OK, "{}", String::from_utf8_lossy(&body));
        let response: Value = studio_json::from_slice(&body).unwrap();
        assert_eq!(response["session_id"], session_id);
        assert_eq!(response["state"], "connecting");
        // The HTTP acceptance boundary persists the connecting session before
        // provider setup completes; wait on its existing completion barrier
        // before staging deletion from the durable rows.
        tokio::time::timeout(
            super::HOST_RUNTIME_SHUTDOWN_WAIT,
            studio_runtime_services::voice::provider::wait_for_start_setup(
                "voice-composed-agent",
                &session_id,
            ),
        )
        .await
        .expect("voice provider setup did not finish before timeout");

        // The deletion transaction persists stop authorization before removing
        // voice rows, matching Python's delete-before-post-commit-stop ordering.
        let delete_intent = store
            .transaction_with_error(move |transaction| -> Result<_, studio_runtime_services::voice::provider::VoiceProviderError> {
                transaction.execute(
                    "INSERT INTO voice_sessions(id,agent,created,native_thread,account_key,connection_id,provider_scope,state) \
                     SELECT 'voice-session-unknown',agent,created+1,native_thread,account_key,connection_id,provider_scope,state \
                     FROM voice_sessions WHERE id=?1 AND agent='voice-composed-agent'",
                    [&session_id],
                )?;
                transaction.execute(
                    "UPDATE runtime_agents SET record=json_set(record,'$.deletedAt',1) WHERE id='voice-composed-agent'",
                    [],
                )?;
                let intent = studio_runtime_services::voice::provider::stage_delete_sessions(
                    transaction,
                    "voice-composed-agent",
                )?;
                transaction.execute(
                    "DELETE FROM voice_sessions WHERE agent='voice-composed-agent'",
                    [],
                )?;
                Ok(intent)
            })
            .await
            .unwrap();
        assert_eq!(delete_intent.len(), 2);

        // Simulate a restart after the deletion transaction commits. Python
        // marks sessions on the vanished connection lost and never sends their
        // stop over a replacement session; Rust must settle both as Unknown.
        worker.shutdown().await.unwrap();
        let (mut post_delete_worker, post_delete_session, post_delete_callbacks) =
            start_voice_worker(directory.path(), Arc::clone(&store), Arc::clone(&factory)).await;
        let delete_provider: Arc<dyn studio_runtime_services::voice::runtime::VoiceRequestPort> =
            Arc::new(
                studio_runtime_services::voice::account_session::AccountSessionVoicePort::new(
                    Arc::clone(&store),
                    post_delete_session,
                    post_delete_callbacks,
                ),
            );
        for intent in &delete_intent {
            studio_runtime_services::voice::provider::end_deleted_sessions_tracked(
                Arc::clone(&store),
                Arc::clone(&delete_provider),
                intent.id.clone(),
                intent.session.clone(),
                2.0,
            )
            .await
            .unwrap();
        }
        post_delete_worker.shutdown().await.unwrap();
        let saved: Vec<_> = delete_intent
            .iter()
            .map(|intent| {
                studio_runtime_store::outbox::Outbox::get::<studio_runtime_store::VoiceEffect>(
                    &store.test_reader().unwrap(),
                    &intent.id,
                )
                .unwrap()
                .expect("unknown deletion remains durable")
            })
            .collect();
        assert_eq!(saved.len(), 2);
        assert!(
            saved
                .iter()
                .all(|row| row.status == studio_runtime_schema::persistence::OutboxStatus::Unknown)
        );
        assert!(saved.iter().all(|row| matches!(
            row.payload.authority,
            studio_runtime_store::VoiceEffectAuthority::Admitted {
                outcome: studio_runtime_schema::persistence::VoiceStopOutcome::Unknown,
                ..
            }
        )));
        assert!(
            studio_runtime_services::voice::provider::pending_delete_intents(&store)
                .await
                .unwrap()
                .is_empty()
        );
        assert_eq!(factory.stop_writes.load(Ordering::SeqCst), 0);

        // Recovery replays the known receipt and retains Unknown without another
        // provider write, even after another worker restart.
        let (mut recovered_worker, recovered_session, recovered_callbacks) =
            start_voice_worker(directory.path(), Arc::clone(&store), Arc::clone(&factory)).await;
        let recovered_provider: Arc<dyn studio_runtime_services::voice::runtime::VoiceRequestPort> =
            Arc::new(
                studio_runtime_services::voice::account_session::AccountSessionVoicePort::new(
                    Arc::clone(&store),
                    recovered_session,
                    recovered_callbacks,
                ),
            );
        for intent in &delete_intent {
            studio_runtime_services::voice::provider::end_deleted_sessions_tracked(
                Arc::clone(&store),
                Arc::clone(&recovered_provider),
                intent.id.clone(),
                intent.session.clone(),
                3.0,
            )
            .await
            .unwrap();
        }
        let replay: Vec<_> = delete_intent
            .iter()
            .map(|intent| {
                studio_runtime_store::outbox::Outbox::get::<studio_runtime_store::VoiceEffect>(
                    &store.test_reader().unwrap(),
                    &intent.id,
                )
                .unwrap()
                .expect("unknown deletion survives recovery")
            })
            .collect();
        assert_eq!(replay, saved);
        assert_eq!(factory.stop_writes.load(Ordering::SeqCst), 0);
        recovered_worker.shutdown().await.unwrap();
    }
}

#[cfg(test)]
mod direct_generation_tests {
    use super::*;

    #[tokio::test]
    async fn direct_generation_uses_only_migrated_account_counter() {
        let root = tempfile::tempdir().expect("isolated state");
        let store = RuntimeStore::open(Some(root.path())).await.expect("store");
        store.transaction(|tx| {
            studio_runtime_store::receipt_schema::create(tx)?;
            tx.execute("INSERT INTO runtime_account_state VALUES('alpha',41,NULL,NULL,0,0,'null',NULL,NULL)", [])?;
            // Any accidental history query must fail, including on a seeded counter.
            tx.execute_batch("DROP TABLE IF EXISTS runtime_operation_receipts")?;
            Ok(())
        }).await.expect("migrated fixture");
        assert_eq!(
            next_direct_generation(&store, "alpha").await.expect("next"),
            Generation(42)
        );
        assert_eq!(
            next_direct_generation(&store, "alpha").await.expect("next"),
            Generation(43)
        );
        assert_eq!(
            next_direct_generation(&store, "beta").await.expect("first"),
            Generation(1)
        );
    }
}

async fn next_direct_generation(
    store: &RuntimeStore,
    account: &str,
) -> Result<Generation, WorkerError> {
    let account = account.to_owned();
    store
        .transaction_with(move |tx| {
            studio_runtime_store::account_state::allocate_next_generation(tx, &account)
                .map(Generation)
                .map_err(WorkerError::from)
        })
        .await
}

/// The home and isolation of a Codex account whose provider session may start:
/// a ready account, or an account whose sign-in request is being dispatched
/// (the sign-in runs in that session, before the account is ready).
pub fn account_home<R: ProviderAuthReader>(
    accounts: &AccountDriver<R>,
    key: &str,
) -> Result<(String, bool), WorkerError> {
    if let Some(record) = accounts
        .login_launch_record(key)
        .map_err(|_| WorkerError::PreparationRefused("account-driver/unavailable".into()))?
    {
        let home = record["home"]
            .as_str()
            .filter(|home| !home.is_empty())
            .ok_or_else(|| WorkerError::PreparationRefused("account-driver/profile-home".into()))?
            .to_owned();
        return Ok((home, key != "default"));
    }
    let initial = accounts
        .snapshot()
        .map_err(|_| WorkerError::PreparationRefused("account-driver/unavailable".into()))?;
    let initial_profile = initial
        .accounts
        .iter()
        .find(|profile| profile.id.as_str() == Some(key))
        .ok_or_else(|| WorkerError::PreparationRefused("account-driver/unavailable".into()))?;
    if initial_profile.provider_kind().map(ProviderKind::key) != Some(ProviderKind::Codex.key()) {
        return Err(WorkerError::PreparationRefused(
            "provider-session/claude-runtime-launch".into(),
        ));
    }
    if accounts
        .get(key)
        .map_err(|_| WorkerError::PreparationRefused("account-driver/unavailable".into()))?
        != AccountStatus::Ready
    {
        return Err(WorkerError::PreparationRefused(
            "account-driver/unavailable".into(),
        ));
    }
    let snapshot = accounts
        .snapshot()
        .map_err(|_| WorkerError::PreparationRefused("account-driver/unavailable".into()))?;
    let profile = snapshot
        .accounts
        .iter()
        .find(|profile| profile.id.as_str() == Some(key))
        .ok_or_else(|| WorkerError::PreparationRefused("account-driver/unavailable".into()))?;
    if profile.provider_kind().map(ProviderKind::key) != Some(ProviderKind::Codex.key()) {
        return Err(WorkerError::PreparationRefused(
            "provider-session/claude-runtime-launch".into(),
        ));
    }
    if profile.disconnected.as_bool() == Some(true)
        || profile.deleted.as_bool() == Some(true)
        || profile.status.as_str() != Some("ready")
    {
        return Err(WorkerError::PreparationRefused(
            "account-driver/unavailable".into(),
        ));
    }
    let home = profile
        .home
        .as_str()
        .filter(|home| !home.is_empty())
        .ok_or_else(|| WorkerError::PreparationRefused("account-driver/profile-home".into()))?
        .to_owned();
    Ok((home, key != "default"))
}

/// The account's provider, home, isolation and registry profile (Claude: the
/// pinned profile the bridge launch verifies).
fn selected_account(
    accounts: &AccountDriver,
    key: &str,
) -> Result<
    (
        ProviderKind,
        String,
        bool,
        Option<studio_runtime_services::claude_bridge::ClaudeLaunchProfile>,
    ),
    WorkerError,
> {
    let snapshot = accounts
        .snapshot()
        .map_err(|_| WorkerError::PreparationRefused("account-driver/unavailable".into()))?;
    // A sign-in account is not listed until it has an identity; its sign-in
    // request still launches its session.
    let kind = match snapshot
        .accounts
        .iter()
        .find(|profile| profile.id.as_str() == Some(key))
    {
        Some(profile) => profile.provider().map_err(|_| {
            WorkerError::PreparationRefused("provider-session/unsupported-provider".into())
        })?,
        None => {
            let profile = accounts
                .login_launch_record(key)
                .ok()
                .flatten()
                .ok_or_else(|| {
                    WorkerError::PreparationRefused("account-driver/unavailable".into())
                })?;
            ProviderKind::of_record(&profile).map_err(|_| {
                WorkerError::PreparationRefused("provider-session/unsupported-provider".into())
            })?
        }
    };
    if kind == ProviderKind::Codex {
        let (home, isolated) = account_home(accounts, key)?;
        return Ok((kind, home, isolated, None));
    }
    let profile = accounts
        .claude_launch_profile(key)
        .map_err(|_| WorkerError::PreparationRefused("account-driver/unavailable".into()))?;
    let home = profile
        .home
        .as_str()
        .filter(|value| !value.is_empty())
        .ok_or_else(|| WorkerError::PreparationRefused("account-driver/profile-home".into()))?
        .to_owned();
    Ok((kind, home, false, Some(profile)))
}

#[cfg(test)]
mod launch_profile_tests {
    #[test]
    fn starting_codex_login_selects_without_a_launch_profile() {
        let root = tempfile::tempdir().expect("temporary root");
        let home = root.path().join("login-home");
        std::fs::create_dir_all(&home).expect("home");
        let registry = root.path().join("registry.json");
        let write = |status: &str| {
            std::fs::write(&registry, studio_json::json!({
                "version": studio_runtime_schema::shared_values::ACCOUNT_REGISTRY_FORMAT_VERSION,
                "defaultAccountKey":"default", "accounts": {
                    "default":{"provider":"codex","id":"default","home":"","label":"Default","source":"CLI","status":"signedOut"},
                    "login-fixture":{"provider":"codex","id":"login-fixture","home":home,"label":"Login","source":"CLI","status":"pending"}
                }, "logins":{"request":{"requestId":"request","accountKey":"login-fixture","status":status,"createdAt":1.0}},"deleteReceipts":{}
            }).to_string()).expect("registry");
            super::AccountDriver::open(&registry).expect("driver")
        };
        let driver = write("starting");
        let (kind, selected_home, isolated, profile) =
            super::selected_account(&driver, "login-fixture").expect("starting login");
        assert_eq!(kind, studio_runtime_providers::ProviderKind::Codex);
        assert_eq!(selected_home, home.to_string_lossy());
        assert!(isolated && profile.is_none());
        assert!(super::selected_account(&write("pending"), "login-fixture").is_err());
    }
}
