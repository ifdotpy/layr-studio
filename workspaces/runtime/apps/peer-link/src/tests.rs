//! Link tests on loopback and on a local relay (iroh test utilities). Each test binds endpoints
//! with fresh keys.

use std::collections::{HashMap, HashSet};
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::{Arc, Mutex, RwLock};
use std::time::{Duration, Instant};

use iroh::endpoint::{Connection, ConnectionError, Endpoint, VarInt};
use iroh::tls::CaTlsConfig;
use iroh::{EndpointAddr, PublicKey, RelayConfig, RelayMap, RelayMode, RelayUrl, SecretKey};
use iroh_relay::server::{
    AllowAll, CertConfig, RelayConfig as RelayServerConfig, Server, ServerConfig, TlsConfig,
};
use rustls::pki_types::CertificateDer;

use crate::identity::{self, PairedKeys};
use crate::{Links, builder};

const LOCAL: SocketAddr = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 0);

/// A node with a fresh pairing key, as the backend writes it (PKCS#8 and SPKI PEM).
struct Node {
    secret: SecretKey,
    private: String,
    public: String,
}

fn node() -> Node {
    let secret = SecretKey::generate();
    let mut private = vec![
        0x30, 0x2e, 0x02, 0x01, 0x00, 0x30, 0x05, 0x06, 0x03, 0x2b, 0x65, 0x70, 0x04, 0x22, 0x04,
        0x20,
    ];
    private.extend_from_slice(&secret.to_bytes());
    let mut public = vec![
        0x30, 0x2a, 0x30, 0x05, 0x06, 0x03, 0x2b, 0x65, 0x70, 0x03, 0x21, 0x00,
    ];
    public.extend_from_slice(secret.public().as_bytes());
    Node {
        private: pem("PRIVATE KEY", &private),
        public: pem("PUBLIC KEY", &public),
        secret,
    }
}

fn pem(label: &str, der: &[u8]) -> String {
    use base64::Engine;
    let body = base64::engine::general_purpose::STANDARD.encode(der);
    format!("-----BEGIN {label}-----\n{body}\n-----END {label}-----\n")
}

fn keys(peers: &[&Node]) -> PairedKeys {
    Arc::new(RwLock::new(
        peers
            .iter()
            .map(|peer| peer.secret.public())
            .collect::<HashSet<_>>(),
    ))
}

async fn endpoint(
    node: &Node,
    keys: PairedKeys,
    relay: RelayMode,
    loopback: bool,
    ca: Option<CaTlsConfig>,
) -> Endpoint {
    let secret =
        identity::secret_key(&node.private, &node.public).unwrap_or_else(|error| panic!("{error}"));
    let mut builder = builder(secret, keys, relay).unwrap_or_else(|error| panic!("{error}"));
    if let Some(ca) = ca {
        builder = builder.ca_tls_config(ca);
    }
    builder = if loopback {
        builder
            .bind_addr(LOCAL)
            .unwrap_or_else(|error| panic!("{error}"))
    } else {
        builder.clear_ip_transports()
    };
    builder
        .bind()
        .await
        .unwrap_or_else(|error| panic!("{error}"))
}

/// Answers each stream with its four bytes, as a request answer would.
fn echo(connection: Connection) {
    tokio::spawn(async move {
        while let Ok((mut send, mut recv)) = connection.accept_bi().await {
            tokio::spawn(async move {
                let mut buffer = [0_u8; 4];
                if recv.read_exact(&mut buffer).await.is_ok() {
                    send.write_all(&buffer).await.ok();
                    send.finish().ok();
                }
            });
        }
    });
}

/// Serves each connection that passes the endpoint hooks.
fn serve(endpoint: Endpoint) {
    tokio::spawn(async move {
        while let Some(incoming) = endpoint.accept().await {
            tokio::spawn(async move {
                if let Ok(connection) = incoming.await {
                    echo(connection);
                }
            });
        }
    });
}

/// One request on a connection: a new stream, four bytes out, four bytes back.
async fn round_trip(connection: &Connection) -> Result<(), String> {
    let (mut send, mut recv) = connection
        .open_bi()
        .await
        .map_err(|error| error.to_string())?;
    send.write_all(b"ping")
        .await
        .map_err(|error| error.to_string())?;
    send.finish().map_err(|error| error.to_string())?;
    let mut buffer = [0_u8; 4];
    recv.read_exact(&mut buffer)
        .await
        .map_err(|error| error.to_string())?;
    if &buffer == b"ping" {
        Ok(())
    } else {
        Err("The answer is wrong".to_owned())
    }
}

type Lines = Arc<Mutex<Vec<String>>>;
type OnConnected = Box<dyn Fn(&str) + Send + Sync>;

fn recorder() -> (Lines, OnConnected) {
    let lines = Lines::default();
    let sink = lines.clone();
    (
        lines,
        Box::new(move |line: &str| {
            if let Ok(mut lines) = sink.lock() {
                lines.push(line.to_owned());
            }
        }),
    )
}

fn names(peer: &Node) -> HashMap<PublicKey, String> {
    HashMap::from([(peer.secret.public(), peer.public.clone())])
}

#[tokio::test]
async fn dials_and_accepts_by_key_on_a_direct_path() {
    let (a, b) = (node(), node());
    let server = endpoint(&b, keys(&[&a]), RelayMode::Disabled, true, None).await;
    let client = endpoint(&a, keys(&[&b]), RelayMode::Disabled, true, None).await;
    let address = server.bound_sockets()[0];
    serve(server.clone());
    let connection = client
        .connect(
            EndpointAddr::new(b.secret.public()).with_ip_addr(address),
            identity::ALPN,
        )
        .await
        .unwrap_or_else(|error| panic!("{error}"));
    round_trip(&connection)
        .await
        .unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(connection.remote_id(), b.secret.public());
}

#[tokio::test]
async fn a_key_outside_the_paired_set_is_refused() {
    let (a, b, c) = (node(), node(), node());
    // The server pairs with a only. The client c is not paired with the server.
    let server = endpoint(&b, keys(&[&a]), RelayMode::Disabled, true, None).await;
    let client = endpoint(&c, keys(&[&b]), RelayMode::Disabled, true, None).await;
    let address = server.bound_sockets()[0];
    serve(server.clone());
    let refused = match client
        .connect(
            EndpointAddr::new(b.secret.public()).with_ip_addr(address),
            identity::ALPN,
        )
        .await
    {
        Err(_) => true,
        Ok(connection) => {
            let closed = connection.closed().await;
            round_trip(&connection).await.is_err()
                && matches!(
                    closed,
                    ConnectionError::ApplicationClosed(close)
                        if close.error_code.into_inner() == u64::from(identity::REFUSED)
                )
        }
    };
    assert!(refused, "the server ran a request for an unpaired key");
}

#[tokio::test]
async fn a_peer_outside_the_dialer_set_is_not_dialed() {
    let (a, b) = (node(), node());
    let server = endpoint(&b, keys(&[&a]), RelayMode::Disabled, true, None).await;
    let client = endpoint(&a, keys(&[]), RelayMode::Disabled, true, None).await;
    let address = server.bound_sockets()[0];
    serve(server.clone());
    let links = Links::new(client, keys(&[]), HashMap::new(), Box::new(|_| {}));
    assert!(links.connect(address, b.secret.public()).await.is_err());
}

#[tokio::test]
async fn reconnects_after_a_drop_and_announces_again() {
    let (a, b) = (node(), node());
    let server = endpoint(&b, keys(&[&a]), RelayMode::Disabled, true, None).await;
    let client = endpoint(&a, keys(&[&b]), RelayMode::Disabled, true, None).await;
    let address = server.bound_sockets()[0];
    serve(server.clone());
    let (lines, on_connected) = recorder();
    let links = Links::new(client, keys(&[&b]), names(&b), on_connected);
    let first = links
        .connect(address, b.secret.public())
        .await
        .unwrap_or_else(|error| panic!("{error}"));
    first.close(VarInt::from_u32(0), b"drop");
    first.closed().await;
    let second = links
        .connect(address, b.secret.public())
        .await
        .unwrap_or_else(|error| panic!("{error}"));
    round_trip(&second)
        .await
        .unwrap_or_else(|error| panic!("{error}"));
    assert_ne!(first.stable_id(), second.stable_id());
    let lines = lines.lock().map(|lines| lines.clone()).unwrap_or_default();
    assert_eq!(lines.len(), 2, "{lines:?}");
    assert!(lines.iter().all(|line| line.starts_with("{\"connected\"")));
}

#[tokio::test]
async fn connects_through_a_local_relay_without_a_direct_path() {
    let (relays, url, cert, _server) = relay_server().await;
    let ca = CaTlsConfig::custom_roots([cert]);
    let (a, b) = (node(), node());
    let server = endpoint(
        &b,
        keys(&[&a]),
        RelayMode::Custom(relays.clone()),
        false,
        Some(ca.clone()),
    )
    .await;
    let client = endpoint(&a, keys(&[&b]), RelayMode::Custom(relays), false, Some(ca)).await;
    for endpoint in [&server, &client] {
        tokio::time::timeout(Duration::from_secs(20), endpoint.online())
            .await
            .unwrap_or_else(|_| panic!("the endpoint did not reach the relay"));
    }
    serve(server.clone());
    let remote = EndpointAddr::new(b.secret.public()).with_relay_url(url);
    let connection = client
        .connect(remote, identity::ALPN)
        .await
        .unwrap_or_else(|error| panic!("{error}"));
    round_trip(&connection)
        .await
        .unwrap_or_else(|error| panic!("{error}"));
}

/// A relay on loopback with a self-signed certificate, which the endpoints trust as a root.
async fn relay_server() -> (RelayMap, RelayUrl, CertificateDer<'static>, Server) {
    let (certs, server_config) = iroh_relay::server::testing::self_signed_tls_certs_and_config();
    let mut relay = RelayServerConfig::new((Ipv4Addr::LOCALHOST, 0));
    relay.tls = Some(TlsConfig::new(
        (Ipv4Addr::LOCALHOST, 0),
        CertConfig::Manual { server_config },
    ));
    relay.access = Arc::new(AllowAll);
    let mut config = ServerConfig::default();
    config.relay = Some(relay);
    let server = Server::spawn(config)
        .await
        .unwrap_or_else(|error| panic!("{error}"));
    let address = server
        .https_addr()
        .unwrap_or_else(|| panic!("the relay has no HTTPS address"));
    let url: RelayUrl = format!("https://{address}")
        .parse()
        .unwrap_or_else(|error| panic!("{error}"));
    let map: RelayMap = RelayConfig::new(url.clone(), None).into();
    (map, url, certs[0].clone(), server)
}

/// Measures one small request on loopback (docs/peer-link.md). Run with `--nocapture`.
#[tokio::test]
async fn loopback_round_trip_time() {
    let (a, b) = (node(), node());
    let server = endpoint(&b, keys(&[&a]), RelayMode::Disabled, true, None).await;
    let client = endpoint(&a, keys(&[&b]), RelayMode::Disabled, true, None).await;
    let address = server.bound_sockets()[0];
    serve(server.clone());
    let connection = client
        .connect(
            EndpointAddr::new(b.secret.public()).with_ip_addr(address),
            identity::ALPN,
        )
        .await
        .unwrap_or_else(|error| panic!("{error}"));
    let mut times = Vec::new();
    for _ in 0..200 {
        let started = Instant::now();
        round_trip(&connection)
            .await
            .unwrap_or_else(|error| panic!("{error}"));
        times.push(started.elapsed());
    }
    times.sort();
    let median: Duration = times[times.len() / 2];
    eprintln!(
        "loopback round trip: median {median:?} over {} requests",
        times.len()
    );
}
