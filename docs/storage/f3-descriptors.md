# Access and federation descriptor evidence

Wave 2 keeps the physical format. `access_tables.rs` owns the 22 access,
federation and server table descriptors; `project_grants::GrantRow` owns grants.
`canvas_schema` registers each exactly once. The descriptor parity witness compares
every catalog table/index literally and independently reconstructs the historical
grant DDL plus its peer-label ALTER. Grant capability triggers retain their
existing generated owner and revocation rule.

The feature bootstrap installs the 20 tables whose historical and current DDL
match through the shared descriptor installer. The two inboxes retain their
pre-ALTER bootstrap DDL: installing their current receipt/admission columns before
the historical migrations would repeat those ALTERs. Historical migrations remain
raw SQL. The catalog stays frozen as independent parity evidence.

The existing access-client, discovered-client, access-invite and server-inbox
state checkers are unchanged, now selected by table-level registry hooks.
Federation peer, room and inbox reports use the existing federation checker.
This preserves its diagnostics, size bounds and duplicate-key leniency; no table
has a second state report. The settings family owns its separate settings report.

Newly reported tables use the shared count-only legacy checker: adding their
descriptors must not reject states that the old checker did not inspect.

Generated CRUD now handles grant insertion, ordered grant reads and revocation;
name reservation and completion; nonce insertion and expiry; access audit append;
typed client writes; invite insertion; federation peer/room/inbox writes; server
command claims, audit completion/expiry, input claims and output-tool claims;
and server inbox admission, waiting transition and deletion. These operations
remain inside their original caller transactions. Insertion still distinguishes
an identity conflict from another UNIQUE violation. New grants still discard an
incoming revocation value, as the old insert did.

## Retained SQL and JSON boundaries

These are cutover limits, not claims that all legacy callers have been converted:

- JSON selectors, generated-expression installation, joins, aggregates, DISTINCT,
  public-only identity projections and legacy rowid ordering remain raw SQL.
- The nonce rate/budget queries retain their aggregate/anti-join semantics;
  historical nonce consolidation retains INSERT-SELECT and conflict handling.
- Federation and server-link readers/mutators still admit historical partial
  objects and normalize duplicate keys through their existing `Value` boundaries.
  Their named descriptor payloads keep exact admitted text, but replacing those
  callers with full typed reads has not been proven behavior-preserving.
- The auto-pair attempt comparator retains its existing `Field<Value>` owner:
  numeric equality, missing/null identity evidence, and duplicate-field behavior
  affect request identity. The new descriptor has a named raw-evidence payload;
  replacing the comparator requires its own equivalence proof.
- Server command snapshot mutation retains the existing `Value` owner, including
  restart recovery of partial/invalid historical snapshots. Its new claim uses
  the named snapshot payload and preserves its original encoded text.
- `runtime_server_inbox.result` remains exact nullable text. Historical delivery
  validation permits arbitrary strings; a typed reply codec cannot be imposed
  without classifying that history. `admission` uses the existing typed
  `AdmittedRequest` with exact storage text.
- Access client/invite payloads reuse their existing strict Rust owners.
  Federation signing identity now reuses the descriptor payload owner instead
  of duplicating its four signing-field declarations in runtime-services.

Room sequence fields reuse the existing representation-preserving safe-integer
owner. The caller regression covers schema-valid `1.0` and `1e0`: repository
writes retain the historical serialization, and typed rewrites retain the exact
original numeric spelling and outer whitespace.

## Per-table wave 3 work

| Source table                  | Required cutover work                                                                                                               |
| ----------------------------- | ----------------------------------------------------------------------------------------------------------------------------------- |
| runtime_access_alias_requests | Retain request-to-target/alias uniqueness; classify nullable historical keys before relation copy.                                  |
| runtime_access_audit          | Retain monotonic AUTOINCREMENT evidence and retention order; decide lossless fractional-time encoding (U1).                         |
| runtime_access_auto_attempts  | Prove comparator equivalence for raw identity fields, missing/null, numeric forms and duplicate keys.                               |
| runtime_access_clients        | Preserve strict client grants, revocation and pairing evidence; classify nullable physical keys and selector codes.                 |
| runtime_access_discovered     | Preserve discovered identity admission and lastSeen behavior; classify nullable keys and fractional clocks.                         |
| runtime_access_invites        | Preserve both invite ID and request_id uniqueness, hash and exact invitation evidence; retain legacy state-check admission.         |
| runtime_access_name_requests  | Preserve one label/actor per request and rowid pending order; classify historical applied values.                                   |
| runtime_access_name_targets   | Preserve one server/label per request and refusal on content conflict.                                                              |
| runtime_access_nonces         | Preserve composite client/nonce identity, federation namespace, integer clock, rate windows and retention budgets.                  |
| runtime_access_unrevokes      | Preserve immutable request-to-client binding and atomic client/attempt deletion.                                                    |
| runtime_federation_identity   | Retain signing-key evidence and public-only projections; prove reversible key-byte encoding before BLOB conversion.                 |
| runtime_federation_inbox      | Preserve peer/message identity, ack/kind virtual constraints and exact receipt evidence; retain duplicate-key state-check leniency. |
| runtime_federation_invites    | Retain token hashes, project scope and use/redemption evidence; decide lossless fractional clocks.                                  |
| runtime_federation_peers      | Preserve approvals/revocation, open extensions and identity key; retain partial-object readers and legacy state checker.            |
| runtime_federation_rooms      | Preserve membership, disclosure and sequence fields; retain partial-object readers and legacy state checker.                        |
| runtime_server_exec           | Preserve one command claim before process start, restart uncertainty and original snapshot evidence.                                |
| runtime_server_exec_audit     | Preserve actor/source/server/cwd/argv hash and completion evidence; decide lossless REAL start/end encoding.                        |
| runtime_server_exec_input     | Preserve one input request per ID and per-handle admission budget.                                                                  |
| runtime_server_inbox          | Preserve admitted identity, conditional transitions and exact nullable result text; retain legacy state diagnostics.                |
| runtime_server_links          | Preserve home-side permission selectors, parent epoch and worker membership; classify historical partial records.                   |
| runtime_server_output_tools   | Preserve first claim/expiry and exact REAL clock before integer conversion.                                                         |
| runtime_server_retired        | Preserve composite ID/direction and exact signatures, including nullable server/actor evidence.                                     |
| project_grants                | Preserve capability vocabulary, active-key projection, peer label and final revocation triggers; retain composite identity.         |

## SQL inventory

An ast-grep inventory compares Rust string/raw-string literals against base
`f95357707543d46618a8b44453ea03614bbec7d5`. It excludes test modules/files,
counts explicit table references in SELECT/INSERT/UPDATE/DELETE statements, and
omits descriptor DDL. A statement referencing two tables counts in both rows.
The label-only name reservation read is retained after a failing caller witness
showed that full-row decoding refused unrelated malformed actor/applied fields.
Generic table-name substitutions, generated statements and dynamically assembled
selectors are not counted; their retained boundaries are listed above.

| Table                         | Before | Retained raw | Converted | Reason for retained raw sites                                                    |
| ----------------------------- | -----: | -----------: | --------: | -------------------------------------------------------------------------------- |
| runtime_access_alias_requests |      2 |            0 |         2 | None.                                                                            |
| runtime_access_audit          |      8 |            2 |         6 | Contextual CAST projection; aggregate retention.                                 |
| runtime_access_auto_attempts  |      3 |            1 |         2 | Legacy intent comparison/admission.                                              |
| runtime_access_clients        |     32 |           24 |         8 | Partial projections, JSON permission selectors, historical migration/admission.  |
| runtime_access_discovered     |      5 |            4 |         1 | JSON ordering/cleanup and legacy admission.                                      |
| runtime_access_invites        |      8 |            7 |         1 | Partial request/hash projections, JSON migration/selectors and legacy admission. |
| runtime_access_name_requests  |      4 |            1 |         3 | Legacy rowid order and partial label admission.                                                              |
| runtime_access_name_targets   |      4 |            2 |         2 | Partial request-target projections.                                              |
| runtime_access_nonces         |      8 |            4 |         4 | Rate/budget aggregates, anti-join and historical INSERT-SELECT.                  |
| runtime_access_unrevokes      |      2 |            0 |         2 | None.                                                                            |
| runtime_federation_identity   |     12 |           10 |         2 | Public-only JSON projections and existence aggregates.                           |
| runtime_federation_inbox      |      4 |            3 |         1 | Receipt admission and JSON/rate selectors.                                       |
| runtime_federation_invites    |      6 |            4 |         2 | Existing partial receipt/redemption mutation.                                    |
| runtime_federation_peers      |     10 |           10 |         0 | Partial historical objects and JSON permission selectors.                        |
| runtime_federation_rooms      |      9 |            9 |         0 | Partial historical objects and JSON membership selectors.                        |
| runtime_server_exec           |      8 |            6 |         2 | Partial restart snapshots and JSON expiry/status selectors.                      |
| runtime_server_exec_audit     |      3 |            0 |         3 | None.                                                                            |
| runtime_server_exec_input     |      3 |            1 |         2 | Input-budget aggregate.                                                          |
| runtime_server_inbox          |      8 |            5 |         3 | Partial identity/admission projections and conditional receipt transitions.      |
| runtime_server_links          |     12 |           12 |         0 | Partial legacy objects and JSON permission/membership projections.               |
| runtime_server_output_tools   |      2 |            0 |         2 | None.                                                                            |
| runtime_server_retired        |      0 |            0 |         0 | No literal DML consumer.                                                         |
| project_grants                |     11 |            6 |         5 | Aggregate/DISTINCT authorization projections and permission joins.               |

Totals: 164 explicit table-reference sites before, 112 retained raw, 52 converted.
The peer/room generic federation writer was also converted but has no literal
table name at its SQL site, so it is outside these counts.

## Size and duplication measurement

Tokei 15 and jscpd (Rust, default thresholds) measure the same 37-file scope:
34 existing files and three added modules. The baseline is `f953577`, except
`json_record_codec.rs` uses the separately landed shared helper state
`fb26645d` so its earlier codec work is not counted again. The new generic
`exact_json_row_writes!` adapter is included. The separately landed raw-extension
decoder is unchanged and excluded. The shared CLI coverage assertion from F2A
is also excluded because its owner measures it independently.

| Metric                 | Before |  After |
| ---------------------- | -----: | -----: |
| Rust code lines        | 35,998 | 37,151 |
| jscpd clones           |    130 |    129 |
| jscpd duplicated lines |  1,339 |  1,333 |
| New clone fingerprints |      — |      0 |

The final increase is 1,153 code lines: 306 test lines, 24 descriptor literal
attribute lines, and 823 other production lines. Test modules were selected
structurally with ast-grep; tokei counts their extracted text. Literal attributes
are physical source lines (their escaped SQL spans more logical lines); other
production lines are the remaining full-file code count. Historical DDL removed
from bootstrap is included in the production delta.

The three largest growth locations are:

- `access_tables.rs`: +700 lines for 22 physical descriptors, transaction adapters,
  bootstrap installation, and acceptance/parity witnesses.
- `access_payloads.rs`: +173 lines for named payload fields and raw extension owners.
- `state_check.rs`: +56 lines for the shared count-only checker, report adapter and
  compatibility witnesses; the count-only helper lands independently.

Repeated id/record write adapters share `exact_json_row_writes!` in the codec
module. Access-client constructors reuse their existing schema owner's
`with_identity` factory, preserving its serialized field order and missing/null
history defaults. jscpd's baseline comparison reports zero new clones and zero
new duplicated lines.
