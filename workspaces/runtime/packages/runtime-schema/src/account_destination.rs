//! Server-owned account eligibility for one lead chat.

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AccountDestination {
    pub account_key: crate::account_key::AccountKey,
    pub eligibility: crate::agent_progress::AccountEligibility,
}
