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

/// Label/range witnesses permit only the explicitly listed aggregate sorts.
pub(crate) fn assert_label_range<P: QueryParams, R>(
    connection: &CodecContext,
    name: &str,
    expected_index: &str,
    query: &studio_db_api::NamedQuery<
        crate::peer_update_settings::Canvas,
        P,
        R,
        studio_db_api::ReadMode,
        studio_db_api::Many,
    >,
    params: &P,
    aggregate_sort_reason: Option<&str>,
    budget: u64,
) {
    let plan = explain(connection, query.sql(), params).unwrap();
    assert!(
        plan.iter()
            .all(|line| !line.contains("SCAN changes") && !line.contains("AUTOMATIC")),
        "{name}: {plan:?}"
    );
    assert!(
        plan.iter().any(|line| line.contains(expected_index)),
        "{name}: {plan:?}"
    );
    if aggregate_sort_reason.is_none() {
        assert!(
            plan.iter().all(|line| !line.contains("TEMP B-TREE")),
            "{name}: {plan:?}"
        );
    }
    let (_, steps) = vm_steps(connection, || {
        crate::peer_update_settings::canvas_read(connection).collect(query, params)
    })
    .unwrap();
    eprintln!("{name}: {steps}/{budget} VM steps; {plan:?}");
    assert!(steps <= budget, "{name}: {steps} VM steps exceed {budget}");
}
