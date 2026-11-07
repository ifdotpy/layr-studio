# Account-cost rollout decoding

Base: `0f148112a50f0e0ac85638350bf40a450d189e15`.

## Read-only live evidence

The supplied `studio-3507.perf.data` caller report attributes 4.33% of all
samples at `SliceRead::skip_to_escape` and 4.02% at `StrRead::parse_str` to
`usage::scan_codex_sessions_with_catalog -> RolloutRow::parse -> from_str<Value>`.
The scanner decodes response items, compactions and event messages before
discarding them. These bodies are not inputs to the account-cost report.

A fresh 30-second recording at 199 Hz on 2026-11-07 15:43 UTC captured 3,805
samples with no lost samples. The workload differs from the earlier recording:
agent preparation and writes dominate the fresh sample. Visible SQLite JSON
translation callers include `sync_policy::mark_agent_projection ->
sync_entity_context::team_counts -> jsonExtractFunc`. Other JSON translation
stacks end in Tokio's blocking pool, so their SQL caller is inconclusive.

The read-only `/api/perf-reporting` snapshot reports 868,203 `Value` decodes,
3,069,097,573 input bytes and 7,620,727,041 ns total decode time. Of these,
510,065 decodes and 1,744,509,994 ns are outside store jobs. The largest labelled
`Value` read job is `runtime-http/routes/process_controls` (4,155 decodes,
2,184,014,293 ns). These are cumulative measurements, not isolated timings for
this change. The oldest profile is stronger evidence for the scanner caller
than aggregate type counters alone.

Local artifacts are under `/home/alex/.cache/codex-studio/tmp/`:
`value-hotpath-fresh.perf.data`, `value-hotpath-fresh.report`,
`value-hotpath-old.report`, and `value-hotpath-report.json`.

## Scope

Filter unused rollout kinds before decoding bodies into `Value`. Retain the
existing translator and cost aggregation for accepted rows. Published API and
SSE contracts are unchanged. This task does not change SQLite projections.

No backend was started, delivered or restarted, and no synthetic load was run.
An unchanged live process cannot measure the effect of the source change;
deterministic fixture decode counts provide the before/after evidence.

## Deterministic witness

`usage::tests::codex_cost_scan_skips_unused_value_decodes` invokes the real
file scanner on four unused rollout kinds. One-time schema initialization is
outside the measured job. Before the change, the assertion fails with four
`Value` decodes; after the change it passes with zero. The borrowed raw heads
still validate JSON syntax. Accepted rows incur a head scan before the existing
full decode; the saving comes from avoiding materialization of unused bodies.

The perf-reporting focused `cargo test --workspace --exclude studio-native-client
-j 4 --features studio-runtime-server/perf-reporting --lib codex_` run passed:
45 services tests passed, one pre-existing heavy-tier test was ignored. This
includes cost pricing, forks, replay timestamps, token-count-only exclusion,
and head compatibility (duplicate keys, escaped kind names, missing/null/non-text
kinds, and malformed JSON). The compatibility witness is selected by the
existing fast-tier `codex_session_fixture_` filter.

## Initial validation on the required base

- `just test-fast`: passed in 546.869 seconds on 2026-11-07; full log:
  `/home/alex/.cache/codex-studio/checks/run-1794066842874-oGGSR8/full.log`.
- Strict workspace/all-targets Clippy, default and
  `studio-runtime-server/perf-reporting`: passed with the three explicitly
  authorized base allowances (`large_futures`, `wildcard_enum_match_arm`,
  `items_after_test_module`). Direct Cargo commands used `-j 4`.
- Rust formatting, Markdown formatting and `git diff --check`: passed.

The initial no-third-allowance Clippy runs both failed on the unchanged
`runtime-schema/src/resource_ref.rs` helper after its test module. The separate
main Clippy correction owns that fix.

## Final validation on corrected main

Replayed onto `784635b19e04d5554f677077f64846691e6bfcd9`. Comparing the source
patch before and after replay with `cmp` passed; both changed Rust files also
retained identical SHA-256 hashes. The earlier fast-gate pass stands, as
explicitly directed: replay changed none of this task's files.

On 2026-11-07 at 16:18 UTC, all sequential checks passed without allowances:

- Clippy ratchet, default and perf-reporting configurations.
- Strict workspace/all-targets Clippy (`-D warnings`), default and
  `studio-runtime-server/perf-reporting` configurations.
- Focused `usage::tests::codex_` tests with perf reporting enabled, including
  the zero-`Value`-decode scanner witness.

Logs: `/home/alex/.cache/codex-studio/tmp/value-hotpath-final-` followed by
`ratchet-default.log`, `ratchet-perf.log`, `clippy-default.log`,
`clippy-perf.log`, and `focused.log`. Direct Cargo commands and the ratchet
used four build jobs. No ratchet baseline was changed.
