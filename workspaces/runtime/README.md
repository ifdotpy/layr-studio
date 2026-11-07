# Runtime workspace

## Change contract

Owns the Rust backend, process supervisor, and runtime prompts.
The server owns SQLite and orchestration; the renderer accesses it over HTTP.
Native privileges stay in the isolated desktop host. Queue revisions are opaque
server-local tokens and must only be compared within the same server.

Follow [Type contracts](../../docs/type-contracts.md) for shared contract
ownership, boundary validation, and current migration coverage.

## Apps and packages

- [`apps/runtime-server/`](apps/runtime-server/): Rust HTTP server and operator CLI.
- [`packages/runtime-services/`](packages/runtime-services/): runtime operations and integration tests.
- [`packages/runtime-store/`](packages/runtime-store/): SQLite state ownership.
- [`packages/read-models/`](packages/read-models/): pure render-ready domain projections.
- [`packages/runtime-schema/`](packages/runtime-schema/): generated API and state contracts.

Use `just packages` to discover manifest-owned packages. For one hypothesis,
use `just test studio-runtime-services <test-name>`; for package checks, use
`just check studio-runtime-services`. `just test-plan` prints the native command
without executing it. See [testing](../../docs/testing.md) for the local gates.

Desktop helpers retain Python. The removed Python backend,
Python oracle runners have no compatibility aliases.
