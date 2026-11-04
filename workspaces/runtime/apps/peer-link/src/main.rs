//! The link between paired Studio servers (docs/peer-link.md).
//!
//! The backend starts this process and writes the configuration as the first stdin line. Later
//! lines replace the paired keys. The process exits when stdin closes. The link uses one iroh
//! endpoint: a peer is dialed and accepted by its key, over a direct path or a relay.

mod frame;
mod identity;
mod inbound;
mod outbound;

use std::collections::{HashMap, HashSet};
use std::io::Write;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::{Arc, Mutex, RwLock};
use std::time::Duration;

use iroh::endpoint::{BindOpts, Connection, IdleTimeout, QuicTransportConfig, VarInt, presets};
use iroh::{Endpoint, PublicKey, RelayMode, RelayUrl, SecretKey};
use serde::Deserialize;
use tokio::io::{AsyncBufReadExt, BufReader};

use identity::PairedKeys;

const CONNECT_SECONDS: u64 = 5;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Config {
    listen: SocketAddr,
    private_key: String,
    public_key: String,
    peers: Vec<String>,
    backend_port: u16,
    forwarded_host: String,
    socket: PathBuf,
    /// Secret shared with the backend. The link sends it with the connection identity.
    link_token: String,
    /// Relay URLs. Empty (the default) means direct paths only: no relay is used.
    #[serde(default)]
    relays: Vec<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Update {
    peers: Vec<String>,
}

/// The paired keys and the open connections. A connection whose key leaves the set closes.
pub struct Links {
    endpoint: Endpoint,
    keys: PairedKeys,
    /// The PEM of each paired key, to name a connection in the `connected` line.
    names: RwLock<HashMap<PublicKey, String>>,
    inbound: Mutex<Vec<Connection>>,
    outbound: tokio::sync::Mutex<HashMap<PublicKey, Connection>>,
    /// Receives each `connected` line. The process writes it to stdout.
    on_connected: Box<dyn Fn(&str) + Send + Sync>,
}

impl Links {
    fn new(
        endpoint: Endpoint,
        keys: PairedKeys,
        names: HashMap<PublicKey, String>,
        on_connected: Box<dyn Fn(&str) + Send + Sync>,
    ) -> Self {
        Self {
            endpoint,
            keys,
            names: RwLock::new(names),
            inbound: Mutex::new(Vec::new()),
            outbound: tokio::sync::Mutex::new(HashMap::new()),
            on_connected,
        }
    }

    fn track_inbound(&self, connection: &Connection) {
        if let Ok(mut connections) = self.inbound.lock() {
            connections.retain(|connection| connection.close_reason().is_none());
            connections.push(connection.clone());
        }
        self.announce(connection);
    }

    /// Tell the backend that a connection to a paired server opened. The backend then sends the
    /// outbox entries of that server at once.
    fn announce(&self, connection: &Connection) {
        let name = self
            .names
            .read()
            .ok()
            .and_then(|names| names.get(&connection.remote_id()).cloned());
        if let Some(name) = name {
            (self.on_connected)(&studio_json::json!({"connected": name}).to_string());
        }
    }

    /// The connection to a paired server. It is reused while open, and dialed again after a drop.
    /// The address is a direct hint; iroh also uses the relays and the paths it learns.
    async fn connect(&self, address: SocketAddr, id: PublicKey) -> Result<Connection, String> {
        let mut connections = self.outbound.lock().await;
        if let Some(connection) = connections
            .get(&id)
            .filter(|connection| connection.close_reason().is_none())
        {
            return Ok(connection.clone());
        }
        let remote = iroh::EndpointAddr::new(id).with_ip_addr(address);
        let connection = tokio::time::timeout(
            Duration::from_secs(CONNECT_SECONDS),
            self.endpoint.connect(remote, identity::ALPN),
        )
        .await
        .map_err(|_| "The connection timed out".to_owned())?
        .map_err(|error| error.to_string())?;
        connections.insert(id, connection.clone());
        self.announce(&connection);
        Ok(connection)
    }

    async fn replace_keys(&self, keys: HashSet<PublicKey>, names: HashMap<PublicKey, String>) {
        if let Ok(connections) = self.inbound.lock() {
            for connection in connections
                .iter()
                .filter(|connection| !keys.contains(&connection.remote_id()))
            {
                connection.close(VarInt::from_u32(2), b"revoked");
            }
        }
        self.outbound.lock().await.retain(|id, connection| {
            if keys.contains(id) {
                return true;
            }
            connection.close(VarInt::from_u32(2), b"revoked");
            false
        });
        if let Ok(mut current) = self.names.write() {
            *current = names;
        }
        if let Ok(mut current) = self.keys.write() {
            *current = keys;
        }
    }
}

fn paired_keys(peers: &[String]) -> Result<HashSet<PublicKey>, String> {
    peers
        .iter()
        .map(|peer| identity::endpoint_id(&identity::spki(peer)?))
        .collect()
}

fn peer_names(peers: &[String]) -> Result<HashMap<PublicKey, String>, String> {
    peers
        .iter()
        .map(|peer| Ok((identity::endpoint_id(&identity::spki(peer)?)?, peer.clone())))
        .collect()
}

fn relay_mode(relays: &[String]) -> Result<RelayMode, String> {
    if relays.is_empty() {
        return Ok(RelayMode::Disabled);
    }
    let urls = relays
        .iter()
        .map(|relay| {
            relay
                .parse::<RelayUrl>()
                .map_err(|_| "A relay URL is invalid".to_owned())
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(RelayMode::custom(urls))
}

fn transport() -> Result<QuicTransportConfig, String> {
    let idle = IdleTimeout::try_from(Duration::from_secs(60))
        .map_err(|_| "The idle timeout is invalid".to_owned())?;
    Ok(QuicTransportConfig::builder()
        .keep_alive_interval(Duration::from_secs(10))
        .max_idle_timeout(Some(idle))
        .build())
}

/// The endpoint without a bind address. The caller adds the sockets.
fn builder(
    secret: SecretKey,
    keys: PairedKeys,
    relay_mode: RelayMode,
) -> Result<iroh::endpoint::Builder, String> {
    Ok(Endpoint::builder(presets::Minimal)
        .secret_key(secret)
        .alpns(vec![identity::ALPN.to_vec()])
        .relay_mode(relay_mode)
        .transport_config(transport()?)
        .hooks(identity::Pairing { keys }))
}

/// All interfaces (the backend default), a Tailscale address, or loopback (tests).
/// Only the IPv4 unspecified address is accepted for all interfaces: the IPv6 socket is added with it.
fn listen_allowed(address: IpAddr) -> bool {
    address == IpAddr::V4(Ipv4Addr::UNSPECIFIED)
        || inbound::tailnet(address)
        || address.is_loopback()
}

fn valid_host(host: &str) -> bool {
    (1..=255).contains(&host.len())
        && host
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b".-:".contains(&byte))
}

fn print_line(line: &str) {
    let mut stdout = std::io::stdout().lock();
    writeln!(stdout, "{line}").ok();
    stdout.flush().ok();
}

async fn run() -> Result<(), String> {
    let mut lines = BufReader::new(tokio::io::stdin()).lines();
    let first = lines
        .next_line()
        .await
        .map_err(|error| error.to_string())?
        .ok_or("The configuration is missing")?;
    let config: Config =
        studio_json::from_str(&first).map_err(|_| "The configuration is invalid".to_owned())?;
    if !listen_allowed(config.listen.ip()) || !valid_host(&config.forwarded_host) {
        return Err(
            "Listen on all interfaces, a Tailscale address or loopback, and use the Serve host"
                .to_owned(),
        );
    }
    let secret = identity::secret_key(&config.private_key, &config.public_key)?;
    let keys: PairedKeys = Arc::new(RwLock::new(paired_keys(&config.peers)?));
    let mut endpoint_builder = builder(secret, keys.clone(), relay_mode(&config.relays)?)?
        .bind_addr(config.listen)
        .map_err(|error| error.to_string())?;
    if config.listen.ip().is_unspecified() {
        // IPv6 is optional: a host without IPv6 keeps the IPv4 socket.
        endpoint_builder = endpoint_builder
            .bind_addr_with_opts(
                SocketAddr::new(Ipv6Addr::UNSPECIFIED.into(), config.listen.port()),
                BindOpts::default().set_is_required(false),
            )
            .map_err(|error| error.to_string())?;
    }
    let endpoint = endpoint_builder
        .bind()
        .await
        .map_err(|error| error.to_string())?;
    let listener = outbound::bind(&config.socket).map_err(|error| error.to_string())?;
    // The backend advertises the IPv4 port, so the ready line names the IPv4 socket.
    let port = endpoint
        .bound_sockets()
        .iter()
        .find(|address| address.is_ipv4())
        .map(|address| address.port())
        .ok_or("The endpoint has no IPv4 socket")?;
    let links = Arc::new(Links::new(
        endpoint.clone(),
        keys,
        peer_names(&config.peers)?,
        Box::new(print_line),
    ));
    let inbound = Arc::new(inbound::Inbound {
        backend_port: config.backend_port,
        forwarded_host: config.forwarded_host,
        link_token: config.link_token,
    });
    // The ready line comes first: a `connected` line may follow it, never precede it.
    print_line(&studio_json::json!({"ready": true, "port": port}).to_string());
    tokio::spawn(inbound::serve(endpoint.clone(), inbound, links.clone()));
    tokio::spawn(outbound::listen(listener, links.clone()));
    while let Some(line) = lines.next_line().await.map_err(|error| error.to_string())? {
        let update: Update =
            studio_json::from_str(&line).map_err(|_| "The key update is invalid".to_owned())?;
        links
            .replace_keys(paired_keys(&update.peers)?, peer_names(&update.peers)?)
            .await;
    }
    endpoint.close().await;
    std::fs::remove_file(&config.socket).ok();
    Ok(())
}

fn main() -> ExitCode {
    let runtime = match tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(_) => return ExitCode::FAILURE,
    };
    match runtime.block_on(run()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            writeln!(std::io::stderr(), "studio-peer-link: {error}").ok();
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests;
