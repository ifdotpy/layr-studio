//! Test-only plan inspection for descriptor-owned query witnesses.
use studio_db_api::{CodecContext, CodecError, QueryParams};

/// SQLite VM instructions executed by an isolated query on a test connection.
pub(crate) fn vm_steps<T>(
    connection: &CodecContext,
    query: impl FnOnce() -> rusqlite::Result<T>,
) -> rusqlite::Result<(T, u64)> {
    use std::sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    };
    let ticks = Arc::new(AtomicU64::new(0));
    let counter = Arc::clone(&ticks);
    connection.progress_handler(
        1,
        Some(move || {
            counter.fetch_add(1, Ordering::Relaxed);
            false
        }),
    )?;
    let result = query();
    connection.progress_handler(0, None::<fn() -> bool>)?;
    Ok((result?, ticks.load(Ordering::Relaxed)))
}

pub(crate) fn explain<P: QueryParams>(
    connection: &CodecContext,
    sql: &'static str,
    params: &P,
) -> Result<Vec<String>, CodecError> {
    params.bind(connection, |bindings| {
        let mut statement = connection.prepare(&format!("EXPLAIN QUERY PLAN {sql}"))?;
        statement.query_map(bindings, |row| row.get(3))?.collect()
    })
}
