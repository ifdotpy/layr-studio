//! Asynchronous provider adapters, process ports and the supervisor v2 client.

/// Open the running executable even when its original name was replaced or unlinked.
#[allow(
    clippy::disallowed_methods,
    reason = "startup callers open their executable on the blocking pool or startup thread"
)]
pub fn current_executable_file() -> std::io::Result<std::fs::File> {
    #[cfg(target_os = "linux")]
    let path = std::path::PathBuf::from("/proc/self/exe");
    #[cfg(not(target_os = "linux"))]
    let path = std::env::current_exe()?;
    std::fs::File::open(path)
}

pub use studio_runtime_store::blocking_pool;
pub mod child_priority;
mod client;
#[cfg(unix)]
pub mod clock;
#[cfg(unix)]
pub mod open_files;
pub mod operational_log;
#[cfg(unix)]
pub mod owned_process_group;
mod ports;
mod protocol;
#[cfg(unix)]
pub mod provider_direct;
mod provider_launch;
pub mod provider_seam;
mod provider_shared;
pub use provider_shared::retained_command_accepted;
#[cfg(any(target_os = "linux", target_os = "macos"))]
pub use provider_shared::retained_environment_accepted;
#[cfg(unix)]
pub mod provider_stdio;
#[cfg(unix)]
pub mod provider_task;
#[cfg(unix)]
mod pty_host;
#[cfg(unix)]
pub mod stream_child;
#[cfg(unix)]
mod supervisor_child;
mod supervisor_contract;
#[cfg(unix)]
pub mod supervisor_daemon;
#[cfg(unix)]
pub(crate) mod supervisor_identity;
mod transport;

pub use client::SupervisorClient;
pub use ports::PtyHost;
pub use protocol::*;
pub use provider_launch::{ProviderHandle, ProviderLaunchSpec, RetainedLaunchValues};
#[cfg(unix)]
pub use pty_host::{
    LocalPtyHost, PtyHostError, PtyIdentity, PtySession, cleanup_orphan, terminal_environment,
    terminal_environment_from,
};
pub use supervisor_contract::MAX_SUPERVISOR_FRAME_BYTES;
pub use transport::SupervisorTransport;

#[cfg(test)]
mod tests;
