# Performance reporting

Performance reporting is an opt-in build feature. The default server and web builds omit the instrumentation. It keeps aggregates in memory and does not add database state.

The server records normalized SQL statement counts, total and maximum duration, and result rows; writer queue and admission waits; selected shared mutex waits; HTTP route duration; SSE event and uncompressed byte counts and change-to-frame latency; and labelled Tokio polls over 10 ms. Startup and operator connections are not profiled. SQLite profile durations reflect SQLite's clock resolution. SSE change-to-frame measures from publication until a frame is produced, not network delivery to the browser. Poll labels cover the explicitly instrumented tasks.

Every minute the server logs up to 20 entries per table, ordered by total duration. `GET /api/perf-reporting` returns the current aggregates and most recent client report. Normal API permission checks apply. The web client reports long tasks, event timing and composer/history render counts and durations once per minute to `POST /api/perf-reporting/client`; it does not poll the endpoint.

SQL parameters are never expanded. Quoted strings, blob and numeric literals are replaced before aggregation, including JSON paths. Each aggregate table has a fixed key and key-length limit; additional labels share an overflow bucket. Statements on instrumented connections during startup, including any migrations using those connections, count toward the limit. Client reports contain timing and fixed labels, not message text. These diagnostics are intended for a trusted operator: they reveal application query structure and activity patterns.

The retained key bytes are bounded to approximately 8.5 MB per table, or 51 MB across the six server aggregate tables, excluding map and value overhead.

For a local build, run `cargo build --release -p studio-runtime-server --features perf-reporting` and `STUDIO_PERF_REPORTING=1 pnpm --filter codex-agents-web run build`. On a host using `just deliver-server`, deliver an authorized commit with `STUDIO_PERF_REPORTING=1 just deliver-server <commit>`; the delivery builds both halves and keeps its feature-enabled release cache separate. This variable affects building, not the runtime environment. The command can restart the live service, so follow the normal delivery procedure.

To turn reporting off, deliver a normal build without `STUDIO_PERF_REPORTING=1`, or rebuild without the Cargo feature and web build variable. Restarting clears the in-memory aggregates.

## Client

Build the web renderer with `STUDIO_PERF_REPORTING=1` to enable client diagnostics. It aggregates browser long tasks, event duration and input delay by event type, and React render duration for the composer and history; every 60 seconds a nonempty window is sent to the server. Reports contain timings and counts, not message text or other user content. The default build excludes this instrumentation. The feature-on Vite plugin injects the authenticated request accessor and reporter mounts at exact source anchors; a changed or ambiguous anchor fails the build.

The flag also selects MobX's development ESM build, enabling `spy`, `trace`, invariant diagnostics and devtools information while other dependencies keep their production builds. The perf root configures MobX once with `enforceActions: "observed"`, `computedRequiresReaction`, `reactionRequiresObservable` and `observableRequiresReaction`. These strict checks warn in the console without rejecting updates or reads. MobX diagnostics stay local and add no network traffic. The default build keeps production MobX and omits this configuration.

## Additional server measurements

At the end of each labelled read job and writer transaction, SQLite's still-prepared statements are swept for full-scan steps, virtual-machine steps, sort operations and automatic-index rows. `sql_by_job` groups these counters and SQL profile timings by read label or writer source file and line, followed by normalized SQL. `sql_sweeps` reports the number of jobs (`count`) and prepared statements visited (`rows`) per label. Statements finalized or evicted from the statement cache before the sweep are not covered; neither are direct connections outside these job boundaries.

`GET /api/perf-reporting` includes `sql_top_time` and `sql_top_fullscan`, each containing up to 20 job/statement entries in descending order by total duration or full-scan steps. The periodic server log prints both rankings as well. `waits` includes separate writer admission, actor queue and writer run times, plus write call-site and labelled read-wrapper durations. `json` counts successful and failed calls to the Studio JSON facade by entry point and Rust type; nested facade calls count once. Code that uses another JSON API bypasses these counters.

### Decode cost

`json_decode_top_time` ranks Rust types by total facade decode time; `json_decode_by_job_top_time` ranks type and store-job origin pairs. Each entry reports `count`, `total_ns` and `bytes`, and each ranking contains up to 20 entries. For example, a costly `AgentRecord` entry identifies the type; the job ranking can then identify writer file and line or labelled read jobs responsible for its decoding. Calls outside a store job use `<outside store job>`. Counts include failures and exclude nested facade calls. Bytes are input lengths for string and slice decodes; value and reader decodes contribute zero bytes because their input size is unavailable. The bounded table shares an `<other>` bucket when its key limit is reached or a label is too long.
