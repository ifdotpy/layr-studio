//! Terminal request bodies and operation limits.
use crate::bounded_integer::BoundedInteger;
use crate::bounded_text::BoundedText;
use serde::{Deserialize, Serialize};

pub const TERMINAL_CREATE_REQUEST_ID_MAX_CODEPOINTS: usize = 200;
pub const TERMINAL_CREATE_AGENT_ID_MAX_CODEPOINTS: usize = 200;
pub const TERMINAL_ID_MAX_CODEPOINTS: usize = 200;
pub const TERMINAL_INPUT_REQUEST_ID_MAX_CODEPOINTS: usize = 200;
pub const TERMINAL_INPUT_MAX_CODEPOINTS: usize = 65_536;
pub const TERMINAL_TITLE_MAX_CODEPOINTS: usize = 120;
pub const TERMINAL_COLS_MIN: i64 = 2;
pub const TERMINAL_COLS_MAX: u64 = 1000;
pub const TERMINAL_ROWS_MIN: i64 = 1;
pub const TERMINAL_ROWS_MAX: u64 = 500;
pub const TERMINAL_COLS_DEFAULT: u64 = 100;
pub const TERMINAL_ROWS_DEFAULT: u64 = 28;

type TerminalId = BoundedText<1, TERMINAL_ID_MAX_CODEPOINTS>;
type Columns = BoundedInteger<TERMINAL_COLS_MIN, TERMINAL_COLS_MAX>;
type Rows = BoundedInteger<TERMINAL_ROWS_MIN, TERMINAL_ROWS_MAX>;

fn present<'de, D: serde::Deserializer<'de>, T: Deserialize<'de>>(
    deserializer: D,
) -> Result<Option<T>, D::Error> {
    T::deserialize(deserializer).map(Some)
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct TerminalCreateBody {
    pub id: BoundedText<1, TERMINAL_CREATE_REQUEST_ID_MAX_CODEPOINTS>,
    pub agent: BoundedText<1, TERMINAL_CREATE_AGENT_ID_MAX_CODEPOINTS>,
    #[serde(
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    pub cols: Option<Columns>,
    #[serde(
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    pub rows: Option<Rows>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TerminalInput {
    pub id: TerminalId,
    pub text: BoundedText<0, TERMINAL_INPUT_MAX_CODEPOINTS>,
    pub request_id: BoundedText<1, TERMINAL_INPUT_REQUEST_ID_MAX_CODEPOINTS>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TerminalResize {
    pub id: TerminalId,
    #[serde(default, deserialize_with = "present")]
    pub cols: Option<Columns>,
    #[serde(default, deserialize_with = "present")]
    pub rows: Option<Rows>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TerminalRename {
    pub id: TerminalId,
    pub title: BoundedText<1, TERMINAL_TITLE_MAX_CODEPOINTS>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TerminalClose {
    pub id: TerminalId,
}

pub enum TerminalAction {
    Input(TerminalInput),
    Resize(TerminalResize),
    Rename(TerminalRename),
    Close(TerminalClose),
}
impl TerminalAction {
    pub fn id(&self) -> &str {
        match self {
            Self::Input(v) => v.id.as_str(),
            Self::Resize(v) => v.id.as_str(),
            Self::Rename(v) => v.id.as_str(),
            Self::Close(v) => v.id.as_str(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Value, json};

    #[test]
    fn action_limits_match_openapi() {
        let doc: Value =
            serde_json::from_str(include_str!("../../runtime-http/openapi.json")).unwrap();
        let schemas = &doc["components"]["schemas"];
        for schema in [
            "TerminalInput",
            "TerminalResize",
            "TerminalRename",
            "TerminalClose",
        ] {
            assert_eq!(
                schemas[schema]["properties"]["id"]["maxLength"],
                TERMINAL_ID_MAX_CODEPOINTS
            );
        }
        assert_eq!(
            schemas["TerminalInput"]["properties"]["request_id"]["maxLength"],
            TERMINAL_INPUT_REQUEST_ID_MAX_CODEPOINTS
        );
        assert_eq!(
            schemas["TerminalInput"]["properties"]["text"]["maxLength"],
            TERMINAL_INPUT_MAX_CODEPOINTS
        );
        assert_eq!(
            schemas["TerminalRename"]["properties"]["title"]["maxLength"],
            TERMINAL_TITLE_MAX_CODEPOINTS
        );
        for schema in ["TerminalCreate", "TerminalResize"] {
            assert_eq!(
                schemas[schema]["properties"]["cols"]["minimum"],
                TERMINAL_COLS_MIN
            );
            assert_eq!(
                schemas[schema]["properties"]["cols"]["maximum"],
                TERMINAL_COLS_MAX
            );
            assert_eq!(
                schemas[schema]["properties"]["rows"]["minimum"],
                TERMINAL_ROWS_MIN
            );
            assert_eq!(
                schemas[schema]["properties"]["rows"]["maximum"],
                TERMINAL_ROWS_MAX
            );
        }
    }

    #[test]
    fn terminal_actions_enforce_shape_and_bounds() {
        assert!(
            serde_json::from_value::<TerminalInput>(
                json!({"id":"i", "text":"x", "request_id":"r"})
            )
            .is_ok()
        );
        assert!(serde_json::from_value::<TerminalResize>(json!({"id":"i"})).is_ok());
        assert!(
            serde_json::from_value::<TerminalRename>(json!({"id":"i", "title":"name"})).is_ok()
        );
        assert!(serde_json::from_value::<TerminalClose>(json!({"id":"i"})).is_ok());
        assert!(serde_json::from_value::<TerminalResize>(json!({"id":"i", "cols":null})).is_err());
        assert!(serde_json::from_value::<TerminalClose>(json!({"id":"i", "extra":true})).is_err());
        assert!(
            serde_json::from_value::<TerminalInput>(json!({"id":"i", "text":"x", "request_id":""}))
                .is_err()
        );
    }
}
