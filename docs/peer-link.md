# QUIC link between paired servers

Paired Studio servers send their signed requests over one QUIC connection per server pair. The
HTTPS channel through Tailscale Serve stays as the fallback. The link changes only the
transport: request signatures, nonces, receipts, request IDs and the Tailscale owner check
stay as in the multi-server credential contract and `docs/cross-server-orchestration.md`.

## Measurement

Two Macs on one Wi-Fi network, a direct Tailscale path, 2026-10-14:

| Transfer                                 | HTTPS (Node helper and Serve) | QUIC link      |
| ---------------------------------------- | ----------------------------- | -------------- |
| One small request                        | about 600 ms                  | 10 ms (median) |
| 512 KiB JSON chunks, one after the other | about 2 MiB/s                 | 8.7 MiB/s      |
| One raw stream of 430 MiB                | not available                 | 23 MiB/s       |

The network limit is about 24 MiB/s (`scp` of 200 MiB). The HTTPS time is mostly the Node
process that each request starts, and the Serve hop.

## Process

`studio-peer-link` (`workspaces/runtime/apps/peer-link`, Rust, `iroh` 1.3.0 on its QUIC stack `noq`) is a child
of the backend (`runtime-server/src/peer_link.rs`). The backend writes its configuration as the first stdin
line, so the private key is never in the arguments or the environment:

```json
{
  "listen": "100.x.y.z:4621",
  "privateKey": "<PKCS#8 PEM>",
  "publicKey": "<SPKI PEM>",
  "peers": ["<SPKI PEM>"],
  "backendPort": 4620,
  "forwardedHost": "<Serve host>",
  "socket": "<state>/multi-server/peer-link.sock",
  "linkToken": "<secret>",
  "relays": []
}
```

The process answers `{"ready": true, "port": N}`. After that line it writes `{"connected": "<SPKI PEM>"}` each time a QUIC connection to a paired server opens, inbound or outbound. The backend then makes the waiting outbox entries of that server due at once (`release_peer`), without a backoff wait. A later line `{"peers": [...]}` replaces the
paired keys and closes the connections of a removed key. The process exits when stdin closes,
so it ends with its backend. The backend starts it on Unix when the Serve origin is known. After
an exit it starts the process again with a doubling delay of at most 30 seconds; the delay resets
after 60 seconds of healthy run. It listens on the Tailscale IPv4 address of the Mac, on UDP
port 4621, or on a free port when 4621 is in use.

`relays` is a list of relay URLs (optional, default empty). Empty means direct paths only: the
process uses no relay, so no public relay sees the traffic. The owner turns relays on by adding
URLs to the backend's link configuration. A relay forwards encrypted QUIC only.

## Identity

One iroh endpoint with the ALPN `studio-peer/1`. The endpoint key is the server's Ed25519 pairing
key, so the endpoint id is the paired public key. The TLS 1.3 handshake of iroh authenticates
both sides by that key (mutual, no second key).

- The process refuses a key outside `peers` before any stream. Its `EndpointHooks` check the
  remote key before a dial (`before_connect`) and after the handshake of an accepted connection
  (`after_handshake`). A refused connection closes with the code `0x100`.
- The dialer and the accepting side both use the same check, so a key removed by `{"peers": ...}`
  closes its connections.
- The process accepts any network path for a paired key: a direct address, the Tailscale address
  when it is one, or a relay. The network path gives no rights.
- The backend's Tailscale owner check (below) still runs for each request. It is removed by the
  project grants change, not by this link.

## Contract version

A paired server states its contract version (the `api-schema` hash of
`runtime-http/openapi.json`) on every signed request, in the header `X-Studio-API-Schema`. The
sender is `remote_transport.rs`; the check is in `incoming_multi_server.rs`, after the signature
check and for a paired server only (a paired window must match the current hash, as before).

- A server accepts the current hash and the earlier hashes in its history. Any other hash gets
  `426` with `X-Studio-API-Schema-Mismatch: 1`. A request with no hash comes from a build from
  before this check; it is accepted as an earlier version. This case is removed one release
  later (the missing hash is then refused like an unknown hash). So a rolling update keeps the
  peers working, and a peer too far behind is refused.
- The history has one owner: `runtime-schema/previous-api-schema-hashes.txt` (newest first),
  read by `api_schema::previous_api_schema_hashes`. `pnpm run contracts:generate` writes it:
  when the generated hash changes, the hash that was current until then is added in front.
  The list keeps `api_schema::PEER_HASH_HISTORY_LEN` (20) hashes, because installs are less
  frequent than contract changes: a peer one install behind is often several contract changes
  behind. `contracts:check` fails when the file is not that value.
- A field leaves the contract only one release after its last reader is gone, so a server
  can decode a request of the previous version. A server decodes no other version.
- The stream of a frontend uses the first frame instead: `/api/sync/stream` sends one
  `api-schema` frame with `mismatch: true` and ends when the hash of the frontend differs.

## Streams

Each request is one bidirectional stream: a 4-byte length and a JSON head
`{method, path, headers, bodyLength}`, then the body. The answer is a 4-byte length and a JSON
head `{status, headers}`, then the body until the stream ends. Limits: a head of 64 KiB, 64
headers, a request body of 64 MiB, the methods GET, POST, PUT, PATCH and DELETE, and paths
under `/api/`.

The receiving link sends the request to the backend on `127.0.0.1` with the headers that Serve
sets: `Host`, `X-Forwarded-Host` (the Serve host), `X-Forwarded-Proto: https` and
`X-Forwarded-For` (the QUIC source address). It removes these headers, `Tailscale-*`,
`Forwarded` and the hop headers from the peer's request. The backend boundary
(`runtime-http/src/routes/incoming_multi_server.rs`) is the same for both transports.

## Backend socket

The backend sends one request per connection to `peer-link.sock`: a JSON line
`{address, publicKey, method, path, headers, bodyLength, timeout}`, then the body. The answer
is a JSON line `{status, headers}`, then body parts (a 4-byte length and data) and a zero length
at the end. A failure is a JSON line `{failure, sent}`.

## Route and fallback

A signed response to a paired server carries `X-Studio-Link: ADDRESS:PORT` when the link runs.
The client keeps the latest address per server and uses the link for the next requests.
Orchestration and multi-server access management use the same QUIC-first signed-request
exchange and share this learned-route state.

- `sent: false`: the paired server cannot have run the request (no connection, a refused key,
  a failed send). The client sends the same signed request over HTTPS and does not try the
  link of that server for 60 seconds.
- `sent: true` without an answer: the outcome is unknown. The client reports
  `remote_unavailable` with "Keep the request ID", as for a lost HTTPS response. It never
  sends the request again by itself.

A TLS 1.3 client can send before the server checks the client key. A connection that the
server closes with a TLS error ran no request, so it counts as `sent: false`.

## Files

The Rust backend serves no export stream route. Files in `server-exports/` travel as chunk
answers to signed requests, over the link or HTTPS, and the whole-file checksum is checked:

- The update puller (`docs/peer-updates.md`) receives the app archive as `update.chunk` answers
  (`runtime-services/src/peer_update_pull.rs`).
- The Git fetch of a remote worker branch receives the bundle as `chunk` answers
  (`runtime-services/src/server_git.rs`).

## Events

Remote events travel as signed requests in the outbox of the sending server: agent events,
command exits and worker state. A queued request wakes the delivery at once, so an event takes
one exchange (10 ms over the link) instead of waiting for the 1-second scheduler tick. The tick
stays the fallback, and a wake that comes before its transaction commits checks the outbox once
more after 50 ms. Request identities, receipts and the lost-response rules do not change.

## Dial by key (iroh)

`studio-peer-link` dials a paired server by its key and the last address the backend learned
(`X-Studio-Link`). The address is a direct hint. iroh tries it, then the relays in `relays`, if any.
Hole punching needs a relay to exchange addresses. Without a relay, only the address hint and
the listen address of the bound socket are used.

Limit: the process binds the configured listen address (the Tailscale address). Other interfaces
are not bound, so a path through another network needs the listen address on that network. The
owner decides the bind address (backend change, not in this link).

Tests (`workspaces/runtime/apps/peer-link/src/tests.rs`): a direct connection by key on loopback; a
key outside `peers` is refused by the server and never dialed by the client; a reconnect after a
drop emits `connected` again; a connection through a local relay with no direct path (the test
relay runs in the test process, with its self-signed certificate as the only root).

Measured 2026-11-02 on the build Mac, loopback, release-free debug test build (`cargo test -p
studio-peer-link loopback_round_trip_time -- --nocapture`): one small request (new stream, four bytes
out, four bytes back), median of 200 requests: 0.42 ms. The QUIC link on Tailscale (2026-10-14) took
10 ms for the same request. That number includes the path, so it is not comparable.

## Not yet

- Reachability still comes from the discovery pass, which runs only on request. Design: the link process
  reports each paired connection when it opens and when it closes (keepalive 10 seconds, idle
  60 seconds). The backend marks a server reachable while a connection is open and leaves the
  unreachable state to discovery, because a closed connection proves only that no traffic ran.
- One long-lived event stream per server pair. The outbox already pushes events, so a stream
  would save only the per-request signature. It needs its own receipt per event to keep the
  lost-response rules; do it only when the event rate makes the per-request cost visible.
- The window reaches a paired server over HTTPS: the desktop main process signs and sends each
  request to the server's Serve origin, without the local backend. Design for
  WebTransport: the desktop main process (it holds the native privileges) opens a WebTransport
  session to the paired link with the same raw public key pinning, and the renderer keeps using
  the local HTTP API. The link accepts one more ALPN (`h3`) only for a session token that the
  paired backend issues to its own owner (the existing UI invitation), and maps each
  WebTransport stream to one request as today. The window keeps HTTPS as the fallback.

## Checks

Run `just test studio-peer-link` for the link process and
`just check studio-runtime-services` for runtime integration checks. The former
Python peer-link and update-stream harnesses were removed with the backend.
See [testing](testing.md) for the current gates and evidence boundaries.
