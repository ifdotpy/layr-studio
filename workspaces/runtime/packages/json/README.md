# Studio JSON

The default feature uses serde_json. `sonic-backend` and `simd-backend` are
experimental: select exactly one feature with `--no-default-features`. The
simd backend uses a thread-local mutable buffer only for `from_str_owned` and
`from_slice_owned`; borrowed entry points use serde_json. All features keep
serde_json's `Value`, `Map`, `Number`, and `json!` types. `RawJson` and
`OwnedRawJson` are the facade's borrowed and owned raw fields; serde_json's
`value::RawValue` remains re-exported for compatibility during migration.

The alternative parsers are not interchangeable with serde_json. Sonic rejects
serde_json `RawValue`, loses the sign of `-0.0`, accepts nesting that serde_json
rejects, and `to_value` exposes sonic's private marker for `OwnedRawJson`.
The facade's sonic raw fields preserve bytes in text and writer round trips;
the owned type reparses its stored string when serialized. Simd rejects
`RawValue`, serializes raw wrappers as private-marker objects, and rejects a
value beyond `u64::MAX` that serde_json represents as a float.
Its owned parser holds a thread-local scratch borrow during deserialization;
a nested owned-parser call on the same thread panics.
These differences affect existing records, so neither is eligible as default.
The `Deserializer`, `Serializer`, and `value` re-exports retain their native
serde_json error types; facade functions return the facade's `Error`.

`fixtures/extract.ts` takes a private read-only database backup and emits the
fixtures. It retains an explicit allowlist of schema keys; all other keys and
all string values are replaced, numeric digits are masked, and byte lengths,
escaping and Unicode widths are preserved. Review its allowlist and generated
data before sharing. No copied database should be committed.
`fixtures/extract-provider.ts` takes a directory of private copies of Codex and
Claude session JSONL files, selects completed-item, tool-call, message, and
usage records, and applies the same sanitizer. Delete the copies after use.
The session logs did not contain separate streaming delta or item-started
records. The two `synthetic-delta-*` fixtures use the provider protocol's
agent-message delta shape and are explicitly synthetic.

`node workspaces/runtime/packages/json/fixtures/measure.ts` builds all backends
and runs Callgrind and DHAT for each fixture and operation. Measurements include
process initialization and 100 iterations. The typed benchmark is a partial
record with common fields and a flattened map of the remaining fields; it is
not a substitute for profiling each complete production record type.

On 2026-11-04, x86-64, Rust 1.99.1 and Valgrind 3.25.1, the sum over the
16 fixtures (100 iterations each, including process setup) was:

| Backend    | Parse Value Ir / allocations | Parse partial record Ir / allocations | Serialize Ir / allocations |
| ---------- | ---------------------------: | ------------------------------------: | -------------------------: |
| serde_json |        952,483,504 / 772,975 |               1,139,212,191 / 914,975 |       373,882,546 / 17,375 |
| sonic-rs   |        831,227,577 / 772,975 |               1,012,600,112 / 914,975 |       197,071,298 / 16,275 |
| simd-json  |        825,794,207 / 779,907 |                 976,393,986 / 900,907 |       279,181,923 / 14,275 |

The [per-fixture results](fixtures/results.tsv) include direct serde_json Value
parsing and borrowed raw parsing. Raw facade calls used the same allocations
and 507 additional instructions per process over 100 calls, invariant across
fixture sizes; the measurement includes argument and operation dispatch. The
alternatives offer material instruction savings on these fixtures but cannot be
the default because of the compatibility failures above.

Outside this facade, the remaining `serde_json::` text is intentional: the
HTTP build generator replaces upstream output paths with `studio_json::`, and
private-marker strings in compatibility code and fixtures must retain their
literal bytes. They are not Rust dependency paths.
