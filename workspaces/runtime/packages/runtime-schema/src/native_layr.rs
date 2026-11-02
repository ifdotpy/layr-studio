//! Server-owned availability of the installed Layr executable.
#[derive(
    Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(
    tag = "status",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum NativeLayrCapability {
    Ready,
    NotInstalled,
    NotReady { reason: String },
}

impl NativeLayrCapability {
    pub fn is_ready(&self) -> bool {
        matches!(self, Self::Ready)
    }

    pub fn message(&self) -> Option<&str> {
        match self {
            Self::Ready => None,
            Self::NotInstalled => Some("Install layr"),
            Self::NotReady { reason } => Some(reason),
        }
    }
}
