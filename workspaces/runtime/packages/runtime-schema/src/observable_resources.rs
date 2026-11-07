//! Observable resource registration. Model and resource types live in the pure
//! studio-observable-codegen library; this module binds them to domain owners.
pub use studio_observable_codegen::{ObservableArg, ObservableModel, ObservableResource};

pub fn models() -> Vec<ObservableModel> {
    vec![]
}
pub fn resources() -> Vec<ObservableResource> {
    vec![]
}
