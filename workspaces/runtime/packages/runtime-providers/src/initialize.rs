//! Single owner of the provider `initialize` request parameters.

use studio_json::{Value, json};

/// Client names Studio reports to a provider peer in `initialize`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClientName {
    /// The retained per-account provider session.
    AccountSession,
    /// An ephemeral model-catalog read.
    Catalog,
    /// An ephemeral catalog-reads or command-cancel session.
    CatalogReads,
    /// The native runtime update probe.
    UpdateProbe,
}

impl ClientName {
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::AccountSession => "codex_agents_canvas",
            Self::Catalog => "codex_studio_catalog",
            Self::CatalogReads => "codex_studio_catalog_reads",
            Self::UpdateProbe => "codex_studio_update",
        }
    }
}

/// Version string Studio reports in `clientInfo`.
const CLIENT_VERSION: &str = "1.0.0";

/// `initialize` params for `client`; the Claude bridge accepts and ignores them.
#[must_use]
pub fn initialize_params(client: ClientName) -> Value {
    json!({
        "clientInfo": {"name": client.name(), "version": CLIENT_VERSION},
        "capabilities": {"experimentalApi": true}
    })
}

/// Codex initialization uses the vendor-owned fields through construction.
#[must_use]
pub fn codex_initialize_params(client: ClientName) -> studio_codex_protocol::InitializeParams {
    use studio_codex_protocol::{ClientInfo, InitializeCapabilities, InitializeParams};
    InitializeParams {
        client_info: ClientInfo {
            name: client.name().into(),
            title: None,
            version: CLIENT_VERSION.into(),
        },
        capabilities: Some(Some(InitializeCapabilities {
            experimental_api: Some(true),
            explicit_gateway_oauth: None,
            extensions: None,
            mcp_server_openai_form_elicitation: None,
            opt_out_notification_methods: None,
            request_attestation: None,
            required_human_approval: None,
        })),
    }
}
