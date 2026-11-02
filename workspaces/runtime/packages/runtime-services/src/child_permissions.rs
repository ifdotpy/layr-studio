use serde::Deserialize;
use serde_json::{Value, json};
use studio_runtime_providers::ProviderKind;
use studio_runtime_schema::claude_permissions::PermissionMode;

/// Resolve permissions from the current rows inside the child commit transaction.
/// Only the permission mode transfers; other Claude settings belong to the child.
pub(crate) fn inherit_claude_permissions(
    child: &mut Value,
    parent: &Value,
    root: &Value,
    requested: Option<studio_runtime_schema::claude_permissions::PermissionMode>,
) -> Result<(), &'static str> {
    if child["provider"] != ProviderKind::Claude.key() {
        if requested.is_some() {
            return Err("permission_mode requires a Claude child");
        }
        return Ok(());
    }
    let inherited = if parent["provider"] == ProviderKind::Claude.key() {
        parent
            .get("permissionMode")
            .filter(|value| !value.is_null())
            .map(PermissionMode::deserialize)
            .transpose()
            .map_err(|_| "Invalid parent Claude permission mode")?
            .unwrap_or(PermissionMode::Default)
    } else {
        root.get("permissionMode")
            .filter(|value| !value.is_null())
            .map(PermissionMode::deserialize)
            .transpose()
            .map_err(|_| "Invalid root permission mode")?
            .unwrap_or(PermissionMode::BypassPermissions)
    };
    let mode = if child["role"] == "reviewer" {
        PermissionMode::Plan
    } else {
        match requested {
            Some(PermissionMode::BypassPermissions) if !inherited.is_full_access() => inherited,
            Some(mode) => mode,
            None => inherited,
        }
    };
    child["permissionMode"] = json!(mode);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn null_permission_inherits_same_default_as_absence() {
        for provider in ["claude", "codex"] {
            for mode in [None, Some(Value::Null)] {
                let mut parent = json!({"provider":provider});
                if let Some(mode) = mode {
                    parent["permissionMode"] = mode;
                }
                let mut child = json!({"provider":"claude", "role":"implementer"});
                inherit_claude_permissions(&mut child, &parent, &parent, None).unwrap();
                assert_eq!(
                    child["permissionMode"],
                    if provider == "claude" {
                        "default"
                    } else {
                        "bypassPermissions"
                    }
                );
            }
        }
    }

    #[test]
    fn child_inherits_canonical_permission_mode() {
        let mut child = json!({"provider":"claude","role":"implementer"});
        let parent = json!({"provider":"claude","permissionMode":"plan"});
        inherit_claude_permissions(&mut child, &parent, &parent, None).unwrap();
        assert_eq!(child["permissionMode"], "plan");
        assert!(child.get("yoloMode").is_none());
        assert!(child.get("claudeOptions").is_none());
    }
}
