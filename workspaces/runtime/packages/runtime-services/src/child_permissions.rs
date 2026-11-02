use studio_runtime_providers::ProviderKind;
use studio_runtime_schema::{claude_permissions::PermissionMode, creation_intent::AgentRole};
use studio_runtime_store::AgentRecord;

/// Resolve permissions from the current rows inside the child commit transaction.
/// Only the permission mode transfers; other Claude settings belong to the child.
pub(crate) fn inherit_claude_permissions(
    child: &mut AgentRecord,
    parent: &AgentRecord,
    root: &AgentRecord,
    requested: Option<PermissionMode>,
) -> Result<(), &'static str> {
    if child.provider.value() != Some(&ProviderKind::Claude) {
        if requested.is_some() {
            return Err("permission_mode requires a Claude child");
        }
        return Ok(());
    }
    let inherited = if parent.provider.value() == Some(&ProviderKind::Claude) {
        parent.permission_mode.unwrap_or(PermissionMode::Default)
    } else {
        root.permission_mode
            .unwrap_or(PermissionMode::BypassPermissions)
    };
    child.permission_mode = Some(if child.role.value() == Some(&AgentRole::Reviewer) {
        PermissionMode::Plan
    } else {
        match requested {
            Some(PermissionMode::BypassPermissions) if !inherited.is_full_access() => inherited,
            Some(mode) => mode,
            None => inherited,
        }
    });
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use studio_runtime_schema::queue_command::Field;

    #[test]
    fn child_inherits_canonical_permission_mode() {
        let mut child = AgentRecord::test_defaults();
        child.provider = Field::Value(ProviderKind::Claude);
        child.role = Field::Value(AgentRole::Implementer);
        let mut parent = AgentRecord::test_defaults();
        parent.provider = Field::Value(ProviderKind::Claude);
        parent.permission_mode = Some(PermissionMode::Plan);
        inherit_claude_permissions(&mut child, &parent, &parent, None).unwrap();
        assert_eq!(child.permission_mode, Some(PermissionMode::Plan));
        parent.permission_mode = None;
        inherit_claude_permissions(&mut child, &parent, &parent, None).unwrap();
        assert_eq!(child.permission_mode, Some(PermissionMode::Default));
        parent.provider = Field::Value(ProviderKind::Codex);
        inherit_claude_permissions(&mut child, &parent, &parent, None).unwrap();
        assert_eq!(
            child.permission_mode,
            Some(PermissionMode::BypassPermissions)
        );
    }
}
