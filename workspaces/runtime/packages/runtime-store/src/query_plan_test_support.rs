//! Test-only plan inspection for descriptor-owned query witnesses.
use studio_db_api::{CodecContext, CodecError, QueryParams};

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
