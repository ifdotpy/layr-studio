//! Typed local federation management requests.
use crate::bounded_integer::BoundedInteger;
use crate::bounded_text::BoundedText;
use crate::queue_command::Field;
use serde::{Deserialize, Serialize};

/// Maximum members in a federation room. Unit: members.
pub const MAX_ROOM_MEMBERS: usize = 50;
pub const MAX_EXPECTED_USER_CODEPOINTS: usize = 254;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "action", rename_all = "snake_case", deny_unknown_fields)]
pub enum FederationRequest {
    Status {},
    SetEnabled {
        enabled: bool,
    },
    ApprovePeer {
        state_id: String,
        #[serde(default)]
        accept_missing_whois: bool,
    },
    RevokePeer {
        state_id: String,
    },
    RoomOptions {
        room_id: String,
        #[serde(deserialize_with = "crate::http_ingress::deserialize_object")]
        disclosure: RoomDisclosure,
    },
    CreateRoom {
        peer_id: String,
        local_members: LocalMembers,
        #[serde(default)]
        share_names: bool,
        #[serde(default)]
        share_status: bool,
    },
    ApproveRoom {
        room_id: String,
        local_members: LocalMembers,
        #[serde(default)]
        share_names: bool,
        #[serde(default)]
        share_status: bool,
    },
    CreateInvite {
        #[serde(default)]
        label: Option<BoundedText<1, { usize::MAX }>>,
        #[serde(default)]
        expected_user: Option<BoundedText<0, MAX_EXPECTED_USER_CODEPOINTS>>,
        #[serde(default)]
        grant: Option<FederationGrant>,
    },
    AcceptPeer {
        #[serde(deserialize_with = "crate::http_ingress::deserialize_object")]
        invitation: FederationInvitation,
    },
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FederationGrant {
    Full,
    Observe,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FederationInvitation {
    pub protocol: BoundedInteger<
        { crate::federation_limits::PROTOCOL_VERSION },
        { crate::federation_limits::PROTOCOL_VERSION as u64 },
    >,
    pub invite_id: String,
    pub token: String,
    pub state_id: String,
    pub label: String,
    pub origin: String,
    pub public_key: String,
    pub expires: BoundedInteger<0, { crate::shared_values::MAX_SAFE_INTEGER as u64 }>,
    #[serde(default)]
    pub lead_name: Option<String>,
    #[serde(default)]
    pub tailscale_status: Option<TailscaleStatus>,
    #[serde(default)]
    pub tailscale_user: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TailscaleStatus {
    Verified,
    Unavailable,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RoomDisclosure {
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub share_names: Field<bool>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub share_status: Field<bool>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(transparent)]
pub struct LocalMembers(Vec<String>);
impl LocalMembers {
    pub fn as_slice(&self) -> &[String] {
        &self.0
    }
}
impl<'de> Deserialize<'de> for LocalMembers {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let values = Vec::<String>::deserialize(deserializer)?;
        if (1..=MAX_ROOM_MEMBERS).contains(&values.len()) {
            Ok(Self(values))
        } else {
            Err(serde::de::Error::custom("invalid member count"))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::FederationRequest;
    use serde_json::json;

    #[test]
    fn federation_limits_match_openapi() {
        let doc: serde_json::Value =
            serde_json::from_str(include_str!("../../runtime-http/openapi.json")).unwrap();
        let schemas = &doc["components"]["schemas"];
        assert_eq!(
            schemas["CreateRoomRequest"]["properties"]["local_members"]["maxItems"],
            super::MAX_ROOM_MEMBERS
        );
        assert_eq!(
            schemas["ApproveRoomRequest"]["properties"]["local_members"]["maxItems"],
            super::MAX_ROOM_MEMBERS
        );
        assert_eq!(
            schemas["CreateInviteRequest"]["properties"]["expected_user"]["anyOf"][0]["maxLength"],
            super::MAX_EXPECTED_USER_CODEPOINTS
        );
        assert_eq!(
            schemas["FederationInvitation"]["properties"]["protocol"]["const"],
            crate::federation_limits::PROTOCOL_VERSION
        );
        assert_eq!(
            schemas["FederationInvitation"]["properties"]["expires"]["maximum"],
            crate::shared_values::MAX_SAFE_INTEGER
        );
    }

    #[test]
    fn management_requests_enforce_discriminants_and_shapes() {
        for value in [
            json!({"action":"status"}),
            json!({"action":"approve_peer","state_id":"p"}),
            json!({"action":"create_room","peer_id":"p","local_members":["a"]}),
            json!({"action":"room_options","room_id":"r","disclosure":{"shareNames":null}}),
        ] {
            assert!(serde_json::from_value::<FederationRequest>(value).is_ok());
        }
        for value in [
            json!({"action":"status","extra":true}),
            json!({"action":"approve_peer"}),
            json!({"action":"create_room","peer_id":"p","local_members":[]}),
            json!({"action":"create_invite","grant":"other"}),
            json!({"action":"room_options","room_id":"r","disclosure":[true,false]}),
            json!({"action":"accept_peer","invitation":[]}),
        ] {
            assert!(
                serde_json::from_value::<FederationRequest>(value.clone()).is_err(),
                "{value}"
            );
        }
    }
}
