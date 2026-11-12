//! Opt-in, process-local, value-free performance aggregates.
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

use serde::Serialize;
use sha2::{Digest, Sha256};

/// Interval between aggregate logs and client reports, in seconds.
pub const REPORT_INTERVAL_SECONDS: u64 = 60;
/// Maximum rows printed from each aggregate table per interval.
pub const LOG_TABLE_ROWS: usize = 20;
/// Poll duration in milliseconds above which a poll is recorded.
pub const SLOW_POLL_MILLISECONDS: u64 = 10;
/// Named read duration, in milliseconds, above which one immediate warning is emitted.
const SLOW_READ_MILLISECONDS: u64 = 500;
/// Maximum distinct labels retained per table, in keys, excluding overflow.
pub const MAX_AGGREGATE_KEYS: usize = 4096;
/// Maximum retained label prefix, in bytes. Six full maps retain at most about
/// 51 MB of key bytes (8.5 MB per map), excluding map and value overhead.
pub const MAX_AGGREGATE_KEY_PREFIX_BYTES: usize = 2048;
const OVERFLOW_KEY: &str = "<other>";

#[derive(Clone, Default, Serialize)]
pub struct Aggregate {
    pub count: u64,
    pub total_ns: u64,
    pub max_ns: u64,
    pub rows: u64,
    pub fullscan_steps: u64,
    pub vm_steps: u64,
    pub sorts: u64,
    pub autoindexes: u64,
}

#[derive(Clone, Default, Serialize)]
pub struct Snapshot {
    pub sql: BTreeMap<String, Aggregate>,
    pub sql_by_job: BTreeMap<String, Aggregate>,
    /// Jobs and prepared statements visited (count and rows, respectively).
    pub sql_sweeps: BTreeMap<String, Aggregate>,
    pub sql_top_time: Vec<(String, Aggregate)>,
    pub sql_top_fullscan: Vec<(String, Aggregate)>,
    pub waits: BTreeMap<String, Aggregate>,
    pub routes: BTreeMap<String, Aggregate>,
    pub polls: BTreeMap<String, Aggregate>,
    pub client: Option<studio_runtime_schema::perf_reporting::PerfClientReport>,
    pub json: BTreeMap<String, studio_json::perf_reporting::Counts>,
    pub json_decode_top_time: Vec<(String, studio_json::perf_reporting::Decode)>,
    pub json_decode_by_job_top_time: Vec<(String, studio_json::perf_reporting::Decode)>,
    pub sse_events: u64,
    pub sse_bytes: u64,
}

/// Perf state memo capacity, in distinct raw SQL texts, to bound retained entries.
const NORMALIZED_SQL_MEMO_MAX_ENTRIES: usize = 4096;
/// Perf state memo admission limit, in bytes of raw SQL, to bound retained text.
const NORMALIZED_SQL_MEMO_MAX_TEXT_BYTES: usize = 8192;

#[derive(Default)]
struct State {
    snapshot: Mutex<Snapshot>,
    normalized_sql: Mutex<BTreeMap<String, Arc<str>>>,
}

impl State {
    fn normalized_sql(&self, sql: &str) -> Arc<str> {
        if sql.len() > NORMALIZED_SQL_MEMO_MAX_TEXT_BYTES {
            return normalize_sql(sql).into();
        }
        if let Some(normalized) = self
            .normalized_sql
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .get(sql)
            .cloned()
        {
            return normalized;
        }
        let normalized: Arc<str> = normalize_sql(sql).into();
        let mut memo = self
            .normalized_sql
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if memo.len() < NORMALIZED_SQL_MEMO_MAX_ENTRIES {
            memo.insert(sql.to_owned(), Arc::clone(&normalized));
        }
        normalized
    }
}

static AGGREGATES: OnceLock<State> = OnceLock::new();

fn limited_key(label: &str) -> String {
    if label.len() <= MAX_AGGREGATE_KEY_PREFIX_BYTES {
        return label.to_owned();
    }
    let mut end = MAX_AGGREGATE_KEY_PREFIX_BYTES;
    while !label.is_char_boundary(end) {
        end -= 1;
    }
    let digest = Sha256::digest(label.as_bytes());
    format!(
        "{}#{}",
        &label[..end],
        data_encoding::HEXLOWER.encode(&digest[..8])
    )
}

fn aggregate_entry<'a>(
    table: &'a mut BTreeMap<String, Aggregate>,
    label: &str,
) -> &'a mut Aggregate {
    let label = limited_key(label);
    let key = if table.contains_key(&label) || table.len() < MAX_AGGREGATE_KEYS {
        label.as_str()
    } else {
        OVERFLOW_KEY
    };
    table.entry(key.to_owned()).or_default()
}

fn update(
    select: impl FnOnce(&mut Snapshot) -> &mut BTreeMap<String, Aggregate>,
    label: &str,
    elapsed: Duration,
) {
    let mut data = AGGREGATES
        .get_or_init(State::default)
        .snapshot
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let entry = aggregate_entry(select(&mut data), label);
    let nanos = u64::try_from(elapsed.as_nanos()).unwrap_or(u64::MAX);
    entry.count = entry.count.saturating_add(1);
    entry.total_ns = entry.total_ns.saturating_add(nanos);
    entry.max_ns = entry.max_ns.max(nanos);
}

pub fn record_sql(sql: &str, elapsed: Duration) {
    let sql = AGGREGATES.get_or_init(State::default).normalized_sql(sql);
    update(|data| &mut data.sql, &sql, elapsed);
    studio_json::perf_reporting::with_job_label(|job| {
        if let Some(label) = job {
            update(
                |data| &mut data.sql_by_job,
                &format!("{label} | {sql}"),
                elapsed,
            );
        }
    });
}
pub fn record_sql_row(sql: &str) {
    let normalized = AGGREGATES.get_or_init(State::default).normalized_sql(sql);
    let mut data = AGGREGATES
        .get_or_init(State::default)
        .snapshot
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let entry = aggregate_entry(&mut data.sql, &normalized);
    entry.rows = entry.rows.saturating_add(1);
}
#[cfg(test)]
thread_local! {
    static SQL_TEXT_CONVERSIONS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static SQL_NORMALIZATIONS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// Read and reset counters of statements still prepared on this connection.
#[allow(
    unsafe_code,
    reason = "SQLite exposes statement enumeration only through its C API"
)]
pub(crate) fn sweep_sql(connection: &rusqlite::Connection, record: bool) {
    use rusqlite::ffi;
    // SAFETY: this connection is exclusively owned by the current store job;
    // no statement is prepared or finalized during this traversal. SQLite owns
    // the statement and SQL pointers until the next prepare/finalize operation.
    unsafe {
        let db = connection.handle();
        let mut statement = std::ptr::null_mut();
        let mut swept = 0_u64;
        loop {
            statement = ffi::sqlite3_next_stmt(db, statement);
            if statement.is_null() {
                break;
            }
            swept = swept.saturating_add(1);
            let sql = ffi::sqlite3_sql(statement);
            if sql.is_null() {
                continue;
            }
            let counts = [
                ffi::SQLITE_STMTSTATUS_FULLSCAN_STEP,
                ffi::SQLITE_STMTSTATUS_VM_STEP,
                ffi::SQLITE_STMTSTATUS_SORT,
                ffi::SQLITE_STMTSTATUS_AUTOINDEX,
            ]
            .map(|status| {
                u64::try_from(ffi::sqlite3_stmt_status(statement, status, 1)).unwrap_or(0)
            });
            if !record || counts.iter().all(|count| *count == 0) {
                continue;
            }
            #[cfg(test)]
            SQL_TEXT_CONVERSIONS.with(|count| count.set(count.get() + 1));
            let sql = std::ffi::CStr::from_ptr(sql).to_string_lossy();
            let normalized = AGGREGATES.get_or_init(State::default).normalized_sql(&sql);
            let mut data = AGGREGATES
                .get_or_init(State::default)
                .snapshot
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let apply = |entry: &mut Aggregate| {
                entry.fullscan_steps = entry.fullscan_steps.saturating_add(counts[0]);
                entry.vm_steps = entry.vm_steps.saturating_add(counts[1]);
                entry.sorts = entry.sorts.saturating_add(counts[2]);
                entry.autoindexes = entry.autoindexes.saturating_add(counts[3]);
            };
            apply(aggregate_entry(&mut data.sql, &normalized));
            studio_json::perf_reporting::with_job_label(|job| {
                if let Some(label) = job {
                    apply(aggregate_entry(
                        &mut data.sql_by_job,
                        &format!("{label} | {normalized}"),
                    ));
                }
            });
        }
        if record {
            studio_json::perf_reporting::with_job_label(|job| {
                if let Some(label) = job {
                    let mut data = AGGREGATES
                        .get_or_init(State::default)
                        .snapshot
                        .lock()
                        .unwrap_or_else(std::sync::PoisonError::into_inner);
                    let entry = aggregate_entry(&mut data.sql_sweeps, label);
                    entry.count = entry.count.saturating_add(1);
                    entry.rows = entry.rows.saturating_add(swept);
                }
            });
        }
    }
}
pub fn record_wait(label: &str, elapsed: Duration) {
    update(|data| &mut data.waits, label, elapsed);
}
/// Preserve the existing read aggregate and emit an immediate warning only for slow reads.
pub fn record_read(label: &str, elapsed: Duration) {
    record_wait(&format!("sqlite read {label}"), elapsed);
    if let Some(line) = slow_read_line(label, elapsed) {
        eprintln!("{line}");
    }
}

fn slow_read_line(label: &str, elapsed: Duration) -> Option<String> {
    (elapsed > Duration::from_millis(SLOW_READ_MILLISECONDS)).then(|| {
        format!(
            "warning: Canvas read {label} took {} ms",
            elapsed.as_millis()
        )
    })
}

pub fn record_route(label: &str, elapsed: Duration) {
    update(|data| &mut data.routes, label, elapsed);
}
pub fn record_poll(label: &str, elapsed: Duration) {
    if elapsed > Duration::from_millis(SLOW_POLL_MILLISECONDS) {
        update(|data| &mut data.polls, label, elapsed);
    }
}
pub fn spawn<F>(label: &'static str, future: F) -> tokio::task::JoinHandle<F::Output>
where
    F: std::future::Future + Send + 'static,
    F::Output: Send + 'static,
{
    let mut future = Box::pin(future);
    tokio::spawn(std::future::poll_fn(move |context| {
        let started = std::time::Instant::now();
        let result = future.as_mut().poll(context);
        record_poll(label, started.elapsed());
        result
    }))
}
pub fn snapshot() -> Snapshot {
    let mut snapshot = AGGREGATES
        .get_or_init(State::default)
        .snapshot
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .clone();
    snapshot.json = studio_json::perf_reporting::snapshot();
    let mut by_type = BTreeMap::<String, studio_json::perf_reporting::Decode>::new();
    let mut decodes: Vec<_> = studio_json::perf_reporting::decode_snapshot()
        .into_iter()
        .collect();
    for (label, decode) in &decodes {
        let ty = label
            .rsplit_once(" | ")
            .map_or(label.as_str(), |(_, ty)| ty);
        let total = by_type.entry(ty.to_owned()).or_default();
        total.count = total.count.saturating_add(decode.count);
        total.total_ns = total.total_ns.saturating_add(decode.total_ns);
        total.bytes = total.bytes.saturating_add(decode.bytes);
    }
    decodes.sort_by_key(|(_, decode)| std::cmp::Reverse(decode.total_ns));
    snapshot.json_decode_by_job_top_time = decodes.into_iter().take(LOG_TABLE_ROWS).collect();
    let mut types: Vec<_> = by_type.into_iter().collect();
    types.sort_by_key(|(_, decode)| std::cmp::Reverse(decode.total_ns));
    snapshot.json_decode_top_time = types.into_iter().take(LOG_TABLE_ROWS).collect();
    let mut entries: Vec<_> = snapshot.sql_by_job.clone().into_iter().collect();
    entries.sort_by_key(|(_, aggregate)| std::cmp::Reverse(aggregate.total_ns));
    snapshot.sql_top_time = entries.iter().take(LOG_TABLE_ROWS).cloned().collect();
    entries.sort_by_key(|(_, aggregate)| std::cmp::Reverse(aggregate.fullscan_steps));
    snapshot.sql_top_fullscan = entries.into_iter().take(LOG_TABLE_ROWS).collect();
    snapshot
}

pub fn record_client(mut report: studio_runtime_schema::perf_reporting::PerfClientReport) {
    for interaction in &mut report.interactions {
        if !matches!(
            interaction.kind.as_str(),
            "keydown"
                | "keyup"
                | "keypress"
                | "pointerdown"
                | "pointerup"
                | "click"
                | "mousedown"
                | "mouseup"
                | "touchstart"
                | "touchend"
        ) {
            interaction.kind = "other".to_owned();
        }
    }
    report
        .renders
        .retain(|render| matches!(render.component.as_str(), "composer" | "history"));
    AGGREGATES
        .get_or_init(State::default)
        .snapshot
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .client = Some(report);
}
pub fn record_sse(bytes: usize) {
    let mut data = AGGREGATES
        .get_or_init(State::default)
        .snapshot
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    data.sse_events = data.sse_events.saturating_add(1);
    data.sse_bytes = data
        .sse_bytes
        .saturating_add(u64::try_from(bytes).unwrap_or(u64::MAX));
}

pub fn log_top() {
    let snapshot = snapshot();
    eprintln!(
        "perf-reporting sse events={} bytes={}",
        snapshot.sse_events, snapshot.sse_bytes
    );
    for (name, entries) in [
        ("sql_top_time", &snapshot.sql_top_time),
        ("sql_top_fullscan", &snapshot.sql_top_fullscan),
    ] {
        for (label, aggregate) in entries {
            eprintln!(
                "perf-reporting {name} {label}: total_ns={} fullscan_steps={} vm_steps={} sorts={} autoindexes={}",
                aggregate.total_ns,
                aggregate.fullscan_steps,
                aggregate.vm_steps,
                aggregate.sorts,
                aggregate.autoindexes
            );
        }
    }
    for (name, table) in [
        ("sql", snapshot.sql),
        ("waits", snapshot.waits),
        ("routes", snapshot.routes),
        ("polls", snapshot.polls),
    ] {
        let mut rows: Vec<_> = table.into_iter().collect();
        rows.sort_by_key(|(_, aggregate)| std::cmp::Reverse(aggregate.total_ns));
        for (label, aggregate) in rows.into_iter().take(LOG_TABLE_ROWS) {
            eprintln!(
                "perf-reporting {name} {label}: count={} total_ns={} max_ns={} rows={} fullscan_steps={} vm_steps={} sorts={} autoindexes={}",
                aggregate.count,
                aggregate.total_ns,
                aggregate.max_ns,
                aggregate.rows,
                aggregate.fullscan_steps,
                aggregate.vm_steps,
                aggregate.sorts,
                aggregate.autoindexes
            );
        }
    }
}

/// Strip values before a statement can enter an aggregate or log.
fn normalize_sql(sql: &str) -> String {
    #[cfg(test)]
    SQL_NORMALIZATIONS.with(|count| count.set(count.get() + 1));
    let mut chars = sql.chars().peekable();
    let mut result = String::new();
    while let Some(ch) = chars.next() {
        if ch == '-' && chars.peek() == Some(&'-') {
            chars.next();
            for next in chars.by_ref() {
                if next == '\n' {
                    break;
                }
            }
            result.push(' ');
        } else if ch == '/' && chars.peek() == Some(&'*') {
            chars.next();
            while let Some(next) = chars.next() {
                if next == '*' && chars.peek() == Some(&'/') {
                    chars.next();
                    break;
                }
            }
            result.push(' ');
        } else if ch == '\'' || ch == '"' {
            result.push('?');
            while let Some(next) = chars.next() {
                if next == ch {
                    if chars.peek() == Some(&ch) {
                        chars.next();
                    } else {
                        break;
                    }
                }
            }
        } else if (ch.is_ascii_digit()
            || ch == '.' && chars.peek().is_some_and(char::is_ascii_digit))
            && !result
                .chars()
                .last()
                .is_some_and(|c| c.is_ascii_alphanumeric() || c == '_' || c == '?')
        {
            result.push('?');
            while chars
                .peek()
                .is_some_and(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '+' | '-' | '_'))
            {
                chars.next();
            }
        } else if (ch == 'x' || ch == 'X')
            && chars.peek() == Some(&'\'')
            && !result
                .chars()
                .last()
                .is_some_and(|c| c.is_ascii_alphanumeric() || c == '_')
        {
            // The following quote consumes the blob value.
        } else {
            result.push(ch);
        }
    }
    collapse_in_lists(&result)
}

fn collapse_in_lists(sql: &str) -> String {
    let bytes = sql.as_bytes();
    let identifier_byte = |byte: &u8| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'$');
    let mut result = String::new();
    let mut cursor = 0;
    while cursor < bytes.len() {
        let rest = &sql[cursor..];
        let next = rest.chars().next().expect("nonempty SQL suffix");
        if bytes
            .get(cursor)
            .is_some_and(|b| b.eq_ignore_ascii_case(&b'i'))
            && bytes
                .get(cursor + 1)
                .is_some_and(|b| b.eq_ignore_ascii_case(&b'n'))
            && (cursor == 0
                || !bytes[cursor - 1].is_ascii_alphanumeric() && bytes[cursor - 1] != b'_')
        {
            let mut index = cursor + 2;
            while bytes.get(index).is_some_and(u8::is_ascii_whitespace) {
                index += 1;
            }
            if bytes.get(index) == Some(&b'(') {
                index += 1;
                loop {
                    if bytes.get(index).is_some_and(|byte| {
                        byte.is_ascii_whitespace() || matches!(byte, b'+' | b'-' | b',')
                    }) {
                        index += 1;
                    } else if bytes.get(index) == Some(&b'?') {
                        index += 1;
                        while bytes.get(index).is_some_and(u8::is_ascii_digit) {
                            index += 1;
                        }
                    } else if bytes
                        .get(index..index + 4)
                        .is_some_and(|word| word.eq_ignore_ascii_case(b"null"))
                        && !index
                            .checked_sub(1)
                            .and_then(|previous| bytes.get(previous))
                            .is_some_and(identifier_byte)
                        && !bytes.get(index + 4).is_some_and(identifier_byte)
                    {
                        index += 4;
                    } else if bytes.get(index) == Some(&b')') {
                        result.push_str("IN (?)");
                        cursor = index + 1;
                        break;
                    } else {
                        break;
                    }
                }
                if cursor == index + 1 {
                    continue;
                }
            }
        }
        result.push(next);
        cursor += next.len_utf8();
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn named_read_warning_is_strictly_above_500_ms() {
        assert!(slow_read_line("fixture", Duration::from_millis(499)).is_none());
        assert!(slow_read_line("fixture", Duration::from_millis(500)).is_none());
        assert_eq!(
            slow_read_line("fixture", Duration::from_millis(501)).as_deref(),
            Some("warning: Canvas read fixture took 501 ms")
        );
    }

    #[test]
    fn sql_normalizes_once_across_profile_rows_and_sweeps() {
        let db = rusqlite::Connection::open_in_memory().unwrap();
        let sql = "SELECT 'private normalization witness' AS normalization_memo_witness";
        let normalized = "SELECT ? AS normalization_memo_witness";
        SQL_NORMALIZATIONS.with(|count| count.set(0));
        let mut statement = db.prepare_cached(sql).unwrap();
        let mut expected_steps = 0;
        for _ in 0..8 {
            statement.query_row([], |_| Ok(())).unwrap();
            expected_steps += statement.get_status(rusqlite::StatementStatus::VmStep) as u64;
            record_sql(sql, Duration::from_nanos(7));
            record_sql_row(sql);
            sweep_sql(&db, true);
        }
        let normalizations = SQL_NORMALIZATIONS.with(std::cell::Cell::get);
        let report = snapshot();
        let entry = &report.sql[normalized];
        assert_eq!(entry.count, 8);
        assert_eq!(entry.rows, 8);
        assert_eq!(entry.total_ns, 56);
        assert_eq!(entry.max_ns, 7);
        assert_eq!(entry.vm_steps, expected_steps);
        assert!(
            !studio_json::to_string(&report)
                .unwrap()
                .contains("private normalization witness")
        );
        assert_eq!(
            normalizations, 1,
            "one normalization per distinct raw SQL text"
        );
    }
    #[test]
    fn sql_normalization_memo_full_preserves_reports() {
        let state = State::default();
        for index in 0..NORMALIZED_SQL_MEMO_MAX_ENTRIES {
            state.normalized_sql(&format!("SELECT {index} AS memo_capacity_witness"));
        }
        let sql = "SELECT 'private fallback witness' AS memo_capacity_witness";
        let expected = normalize_sql(sql);
        SQL_NORMALIZATIONS.with(|count| count.set(0));
        let cached = state.normalized_sql("SELECT 0 AS memo_capacity_witness");
        assert_eq!(&*cached, expected);
        assert_eq!(SQL_NORMALIZATIONS.with(std::cell::Cell::get), 0);
        for _ in 0..2 {
            let normalized = state.normalized_sql(sql);
            let mut snapshot = state.snapshot.lock().unwrap();
            let entry = aggregate_entry(&mut snapshot.sql, &normalized);
            entry.count += 1;
            entry.rows += 1;
        }
        assert_eq!(SQL_NORMALIZATIONS.with(std::cell::Cell::get), 2);
        let memo = state.normalized_sql.lock().unwrap();
        assert_eq!(memo.len(), NORMALIZED_SQL_MEMO_MAX_ENTRIES);
        assert!(!memo.contains_key(sql));
        let mut direct = Snapshot::default();
        let entry = aggregate_entry(&mut direct.sql, &expected);
        entry.count = 2;
        entry.rows = 2;
        assert_eq!(
            studio_json::to_string(&*state.snapshot.lock().unwrap()).unwrap(),
            studio_json::to_string(&direct).unwrap()
        );
    }
    #[test]
    fn sql_normalization_memo_skips_long_text() {
        let state = State::default();
        let sql = format!(
            "SELECT '{}'",
            "x".repeat(NORMALIZED_SQL_MEMO_MAX_TEXT_BYTES)
        );
        SQL_NORMALIZATIONS.with(|count| count.set(0));
        // Holding the report lock must not prevent memo lookups or normalization.
        let _snapshot = state.snapshot.lock().unwrap();
        for _ in 0..2 {
            assert_eq!(&*state.normalized_sql(&sql), "SELECT ?");
        }
        assert_eq!(SQL_NORMALIZATIONS.with(std::cell::Cell::get), 2);
        assert!(state.normalized_sql.lock().unwrap().is_empty());
        let boundary = "x".repeat(NORMALIZED_SQL_MEMO_MAX_TEXT_BYTES);
        state.normalized_sql(&boundary);
        state.normalized_sql(&boundary);
        assert_eq!(SQL_NORMALIZATIONS.with(std::cell::Cell::get), 3);
        assert_eq!(state.normalized_sql.lock().unwrap().len(), 1);
    }
    #[test]
    fn sql_sweep_converts_only_executed_statements() {
        let db = rusqlite::Connection::open_in_memory().unwrap();
        let mut active = db
            .prepare_cached("SELECT 'sweep conversion witness' AS sweep_conversion_witness")
            .unwrap();
        let _inactive = db.prepare_cached("SELECT 'inactive witness'").unwrap();
        let conversions = || SQL_TEXT_CONVERSIONS.with(|count| count.replace(0));
        conversions();
        active.query_row([], |_| Ok(())).unwrap();
        sweep_sql(&db, false);
        assert_eq!(conversions(), 0, "discarding counters needs no SQL text");
        active.query_row([], |_| Ok(())).unwrap();
        let expected_steps = active.get_status(rusqlite::StatementStatus::VmStep) as u64;
        let sql = "SELECT ? AS sweep_conversion_witness";
        let previous_steps = snapshot().sql.get(sql).map_or(0, |entry| entry.vm_steps);
        sweep_sql(&db, true);
        assert_eq!(conversions(), 1, "only one statement executed");
        assert_eq!(
            snapshot().sql[sql].vm_steps - previous_steps,
            expected_steps
        );
        sweep_sql(&db, true);
        assert_eq!(conversions(), 0, "counters were reset by the prior sweep");
    }
    #[test]
    fn sql_values_are_removed() {
        let sql = normalize_sql("SELECT '$.agent', 'a''b', x'abcdef', 42 FROM t WHERE n = ?1");
        assert_eq!(sql, "SELECT ?, ?, ?, ? FROM t WHERE n = ?1");
        assert_eq!(
            normalize_sql("SELECT * FROM t WHERE x IN (1, 2, ?3)"),
            "SELECT * FROM t WHERE x IN (?)"
        );
        assert_eq!(
            normalize_sql("SELECT \"private\", x'beef', 1_234, 1e5, 0x1F, -42"),
            "SELECT ?, ?, ?, ?, ?, -?"
        );
        assert_eq!(
            normalize_sql("SELECT * FROM t WHERE x IN (-1, 2, 'private', ?4)"),
            "SELECT * FROM t WHERE x IN (?)"
        );
        assert_eq!(normalize_sql("x IN (- 1, - 2)"), "x IN (?)");
        assert_eq!(normalize_sql("x IN (.1, .2)"), "x IN (?)");
        assert_eq!(normalize_sql("x IN (NULL, 1, NULL)"), "x IN (?)");
        assert_eq!(normalize_sql("x IN (- - 1, + + 2)"), "x IN (?)");
        assert_eq!(normalize_sql("x IN (+-1, - .5)"), "x IN (?)");
        assert_eq!(normalize_sql("x IN (NULL, -1)"), "x IN (?)");
        assert_eq!(normalize_sql("x IN (NULLNULL, ?)"), "x IN (NULLNULL, ?)");
        assert_eq!(normalize_sql("x IN (NULL_x, ?)"), "x IN (NULL_x, ?)");
        assert_eq!(
            normalize_sql("SELECT '/* private */' -- private\nFROM t"),
            "SELECT ?  FROM t"
        );
    }

    #[test]
    fn sql_aggregates_limit_distinct_statements() {
        let mut table = BTreeMap::new();
        for index in 0..=MAX_AGGREGATE_KEYS {
            aggregate_entry(&mut table, &format!("statement_{index}")).count += 1;
        }
        assert_eq!(table.len(), MAX_AGGREGATE_KEYS + 1);
        assert_eq!(table[OVERFLOW_KEY].count, 1);
    }

    #[test]
    fn long_labels_are_bounded_and_distinct() {
        let prefix = "a".repeat(MAX_AGGREGATE_KEY_PREFIX_BYTES);
        let one = limited_key(&(prefix.clone() + "one"));
        let two = limited_key(&(prefix + "two"));
        assert!(one.len() <= MAX_AGGREGATE_KEY_PREFIX_BYTES + 17);
        assert_ne!(one, two);
        assert!(
            limited_key(&"é".repeat(MAX_AGGREGATE_KEY_PREFIX_BYTES))
                .is_char_boundary(MAX_AGGREGATE_KEY_PREFIX_BYTES)
        );
    }
}
