# studio-observable-codegen

## Change Contract

- **Responsibility:** Pure generation of observable TypeScript models and resource facades from Rust metadata and OpenAPI components.
- **Public interface:** `ObservableModel::of<T>`, `ObservableResource::of<T>`, `ObservableArg`, `render`, and typed `GenerationError` diagnostics.
- **Boundaries:** No runtime, HTTP, persistence, filesystem, or process dependencies. `studio-contracts` registers owners, formats output and checks drift.
- **Invariants:** Only reachable closed objects, scalar leaves, nullable values, arrays and string-tagged object unions are supported. Other shapes fail with their schema path. Identity and ordering fields must be required strings; opaque fields must exist. Ordered collections use binary insertion by the server-owned key. Public data derives from generated DTOs and is deeply readonly; mutable models stay private.
- **Configuration owner:** Rust registration in `runtime-schema/src/observable_resources.rs`. Branded arguments are exercised by a fixture only; production currently exposes the singleton sidebar. Fixture-only argument facades retain their per-key slots; disposal support waits for a production resource with arguments. The production singleton drops its graph through the existing subscription cache eviction callback.
- **Focused checks:** `cargo test -j 4 -p studio-observable-codegen`; TypeScript generator witnesses in repository-checks.
