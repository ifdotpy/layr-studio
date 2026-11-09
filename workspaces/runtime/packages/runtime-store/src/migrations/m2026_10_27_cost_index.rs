//! Replace the group lookup index with a covering index without rebuilding facts.
use studio_db_api::Table;

/// Group lookup and write identity prefix, followed by the remaining read columns.
pub(crate) const INDEX: &str = crate::analytics_schema::AnalyticsCostGroupsRow::INDEXES[0].sql;
