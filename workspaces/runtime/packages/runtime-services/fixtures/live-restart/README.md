# Live restart fixture

Regenerate explicitly from a database supplied as the required argument (read-only):

```sh
TMPDIR=/path/to/private/scratch node workspaces/runtime/packages/runtime-services/fixtures/live-restart/export.ts /path/to/canvas.sqlite3
node_modules/.bin/oxfmt workspaces/runtime/packages/runtime-services/fixtures/live-restart/rows.json
```

Use an existing private scratch folder with room for a database copy.
The exporter uses the SQLite backup API with a read-only source connection, reads
only the temporary copy, and removes that private copy in `finally`. It never
starts a server. The committed plain JSON stays below 100 KB and contains a bounded selection of
transfer, monitor, tool-owner, outbox, queue, approval, account-state and execution rows,
plus the agent owners those rows reference. Historical rows supplement any
currently unsettled rows; this is a storage-shape witness, not an archived incident
snapshot. The exact failing transfer proposal from November 11 was refused before
persistence. Its regression variant copies the captured member `settings`
(including the raw `workerDefaults` object) into `targetSettings` and marks the
operation pending before the real startup barrier. This recreates the flatten/raw
decoder boundary without inventing worker-default fields.

Identifiers and paths become stable placeholders; repeated identities keep the
same placeholder. Free text, prompts, command text, file contents, names,
signatures, tokens and credentials become `redacted`.
Protocol field names and enum literals are retained. JSON inside TEXT columns
stays TEXT; nulls, numbers, booleans and nesting are retained. Long arrays and the
dynamic `layrTurnSaves` history map retain at most two representative elements. Two-element tuples keep both elements. The fixture
loader rejects home paths, account markers, credential/key patterns, hostnames,
real UUIDs and long free text, including inside JSON TEXT columns. Regeneration
must be reviewed before saving the fixture.

`live_restart_rows_open_account_worker_and_restore_callbacks_without_retry` loads
rows into a fresh `RuntimeStore`, invokes startup callbacks and monitors, opens
the real account worker, and checks repeated wake barriers. The server's
`live_restart_monitor_status_receipt_timeout_finishes_recovery_once` invokes the
real `RetainedMonitorPort` and `MonitorRuntime` with the captured operation,
reactivating its lifecycle to represent an unanswered command at restart. Its
session waits through the status-query deadline and returns Unknown. Two ticks
must yield one request, a durable lost outcome and one exit notice.

Callback and response hashes derived from anonymized identities are recomputed
by the loader using the existing runtime identity owner. No dispatcher or native
process is started while seeding.

Run the focused witnesses with workspace feature resolution and four build jobs:

```sh
cargo nextest run -j 4 --workspace --exclude studio-native-client --profile fast -E 'test(live_restart)'
```

No live model, command execution or live server is required. Synthetic session
replies supply only the unavailable provider boundary; startup decisions and
persistent reads and writes use the real callers.

`public-contract.json` owns the reviewed public field names, wire methods and
domain literals for this corpus. Agent keys include the public field list declared
by the typed AgentRecord decoder, including optional fields absent from a sample.
Both the exporter and privacy witness consume
it. Dynamic map keys and unknown fields/methods always become stable placeholders,
even when they look like identifiers. Contract vocabulary changes require review;
regeneration never learns an allowlist from live rows.
