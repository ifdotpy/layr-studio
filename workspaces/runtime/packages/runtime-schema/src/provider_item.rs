//! The kind of a provider thread item at the provider boundary. Studio knows some
//! kinds; a kind it does not know stays `Unknown`, with its raw item, so a newer
//! provider never breaks the transcript and nothing is dropped.

use studio_json::Value;

/// A thread item kind that Studio handles by name. Owner of the list: this enum.
#[derive(
    Clone, Copy, Debug, Eq, PartialEq, strum::EnumIter, strum::EnumString, strum::IntoStaticStr,
)]
#[strum(serialize_all = "camelCase")]
pub enum ProviderItemKind {
    UserMessage,
    AgentMessage,
    Reasoning,
    Plan,
    CommandExecution,
    FileChange,
    McpToolCall,
    DynamicToolCall,
    ComputerToolCall,
    CollabAgentToolCall,
    WebSearch,
    ImageView,
    ImageGeneration,
    EnteredReviewMode,
    ExitedReviewMode,
    ContextCompaction,
    HookPrompt,
}

impl ProviderItemKind {
    /// The wire name of the kind.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        self.into()
    }
}

/// A provider thread item: a known kind, or an unknown kind with its raw item.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ProviderItem<'a> {
    Known {
        kind: ProviderItemKind,
        raw: &'a Value,
    },
    Unknown {
        kind: &'a str,
        raw: &'a Value,
    },
}

impl<'a> ProviderItem<'a> {
    /// The item `raw`; None when it has no text `type`.
    #[must_use]
    pub fn of(raw: &'a Value) -> Option<Self> {
        let name = raw.get("type").and_then(Value::as_str)?;
        Some(match name.parse() {
            Ok(kind) => Self::Known { kind, raw },
            Err(_) => Self::Unknown { kind: name, raw },
        })
    }

    /// The known kind; None for an unknown one.
    #[must_use]
    pub fn known(self) -> Option<ProviderItemKind> {
        match self {
            Self::Known { kind, .. } => Some(kind),
            Self::Unknown { .. } => None,
        }
    }

    /// The wire name of the kind, known or not.
    #[must_use]
    pub fn name(self) -> &'a str {
        match self {
            Self::Known { kind, .. } => kind.as_str(),
            Self::Unknown { kind, .. } => kind,
        }
    }

    #[must_use]
    pub fn raw(self) -> &'a Value {
        match self {
            Self::Known { raw, .. } | Self::Unknown { raw, .. } => raw,
        }
    }

    #[must_use]
    pub fn is(self, kind: ProviderItemKind) -> bool {
        self.known() == Some(kind)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use strum::IntoEnumIterator;
    use studio_json::json;

    #[test]
    fn known_kinds_round_trip_and_new_kinds_keep_their_raw_item() {
        for kind in ProviderItemKind::iter() {
            let raw = json!({"type": kind.as_str()});
            let item = ProviderItem::of(&raw).expect("item");
            assert_eq!(item.known(), Some(kind));
            assert_eq!(item.name(), kind.as_str());
        }
        assert_eq!(ProviderItemKind::McpToolCall.as_str(), "mcpToolCall");
        let raw = json!({"type":"hologram","id":"x"});
        let item = ProviderItem::of(&raw).expect("item");
        assert_eq!(
            item,
            ProviderItem::Unknown {
                kind: "hologram",
                raw: &raw
            }
        );
        assert_eq!(item.raw()["id"], "x");
        assert!(ProviderItem::of(&json!({"id":"no type"})).is_none());
    }
}
