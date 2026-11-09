# Typed SQLite contracts

`DbTable` generates static DDL, cached CRUD SQL, positional decoding and stack
bindings. Column unit structs bind the row, value type and SQL name. A private
application marker admits a borrowed transaction through a trusted store adapter;
the outer owner keeps commit, rollback, audit and publication policy. SQL names
must be lowercase non-keyword identifiers; keys cannot be nullable, and non-INTEGER
primary keys explicitly declare NOT NULL.

`DbEnum` requires fieldless variants with explicit permanent i64 discriminants.
It generates INTEGER codecs, CHECK values and `DbValue::CODE_NAMES` for sqlite3
inspection. Wire derives remain independent. The existing runtime-schema
`persistence::domain!` owner supports this alongside historical TEXT enums.

`db_query!(tx, Row, filter(Row::COL.eq(&value)), order_by(Row::COL.desc), limit(10))`
emits static SQL and column-typed bindings, then executes through `prepare_cached`.
Clauses may appear in any input order. Comparisons, explicit null predicates,
AND, multiple orderings and `in_(&slice)` are supported. IN uses one rarray binding
regardless of slice length; an empty slice matches no rows. Connection owners load
`rusqlite::vtab::array::load_module` once, alongside the existing cache capacity.
Joins, aggregates, FTS and JSON expressions stay handwritten in the transaction.

`db_schema![Database; Row, ...]` returns ordered descriptors and state-check
functions. Descriptor diffs report changes, including enum mappings; the existing
migration runner executes transforms and progress. Descriptors export table and
index DDL/catalog entries. Custom `Codec<T>` implementations receive a plain
connection reference. JSON stays with runtime-store's `JsonRecordCodec` adapter,
which uses `JsonRecord` validation/encoding and preserves admitted raw text.
`private` marks descriptor visibility; permission projections stay application-owned.
Receipts opt out of generic upsert/delete with `#[db(receipt)]`; `RequestResults`
owns replay and conflict policy.

Nullable comparisons retain SQL semantics: `eq(&None)` and `ne(&None)` match
nothing. Use `is_null()` and `is_not_null()` to select missing/present values.
Codec columns use the same connection-aware encoder in CRUD and query bindings.
`#[db(check = check_row)]` runs an application `fn(&Row) -> Result<(), E>` after
decode and before insert/upsert. A JSON row owner uses this hook to call
`RawJsonRecord::check_key` against its physical key; field validation alone cannot
establish that relationship. Key nullability is checked through `DbValue` in an
eager const assertion, so a type alias cannot hide an optional key.

State checks count declared SQL constraint violations independently of the physical
DDL, then decode only the other rows, so each rejected row is counted once.
Boolean decoding accepts only INTEGER 0/1; optional booleans additionally accept NULL.

Legacy descriptors use `#[db(legacy_ddl = "CREATE TABLE …")]` and repeated
`#[db(legacy_index = "CREATE INDEX …")]` to keep their exact physical SQL in one
owner. Explicit column overrides on legacy DDL owners may use ASCII camelCase;
case-variant SQL keywords and invalid identifiers remain rejected. The existing
quoted lowercase legacy-keyword allowance stays unchanged. New tables continue
to derive DDL. `#[db(generated)]` fields are selected
but omitted from insert/upsert and cannot be assigned by `db_update!`.
Generated DML always quotes identifiers. Non-optional Rust keys remain required
even when legacy DDL permits NULL; state checks report those rows.

`Read::from_adapter(connection, capability)` offers get/query for connection-based
readers; mutations require `Tx`. `db_update!(tx, Row, set(Row::COL = &value),
filter(Row::ID.eq(&id)))` and `db_delete!(tx, Row, filter(...))` share the query
filter grammar and return affected rows. Mutable tables also support
`tx.insert_new(&row)` (only key conflicts return false), while
`tx.insert_rowid(&row)` returns SQLite's inserted rowid. `Descriptor::install_if_absent`
installs bootstrap DDL without replacing existing tables.

`#[db(state_check = check_row)]` calls a
`fn(&Connection) -> rusqlite::Result<StateCheck>` instead of the generic decoder for
that table’s single state-check report;
read/write admission still belongs to `check`. State-check results include counts
and aggregated pointer/keyword diagnostics. Codecs can return a boxed
`StateCheckFailure` through a rusqlite conversion error to preserve decode details.

`DbProjection` selects named fields from one existing table owner:

```rust,ignore
#[derive(DbProjection)]
#[db(of = SessionRow)]
struct SessionId { id: String }
let rows = db_query!(read, SessionRow, project(SessionId),
    filter(SessionRow::AGENT.eq_ref(agent)), limit(1u32))?;
```

A projection decodes only its selected columns and does not run a full-row check.
Its field types must match the owner. `#[db(field = owner_field)]` selects a
renamed Rust field. `#[db(raw)]` explicitly reads the codec's exact `Stored` type
instead of interpreting it; this preserves historical replay/refusal boundaries.
`eq_ref`, `ne_ref`, `lt_ref`, `le_ref`, `gt_ref`, and `ge_ref` accept borrowed native
`str` and `Option<&str>` bindings. Ordinary comparisons keep their original owned
type inference, including `None` and `.into()`. Custom codecs keep their encoder.

`tx.update_existing(&row) -> Result<bool>` validates and replaces only an existing
key; generated columns are never assigned. `#[db(revision)]` on one writable
native non-key field also generates `tx.update_existing_at(&row, &expected)`.
The supplied row contains the new revision; the predicate compares the stored
revision with `expected`. Receipt owners have neither operation.

A physical owner declares `#[db(partitioned_by = Kind)]`, with a `DbEnum` kind
field included in its composite key. Per-kind rows derive `DbPartition` with
`#[db(of = PhysicalRow, kind = Kind::Variant)]`, declare the remaining key fields,
and choose typed payload codecs and checks. The compiler checks key shape, kind
type, column names and codec storage types against the physical owner. Kind is a
fixed SQL literal supplied by its enum owner, never a field callers can mismatch.
Physical owners have no generic write capability; callers write per-kind rows.
`tx.partition::<PlanRow>().get/put/delete/list` delegates to the same table CRUD
as ordinary rows. Query/update/delete macros and state checks include the kind
predicate. Partition indexes are partial indexes for that kind; install the
physical descriptor and each partition's indexes. Receipt partitions allow reads
and explicit insertion, while omitting generic replacement and deletion.

For a single result, `project_one(SessionId)` returns `Result<Option<SessionId>>`
through `query_row(...).optional()`, with neither LIMIT nor a temporary vector.
The same clause family has `scalar(Row::COLUMN)` for `Option<Column::Value>`,
`scalar_raw(Row::COLUMN)` for exact codec storage, `scalars(Row::COLUMN)` for an
ordered vector, and `exists()` for a Boolean presence check. All keep the owner’s
typed filters and partition predicate. A nullable scalar keeps missing-row and
SQL NULL distinct as `None` and `Some(None)`; scalar codec errors stay errors.
Only one selection clause is allowed. Single-result clauses do not accept LIMIT.

A partition cannot declare a field whose resolved SQL column is the physical
kind column, including renamed fields. The generated eager assertion rejects it
at compilation. Partition indexes use `index(...)`, never `legacy_index`; a
supplied predicate is grouped as `kind = code AND (predicate)`.

Codecs separate owned read storage (`Stored: DbValue`) from write bindings
(`Encoded<'a>: ToSql`). `encode` can return `&'a str` or `&'a [u8]` borrowed from
the admitted value. Generated inserts, full-row updates and column assignments
bind that reference directly; JSON record codecs do not clone their raw text.
