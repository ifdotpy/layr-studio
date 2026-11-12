# Providers workspace

Owns external model-provider process integrations used by Studio.

See [Type contracts](../../docs/type-contracts.md) for cross-language typing
rules and current migration coverage.

## Provider extensibility

Shared application code works with provider-neutral contracts. Provider identity
has one Rust owner, [`ProviderKind`](../runtime/packages/runtime-schema/src/provider_kind.rs).
Rust consumers import that enum; TypeScript and schema consumers use its generated
contract. Do not maintain a second provider enum, union, or list in a consumer.

Parse external provider names at the boundary. Missing, unknown, or malformed
names must remain unsupported, never select an existing provider by default.
Convert historical representations in a dated storage migration before normal
readers run; do not retain compatibility defaults in the identity owner or callers.
Follow the [runtime migration policy](../../docs/runtime-migrations.md) for the
single migration owner, dates, supported sources, rollback, and removal rules.

Provider dispatch must be exhaustive: use an enum `match` or a typed mapping that
requires every supported provider. Do not use `if provider == "codex" ... else ...`,
a wildcard arm, or a fallback that gives a new provider another provider's behavior.
Keep native SDK, credentials, transcript formats, and provider-specific semantics
in their owning adapters. Shared callers consume normalized values or the
adapter's explicit capabilities.

Each provider decides every user control through an exhaustive match in
`ProviderKind::capability`, returning `Supported` or `Unsupported { reason }`.
`ProviderKind::capabilities` derives the generated UI list from those decisions:

steer, rewind, fork, model change, effort, compaction, approvals. The UI shows
only the controls in that list, through the generated `providerSupports`. Do not
test for a provider name to show or hide a control. Internal operations and
runtime policies have their own exhaustive owner, `ProviderFeature::support`;
they are not additional UI controls. Implemented command arms establish command
support without a second runtime capability check.

`edit(turn)` serves every provider: the server hides the items after the turn,
snapshots and restores the files of the turn, and forks the provider thread
through `ProviderAdapter::encode(ProviderCommand::EditThread)`
(`runtime-services/src/edit_turn.rs`). The bridge keeps no rollback of its own.

To add a provider, extend the identity owner and its adapter, regenerate contracts,
and implement the cases exposed by exhaustive checks. Verify the new boundary
alongside existing providers and unknown input. Reuse the existing adapter and
generation mechanisms; provider support does not require a new framework.

## Apps and navigation

- [`apps/claude-bridge/`](apps/claude-bridge): the Node Claude Agent SDK
  process; see its [Change Contract](apps/claude-bridge/README.md) and
  [`package.json`](apps/claude-bridge/package.json).

There are no provider packages yet. Runtime and desktop apps call the bridge;
the bridge does not import either app. Keep provider credentials and native
session behavior with the provider integration that owns them.

For one hypothesis, use `just test studio-claude-bridge [case]`; for a bridge
change, use `just check studio-claude-bridge`. The root
[`justfile`](../../justfile) discovers packages from manifests; the bridge
manifest owns test execution and discovery. The pre-PR server/client gate is
documented in [docs/testing.md](../../docs/testing.md).
