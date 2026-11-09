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
