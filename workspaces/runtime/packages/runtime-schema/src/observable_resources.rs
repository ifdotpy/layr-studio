//! Binds the pure observable generator to the Rust sidebar contract owners.
pub use studio_observable_codegen::{ObservableArg, ObservableModel, ObservableResource};

pub fn models() -> Vec<ObservableModel> {
    vec![
        ObservableModel::of::<crate::sidebar::SidebarChatRow>()
            .identity("id")
            .ordered_by("sortKey"),
    ]
}
pub fn resources() -> Vec<ObservableResource> {
    vec![ObservableResource::of::<crate::sidebar::SidebarChatList>(
        "sidebar",
        "sidebar",
        vec![],
    )]
}
