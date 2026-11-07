//! Axum HTTP edge for the Studio runtime.

mod bounded_entities;
pub mod error;
mod http_body_limits;
#[cfg(test)]
mod ingress_parity_tests;
#[cfg(test)]
use studio_runtime_schema::ingress_pattern;
mod json_body;
pub mod middleware;
mod model_entities;
mod mutation_tasks;
mod route_class;
pub mod router;
pub mod routes;
pub mod schema;
mod validation;

pub use router::{HttpState, router};
pub use routes::{NativeContextRepairPort, backend_build_identity, initialize_backend_identity};
pub use schema::{API_SCHEMA_HASH_HEADER, API_SCHEMA_MISMATCH_HEADER, api_schema_hash, operations};

// Generated HTTP types follow the schema, including nested optional fields.
pub use studio_runtime_schema::generated;

#[cfg(test)]
mod tests;
