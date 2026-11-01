# Peer-to-peer project peering

- **Status:** Accepted by the owner, 2026-11-01 ("Давай p2p. Пейринг per project и zero trust").
- **Audience:** maintainers of the peer link, federation, multi-server and the team view.
- **Builds on:** [target client and server architecture](2026-10-28-target-client-server-architecture.md),
  [QUIC link between paired servers](../peer-link.md), [peer room federation](../peer-federation.md).

## In short

Two Studio servers of different owners connect directly, without a shared tailnet. A
connection is addressed by the server key, not by an address. Pairing grants access to one
project, not to the server. Every request is checked against that grant; the network path
gives no trust. The first use is the leads of one project talking as members of one team,
the same way a lead talks to its subagents.

## Context

- The peer link (`studio-peer-link`) already runs QUIC with raw Ed25519 public keys
  (`studio-peer/1`) between paired servers. Today it listens on a Tailscale address, and a
  Tailscale owner check runs on top.
- Peer federation already pairs servers of two users, approves rooms on both sides, signs
  messages and delivers them through the durable outbox with receipts.
- The owner and a second maintainer work on one project from two machines, on different
  networks. Their agents reach each other only through GitHub comments today. One answer
  waited 12 hours unread.

## Decision

### Transport: one key, any network

- Each server dials a peer by its public key. Connection setup tries a direct path first
  (UDP hole punching), then falls back to a relay. This is the model of iroh
  (n0.computer, Rust, MIT): QUIC endpoints addressed by Ed25519 keys, NAT traversal, relay
  fallback. Use iroh for discovery and dialing; keep `quinn` and the `studio-peer/1` ALPN.
- The relay forwards encrypted QUIC only. It cannot read or change a message. Start with a
  relay we run (for example on the existing gateway host); the public n0 relays are a
  fallback the owner can turn off.
- A Tailscale path, when present, is just one more direct path. Nothing depends on it.

### Zero trust

- The network path gives no rights. A request from a tailnet address or a LAN address is
  treated like one from the internet.
- Every connection is authenticated by the peer key (mutual TLS with raw public keys).
  Every request is signed (`studio-multi-server-v1`) and replay-protected, as today.
- Every request is authorized by the project grant of that key (below), at the edge, as a
  `Principal`. Unknown key, revoked key, or a project outside the grant: refused before any
  handler runs.
- Least privilege: a grant lists capabilities explicitly. Nothing is implied.
- Scope of this decision: peers. The owner's own devices and the loopback desktop keep the
  current rules (device keys and the shared trust list). Moving them to zero trust is a
  separate decision.

### Pairing per project

- A pairing joins one local project and one remote project. A server can have many
  pairings with one peer, one per project.
- Invitation: the owner of the project creates it in the project settings. It holds the
  server key, the project id, a one-use secret and an expiry (15 minutes). It travels out
  of band (message, QR). The other owner accepts it and picks the local project. Both
  owners see and confirm the peer key fingerprint and the granted capabilities.
- Grant, stored on each side for its own project:

  ```rust
  struct ProjectGrant {
      peer: ServerKey,
      local_project: ProjectId,
      remote_project: ProjectId,
      capabilities: BTreeSet<Capability>,
      revoked_at: Option<Timestamp>,
  }
  enum Capability {
      LeadMessages,   // the paired leads send each other messages
      // later, each only when an owner adds it: ReadProjectState, ProposeChange, ...
  }
  ```

- The first version has one capability, `LeadMessages`.
- A revocation is final and local. It closes the connections of that grant and drops its
  queued outbound messages. Pairing again makes a new grant.
- `Principal` gains `ProjectPeer { peer, project, capabilities }`. The permission table
  gets one row: `LeadMessages` allows sending a message to the paired lead of that project
  and nothing else. A peer cannot spawn, stop, assign work, read files or read other
  projects.

### Leads in one team

- Each project names its lead chat. After pairing, each lead sees the other lead as a
  member of its team, with the peer label. Subagents do not join; they talk to their own
  lead.
- The lead uses the same message tool it uses for its subagents. A message to the peer lead
  goes into the outbox (`federation_message`, stable id, signed), is delivered over the
  peer connection, and wakes the receiving lead like a subagent report wakes its lead.
- Received text is untrusted input. It cannot run commands; the receiving lead decides.

## Not in this decision

- Sharing project state, files or layr lines between peers.
- Zero trust for the owner's own devices.
- Group chats with more than two servers per project (one pairing per pair for now).

## Open points

- Relay host and its operation (where, who runs it, rate limits).
- Whether the existing federation rooms move onto project grants or stay as they are.
- Lead chat identity: the lead of a project must run as a Studio chat to join the team.
