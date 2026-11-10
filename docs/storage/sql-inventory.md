# SQL inventory and consolidation ownership

`workspaces/tooling/apps/repository-checks/sql-ratchet.ts` owns the SQL API list,
source scanner and file ownership rules. Its generated Clippy configuration is
checked for parity in `clippy-ratchet.test.ts`. The fast gate runs
`sql-ratchet.test.ts`, including the actual repository scan.

Generate the source inventory and the full, exclusive file-owner manifest:

```sh
node workspaces/tooling/apps/repository-checks/sql-ratchet.ts \
  --inventory /tmp/issue-edit/p0-inventory.json \
  --owners /tmp/issue-edit/p0-owner-manifest.json
```

Add compiler-resolved Rust calls to the inventory with the existing Clippy gate:

```sh
node workspaces/tooling/apps/repository-checks/clippy-ratchet.ts \
  --exclude-native --sql-inventory /tmp/issue-edit/p0-inventory.json
```

This uses workspace all-targets resolution, including tests, method aliases,
Transaction dereferencing and expanded macros. Native-only targets and inactive
conditional branches remain structural evidence on this host. The diagnostics
are saved beside the inventory as `.diagnostics.ndjson`. Neither command opens
an application database or starts a server.

Each site names its file, enclosing symbol, API, source byte span, table evidence
and byte-exact statement fingerprint (including SQL literal whitespace). Rust syntax trees identify call expressions, SQL
literals and persistence boundary imports. TypeScript syntax trees identify SQL
strings and templates, including embedded Python. Python uses syntax trees too.
SQL fixture files are complete frozen batches. Compiler records are explicitly
marked `compiler`; unresolved method-name candidates remain `structural` and
are not reported as proven SQL calls. Dynamic table evidence is explicitly
`<dynamic>`. Descriptor references supply table evidence without requiring a
literal table name at every typed call. This is a conversion inventory, not a
proof that arbitrary computed strings resolve to a particular table.

Ownership follows the consolidation F1–F8 grouping of the source-to-target
manifest in `map.md`, descriptor references and its caller ledger. Shared files
and diagnostic entry points belong to `F1-shared`. Files with no static family
evidence stay with F1 for integration and extraction. Sole StoredText codec and
Supervisor owners retain their exclusive scopes. The manifest includes every
tracked or nonignored source file once; generated outputs and unrelated files
also receive an integration owner. Review generic/dynamic callers with F1
before assigning exclusive family edits.

`sql-baseline.json` freezes file, API and fingerprint with multiplicity. It
is generated as sorted file → API → fingerprint arrays, one physical line per
file; Oxfmt ignores this generated artifact. Fingerprints are the first 16 hex
characters (64 bits) of SHA-256 over the symbol, a NUL separator and exact
statement bytes. Identical SQL moved between symbols receives a different
fingerprint; symbols remain absent from the JSON. Duplicate array entries retain their
separate allowance budgets. Symbol and family evidence stay in the inventory
and owner manifest rather than repeating in the baseline. New fingerprints, file paths, APIs and excess duplicate calls fail. Removals are permitted;
`sql-ratchet.ts --shrink` removes obsolete source allowances while retaining
compiler allowances. Infrastructure additions require explicit reviewed
baseline entries. Only db-api and its derive generator implement SQL without a
legacy allowance. Typed query macros are recorded as typed caller edges.
Conservative source candidates can include unrelated methods with the same
name; their compiler records distinguish the actual API. New such candidates
need review rather than an implicit permission for an entire application file.

Generated-query constructor spellings are a separate unconditional text rule:
only their exact definition module in db-api may contain them. Comments,
fixtures, documentation and generator source are checked too. The legacy
baseline cannot waive that boundary.

The source tier is deliberately conservative rather than a Rust resolver.
The mandatory landing command is `clippy-ratchet.ts --exclude-native`; its SQL
check is unconditional. It must observe the two compiled alias/macro witnesses
in the db-api test target, or it fails even if no new caller was reported.
TypeScript and JavaScript SQLite capabilities are frozen by exact file path.
Any literal equal to a supported driver specifier marks the module, covering
imports (including type-only and dynamic imports), re-exports and requires.
Computed method access and receiver names do not affect this check. Python
execution calls remain inventoried independently of their SQL argument shape.

The fast gate already caches this step through `test-result-cache.ts`: its
content key hashes every tracked or nonignored input file, the command and
shared tooling inputs. Unchanged source trees reuse a passing scan, including
across Layr lines. No second persistent cache is needed. The uncached scan uses
per-file symbol indexes; its measured cost is reported in the task evidence.

One-time migration audit, completed 2026-11-10 at 18:03 UTC:

```text
Legacy entries: 21188
Exact symbol-and-byte checks: 21188
Unverified legacy entries: 0
Legacy entries mapped one-to-one: 21188 / 21188
Unmapped entries: 0
Added entries: 8
Compiler-resolved sites: 6448
```

The original baseline at Git `b64c63792c57ad4650f66ca653c19080c25cf5f4`
was paired by file, symbol and API with the exact-byte baseline at Git
`12e295670d36b1711eb3184c099d9ef3d66e1ec9`. Every selected statement's
original symbol and exact bytes were verified against that snapshot before
applying the same symbol + NUL + exact-byte fingerprint as production. The
moved-allowance regression failed before the audit fix and then reported
`mapped=0`, `unmapped=1`, `added=1` as required.

All additions from that recorded run:

| File                                                                     | API                         | Fingerprint        |
| ------------------------------------------------------------------------ | --------------------------- | ------------------ |
| `workspaces/client/apps/web/scripts/verify-rust-chat-scale.ts`           | `typescript::sqlite-driver` | `98325e2380e80fda` |
| `workspaces/client/apps/web/src/scaleVerifierHelpers.test.ts`            | `typescript::sqlite-driver` | `98325e2380e80fda` |
| `workspaces/client/apps/web/tests/outbox/remove-sending-browser.spec.ts` | `typescript::sqlite-driver` | `98325e2380e80fda` |
| `workspaces/client/apps/web/tests/playwright.ts`                         | `typescript::sqlite-driver` | `98325e2380e80fda` |
| `workspaces/runtime/packages/json/fixtures/extract.ts`                   | `typescript::sqlite-driver` | `98325e2380e80fda` |
| `workspaces/tooling/apps/repository-checks/sql-ratchet.ts`               | `typescript::sqlite-driver` | `98325e2380e80fda` |
| `workspaces/tooling/apps/repository-checks/tests/sql-ratchet.test.ts`    | `embedded::sql`             | `b46b67c8109c9d1d` |
| `workspaces/tooling/apps/repository-checks/tests/sql-ratchet.test.ts`    | `embedded::sql`             | `46d6fc553e87091e` |

The migration-only reader, audit helper, CLI option and audit-specific tests
were deleted after this run. They have no continuing application use once the
legacy baseline is retired. Production symbol-binding, moved-call,
multiplicity, format and size regression tests remain.

Measure a cold source scan with this exact command from the repository root:

```sh
node --input-type=module -e 'import {scanSql} from "./workspaces/tooling/apps/repository-checks/sql-ratchet.ts"; const begin=performance.now(); const scan=scanSql(); console.log(JSON.stringify({milliseconds:performance.now()-begin,sites:scan.sites.length,files:scan.manifest.length}));'
```

The Clippy inventory command prints the measured resolved compiler site count.

Measured 2026-11-10 on the P0-A tree based on main `3ee53b01`: the commands
above reported **6,448 resolved compiler sites** and **11,498.866783 ms** for a
cold source scan (**15,128 sites**, **4,045 files**). The migration audit mapped
**21,188 / 21,188** legacy entries with zero unmapped entries and listed all
eight additions (six exact file capabilities and two literal-space fixtures).

The compact baseline retains all **21,196 allowance counts** in **606,034
bytes**, with **1,087 file groups** (1,089 physical lines including braces),
compared with 7,667,552 bytes / 148,374 lines in `f70bb179`. Symbol and reason
fields are absent from this generated file; the inventory and manifest retain
caller and ownership evidence. Enforcement uses symbol-bound 16-hex fingerprints and preserves duplicate
counts. The verified one-time migration result is recorded above; its code
was removed.
