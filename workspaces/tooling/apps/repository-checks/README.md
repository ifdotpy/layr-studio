# Repository checks

## Change Contract

This app owns checks for staged repository content and the checker invoked by
the pre-commit hook. Its public interface is the checker command used by
`.githooks/`; it may inspect repository files but application code must not
depend on it at runtime. Preserve Git-index-only staged checks and never rewrite
the index or working tree. Configuration and commands are owned by the root
manifest and Oxlint/Oxfmt configuration. Focused check:
`pnpm run test:pre-commit` from the repository root, as defined by the
[root manifest](../../../../package.json).

The compiler and Rust lint policy tests also run negative contract witnesses.
They check that the root Oxlint policy rejects type erasure and unchecked
compiler suppressions, and that native Rust/Clippy rejects unhandled additions
and wildcard patterns that would hide them. The Rust witness copies the owning
root manifest and Clippy settings into a disposable dependency-free crate; it
does not build the application or interpret Rust source itself. Run the focused
witnesses with
`node --test workspaces/tooling/apps/repository-checks/tests/compiler-policy.test.ts workspaces/tooling/apps/repository-checks/tests/rust-lint-policy.test.ts`.

## Provider updates

`just provider-update codex <exact-version>` requires that CLI on `CODEX_BIN`
(or `PATH`) and prints candidate schema SHA and vendor response-source provenance
for review. Rerun with `--reviewed-sha <sha>` to update the contract pin and
response links, refresh, generate, check drift, and compile with the Clippy ratchet.
`just provider-update claude <exact-sdk-version>` updates the bridge manifest's
single SDK pin, runs `pnpm install --ignore-scripts`, then generation, freshness,
SDK declaration drift and bridge compilation. New methods and SDK members still
require explicit decisions in their existing disposition owners. Both paths
report changed files on failure; inspect those files before rerunning. CLI
installation, runtime version-notice baselines, delivery and live compatibility
validation remain separate operator actions.

## Syntax guards and ratchet

The ast-grep rules under `ast-grep/rules` detect direct JSON round trips,
selected JSON-indexed dispatch and serde defaults, parse casts, casts through
unknown, and casts to any. Rules contain the corrective message. They operate
on syntax alone: they do not resolve types, aliases, or prove that other casts,
JSON access, or switch statements are safe. Type-aware checks own those cases.

`ratchet-baseline.json` records each rule's existing matches by repository path
and a SHA-256 fingerprint of whitespace-normalized matched text plus its
occurrence index within that file. `pnpm run check:ast-grep` fails on new matches
and on stale baseline entries. After fixing violations, run
`node workspaces/tooling/apps/repository-checks/ast-grep.ts --shrink` to remove
only stale entries; it cannot add new matches. Review the resulting diff. The
scan runs in `lint:all` and the short tooling tier. It is not in the staged
hook: the full baseline must be checked against a consistent repository snapshot,
while the hook checks index content only.

The root [Cargo lint policy](../../../../Cargo.toml) also rejects boolean-heavy
function signatures, boolean matches, oversized enum variants, and references
to small Copy values. Its native witnesses verify each rule; callbacks required
by external APIs may document a narrow exception.

## Deferred Clippy lints

`just clippy-runtime` runs the workspace Clippy gate excluding the native client,
which requires a built web bundle, and checks deferred lint findings against the
shared ratchet baseline. Hosted Clippy also checks the native client. The Value
conversion restrictions apply to runtime services and store; generated HTTP types are allowed at their
module because the schema determines their shape. After fixing findings, run
`node workspaces/tooling/apps/repository-checks/clippy-ratchet.ts --shrink --exclude-native` and
review the baseline diff. This command removes fixed entries but cannot add new
ones. Fingerprints use the lint and whitespace-normalized diagnostic span, with
an occurrence index per file; changing a span is treated as a new finding.

## React glue boundary

`react-glue.ts` owns the five forbidden React hooks and syntax-only listener-set
check for web production source. Tests, stories and fixtures are excluded.
`react-glue-exceptions.json` is the sorted, one-entry-per-line exception owner,
seeded from main 6a386746. Entries marked `pending E1–E5` are migration debt;
after conversion only one-line DOM integration reasons may remain. Remove an
entry when its final forbidden API disappears. Then regenerate the two Oxlint
overrides with `node workspaces/tooling/apps/repository-checks/react-glue.ts`
and format `.oxlintrc.json`. Generation preserves the network boundary.

The network-boundary tests check the manifest ceiling, stale entries, new files,
generated-config parity and negative import/listener witnesses in `test-fast`.
The staged hook reads the complete source and manifest from the index and checks
listener sets before running Oxlint. The TypeScript syntax check rejects literal
`Set<() => void>` and named listener/subscriber/observer/callback sets in files
with subscribe/notify implementations. It does not resolve callback type aliases
or follow custom collection factories.

`react-hook-ceilings.json` owns per-file call ceilings for `useMemo`,
`useCallback` and `useRef`, seeded from main `44bbf5237ecb947620f44d2bb3de8f6fca98c770`.
New files have zero allowance for these three hooks. The existing TypeScript parser
counts named React imports (including named default aliases), default and
namespace member calls, and namespace destructuring. It matches identifiers by
spelling, so a shadowed hook name can produce a conservative false positive.
Keep trivial local UI state in React `useState`; move async/derived/shared logic
into MobX stores. `useState` has no ceiling. The sanctioned
`src/stores/Owned.tsx` class boundary uses no hooks and passes these checks. DOM refs remain within the recorded ceilings.

After removing hooks, lower ceilings with
`node workspaces/tooling/apps/repository-checks/react-glue.ts --update`.
This command refuses any increase before writing. Use `--check` for a read-only
count check. `lint:all`, staged checks and the short tooling tier enforce counts;
all entry points also reject increases relative to the merge base with main
(or the previous commit when checking main). Before the manifests existed,
allowances are derived with the same scanners from the pinned seed source
revision above. Historical comparisons require Git history. Keep the
existing no-argument command for regenerating the Oxlint glue overrides.

The `web-*` ast-grep rules reject `runInAction` calls, `Set<() => void>` listener
sets, `makeObservable` calls and effects used only to start/load/subscribe/dispose
a store. Prefer generator flows and owner-managed lifetimes. They share the
production-source exclusions of `react-glue.ts`. Existing main matches alone
are grandfathered in `ratchet-baseline.json`, including computed-only sidebar
facades. The exact generated owner `src/generated/observableResources.ts` is
exempt from the `makeObservable` rule because its generator owns explicit
annotations. Other rules still cover that file. The lifetime-effect rule uses
receivers containing store/model/owner in their name to preserve DOM integration
such as `video.load()`. Staged scans read their configuration and rules from the
index. Both staged and full scans reject baseline additions relative to the base.
Shrink this existing baseline
with `node workspaces/tooling/apps/repository-checks/ast-grep.ts --shrink`.
The rules inspect syntax and do not resolve renamed MobX imports or callback
indirection. Ast-grep tests cover TypeScript and TSX forms in `ast-grep/tests`.

`web-no-ad-hoc-operation-state` uses the existing TypeScript syntax parser and
the same fingerprint ratchet. `operation-state.ts` names the migrated owners;
new production modules importing `RequestRegistry` or `requestOperations` also
join the check. Writable operation fields, operation-named state/ref hooks,
direct mutation transport, mutation UUID/stable-ID generation, request-outbox
writes, `runInAction` and reload/refetch calls are rejected. Import aliases,
member aliases and destructuring are followed by spelling, without resolving
types or arbitrary callback indirection. Registry projections, validation,
read loading and navigation IDs remain local concerns.

The exact-path exceptions live in `ratchet-baseline.json`, each with a protocol
reason. Four transport calls are initially allowed: the rename and project-location
adapters and two unmigrated AppUi operations. Their fingerprints can only shrink;
adding another call in an allowed file still fails. The immutable migration seed
is main `4f44f7fc`, used only when history predates this rule. Registry internals
own identity and lifecycle and are outside the migrated-consumer scope. Other
specialized protocol owners join only when migrated. Run the AST witnesses with
`node --test workspaces/tooling/apps/repository-checks/tests/ast-grep.test.ts`.
