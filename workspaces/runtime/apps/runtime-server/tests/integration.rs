mod browser_launcher;
mod install_layout;
mod live_handover;
mod migration_startup;
mod operator_cli;
mod state_check;
mod wave7_p01_registry;
// Register every integration test module here; the tooling witness checks coverage.

#[cfg(unix)]
pub(crate) use studio_runtime_server::host_runtime;

mod fixture_support;
