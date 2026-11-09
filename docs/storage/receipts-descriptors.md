# Receipt and account descriptors

Wave W4-C registers all eleven current tables in `receipt_tables.rs` without
changing their physical format. Existing transfer, capacity and usage-resume
checks retain their original diagnostics; the other tables count rows without
new rejection. Named raw envelopes retain polymorphic evidence; their existing
service callers still own interpretation. A descriptor does not authorize replay,
redemption, or a second external attempt.

| Table                        | Later consolidation requirement                                                                                               |
| ---------------------------- | ----------------------------------------------------------------------------------------------------------------------------- |
| request_results              | Preserve exact reply/reference/rejected variants, signature conflict priority, Unix-ms TTL and stable reference revisions.    |
| runtime_outbox               | Preserve kind-specific owner/admission signatures, unknown-effect fences, stream order, exact payloads and Unix-ms deadlines. |
| runtime_account_state        | Preserve independent generation, connection, ACK and recovery facts, exact stream metadata and observation seconds.           |
| runtime_account_transfers    | Preserve member order, missing/null/unknown evidence, target selector and existing rowid admission watermarks.                |
| runtime_analytics_projection | Preserve the Canvas sequence, complete post-commit evidence and atomic analytics capture cursor.                              |
| runtime_capacity_retries     | Preserve physical agent ownership, bounded typed snapshots, safe integral decimals and existing validation diagnostics.       |
| runtime_safety_retries       | Preserve stage/dispatch identities and exact uncertain provider receipts before moving the service-owned envelope.            |
| runtime_usage_resumes        | Preserve external row keys, open cause/status, arbitrary account evidence, missing/null and auth recovery filtering.          |
| runtime_limit_reset_attempts | Preserve credit/idempotency identity, admission and certainty evidence; missing replies never admit another charge.           |
| runtime_limit_reset_requests | Preserve request-to-attempt bindings independently of request-result retention.                                               |
| runtime_error_incidents      | Preserve dismissal-only null payloads, first diagnostic body, Unix-ms history and independent dismissal retention.            |

## SQL sites

`RequestResults::settle_reference` uses the generated signature scalar: it reads
no result payload or unrelated reference fields. The other receipt replay tuple,
expiry and bounded indexed cleanup stay raw because their existing forms are
shorter than separate projection declarations or include subqueries.

Remaining DML stays raw for these reasons:

- Outbox repository: family-generic decode, immutable identity conflict predicates
  and JSON selectors are shorter in the existing SQL.
- Account state: narrow generation/ACK/observation reads and arithmetic updates
  must not newly decode unread stream or failure evidence.
- Account transfers: joins, JSON selection, partial updates and size-bounded raw
  reads retain the existing caller's admission boundaries.
- Analytics projection: connection-bound append/prune and sequence management
  preserve the owner's transaction and are already shorter than adapters.
- Capacity, safety and usage continuations: JSON predicates and guarded
  physical-agent writes retain the existing service contracts.
- Limit reset attempts/requests: service-owned credit scans and alias writes keep
  redemption and lost-reply decisions within their existing transaction.
- Error incidents: first-body-wins and monotone dismissal upserts are shorter raw.

DDL and index declarations come only from the descriptors. Dated migration
fixtures remain historical evidence. Enum triggers retain their shared enum
owner and are installed after the tables.

## Historical installation order

The Oct 30 text-blob migration calls feature preparation while transfers still
have only `id` and `record`. Feature preparation installs the transfer status
index only; the Nov 6 idle-unload migration adds `target_account_key` and then
installs the complete transfer descriptor. The descriptor remains the sole owner
of both index definitions. The upgrade regression runs those actual migrations
in order over a populated historical two-column transfer table.

The remaining W4-C index dependencies are available before their installers:
request-result indexes use `id`, `created` and `result`, established by Oct 29
before receipt creation runs on Oct 30; every outbox index uses original receipt
columns, with none referencing the later `target` column; incident indexes use
original receipt-history columns; usage-resume indexes use the original `agent`
and `record` columns. The other six tables add no explicit indexes. Request-result
enum triggers stay in their existing post-column migration owner and are not
installed by early feature preparation.
