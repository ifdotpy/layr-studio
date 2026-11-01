//! Shared bounds for HTTP ingress and its service consumers.

/// Maximum account display label length, in Unicode code points.
pub const ACCOUNT_NAME_MAX_CHARS: usize = 32;
/// Maximum login label length, in Unicode code points.
pub const LOGIN_LABEL_MAX_CHARS: usize = 32;
/// Maximum login email length, in Unicode code points.
pub const LOGIN_EMAIL_MAX_CHARS: usize = 320;
/// Length of an account mutation request ID, in ASCII bytes.
pub const ACCOUNT_MUTATION_ID_BYTES: usize = 36;
/// Length of an account login ID, in ASCII bytes.
pub const ACCOUNT_LOGIN_ID_BYTES: usize = 36;
/// Maximum Claude file-backup agent ID length, in Unicode code points.
pub const BACKUP_AGENT_ID_MAX_CHARS: usize = 1024;
/// Maximum Claude file-backup request ID length, in Unicode code points.
pub const BACKUP_REQUEST_ID_MAX_CHARS: usize = 1024;
/// Maximum profile name length, in Unicode code points.
pub const PROFILE_NAME_MAX_CHARS: usize = 100;
/// Maximum profile model length, in Unicode code points.
pub const PROFILE_MODEL_MAX_CHARS: usize = 100;
/// Maximum profile instructions length, in Unicode code points.
pub const PROFILE_INSTRUCTIONS_MAX_CHARS: usize = 16000;
/// Maximum complaint request ID length, in Unicode code points.
pub const COMPLAINT_ID_MAX_CHARS: usize = 200;
/// Maximum complaint text length, in Unicode code points.
pub const COMPLAINT_TEXT_MAX_CHARS: usize = 12000;
/// Maximum attachment agent ID length, in Unicode code points.
pub const ASSET_AGENT_ID_MAX_CHARS: usize = 200;
/// Maximum attachment ID length, in Unicode code points.
pub const ASSET_ID_MAX_CHARS: usize = 200;
/// Maximum attachment file name length, in Unicode code points.
pub const ASSET_NAME_MAX_CHARS: usize = 255;
/// Maximum forwarding server ID length, in Unicode code points.
pub const FORWARD_SERVER_MAX_CHARS: usize = 200;
/// Minimum and maximum forwarding token length, in Unicode code points.
pub const FORWARD_TOKEN_MIN_CHARS: usize = 16;
pub const FORWARD_TOKEN_MAX_CHARS: usize = 200;
/// Maximum forwarding path length, in Unicode code points.
pub const FORWARD_PATH_MAX_CHARS: usize = 2048;
/// Exact forwarding bridge ID length, in ASCII bytes.
pub const FORWARD_BRIDGE_ID_BYTES: usize = 32;
/// Smallest forwarding port, in TCP port numbers.
pub const FORWARD_PORT_MIN: u16 = 1024;
/// VM memory bounds, in bytes.
pub const VM_MEMORY_MIN_BYTES: i64 = 1_073_741_824;
pub const VM_MEMORY_MAX_BYTES: u64 = 68_719_476_736;
/// VM system disk bounds, in bytes.
pub const VM_SYSTEM_DISK_MIN_BYTES: i64 = 8_589_934_592;
pub const VM_SYSTEM_DISK_MAX_BYTES: u64 = 137_438_953_472;
/// VM data disk bounds, in bytes.
pub const VM_DATA_DISK_MIN_BYTES: i64 = 8_589_934_592;
pub const VM_DATA_DISK_MAX_BYTES: u64 = 1_099_511_627_776;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ingress_limit_owners_match_openapi_consumers() {
        let document: serde_json::Value =
            serde_json::from_str(include_str!("../../runtime-http/openapi.json")).unwrap();
        let checks: &[(&str, u64)] = &[
            (
                "AccountNameRequest/properties/label/maxLength",
                ACCOUNT_NAME_MAX_CHARS as u64,
            ),
            (
                "AccountNameRequest/properties/request_id/maxLength",
                ACCOUNT_MUTATION_ID_BYTES as u64,
            ),
            (
                "DeleteAccountRequest/properties/request_id/maxLength",
                ACCOUNT_MUTATION_ID_BYTES as u64,
            ),
            (
                "LoginRequest/properties/login_id/maxLength",
                ACCOUNT_LOGIN_ID_BYTES as u64,
            ),
            (
                "ClaudeCancelRequest/properties/login_id/maxLength",
                ACCOUNT_LOGIN_ID_BYTES as u64,
            ),
            (
                "LoginRequest/properties/email/anyOf/0/maxLength",
                LOGIN_EMAIL_MAX_CHARS as u64,
            ),
            (
                "LoginRequest/properties/label/anyOf/0/maxLength",
                LOGIN_LABEL_MAX_CHARS as u64,
            ),
            (
                "ClaudeFileBackupsRequest/properties/id/maxLength",
                BACKUP_AGENT_ID_MAX_CHARS as u64,
            ),
            (
                "ClaudeFileBackupsRequest/properties/request_id/maxLength",
                BACKUP_REQUEST_ID_MAX_CHARS as u64,
            ),
            (
                "ProfileBody/properties/instructions/maxLength",
                PROFILE_INSTRUCTIONS_MAX_CHARS as u64,
            ),
            (
                "ProfileBody/properties/name/anyOf/0/maxLength",
                PROFILE_NAME_MAX_CHARS as u64,
            ),
            (
                "ProfileBody/properties/model/anyOf/0/maxLength",
                PROFILE_MODEL_MAX_CHARS as u64,
            ),
            (
                "ComplaintBody/properties/id/maxLength",
                COMPLAINT_ID_MAX_CHARS as u64,
            ),
            (
                "ComplaintBody/properties/text/maxLength",
                COMPLAINT_TEXT_MAX_CHARS as u64,
            ),
            (
                "AssetUpload/properties/agent/maxLength",
                ASSET_AGENT_ID_MAX_CHARS as u64,
            ),
            (
                "AssetUpload/properties/id/anyOf/0/maxLength",
                ASSET_ID_MAX_CHARS as u64,
            ),
            (
                "AssetUpload/properties/name/maxLength",
                ASSET_NAME_MAX_CHARS as u64,
            ),
            (
                "BrowserForward/properties/server/maxLength",
                FORWARD_SERVER_MAX_CHARS as u64,
            ),
            (
                "BrowserForward/properties/token/minLength",
                FORWARD_TOKEN_MIN_CHARS as u64,
            ),
            (
                "BrowserForward/properties/token/maxLength",
                FORWARD_TOKEN_MAX_CHARS as u64,
            ),
            (
                "BrowserForward/properties/path/maxLength",
                FORWARD_PATH_MAX_CHARS as u64,
            ),
            (
                "BrowserForward/properties/port/minimum",
                FORWARD_PORT_MIN as u64,
            ),
            (
                "LinuxVMSettings/properties/memoryBytes/minimum",
                VM_MEMORY_MIN_BYTES as u64,
            ),
            (
                "LinuxVMSettings/properties/memoryBytes/maximum",
                VM_MEMORY_MAX_BYTES,
            ),
            (
                "LinuxVMSettings/properties/systemDiskBytes/minimum",
                VM_SYSTEM_DISK_MIN_BYTES as u64,
            ),
            (
                "LinuxVMSettings/properties/systemDiskBytes/maximum",
                VM_SYSTEM_DISK_MAX_BYTES,
            ),
            (
                "LinuxVMSettings/properties/dataDiskBytes/minimum",
                VM_DATA_DISK_MIN_BYTES as u64,
            ),
            (
                "LinuxVMSettings/properties/dataDiskBytes/maximum",
                VM_DATA_DISK_MAX_BYTES,
            ),
            (
                "AccountStartThrottleRequest/properties/burst/maximum",
                crate::shared_values::ACCOUNT_START_BURST_MAX,
            ),
            (
                "AccountStartThrottleRequest/properties/pauseSeconds/maximum",
                crate::shared_values::ACCOUNT_START_PAUSE_MAX_SECONDS,
            ),
            (
                "AccountStartThrottleRequest/properties/maxRunningTurns/maximum",
                crate::shared_values::ACCOUNT_RUNNING_TURNS_MAX,
            ),
            (
                "DefaultAccountGroupsRequest/properties/groups/maxItems",
                crate::shared_values::ACCOUNT_GROUPS_MAX as u64,
            ),
        ];
        let additional: &[(&str, u64)] = &[
            (
                "AccountNameRequest/properties/request_id/minLength",
                ACCOUNT_MUTATION_ID_BYTES as u64,
            ),
            (
                "DeleteAccountRequest/properties/request_id/minLength",
                ACCOUNT_MUTATION_ID_BYTES as u64,
            ),
            (
                "LoginRequest/properties/login_id/minLength",
                ACCOUNT_LOGIN_ID_BYTES as u64,
            ),
            (
                "ClaudeCancelRequest/properties/login_id/minLength",
                ACCOUNT_LOGIN_ID_BYTES as u64,
            ),
            (
                "AccountStartThrottleRequest/properties/burst/minimum",
                crate::shared_values::ACCOUNT_START_BURST_MIN,
            ),
            (
                "AccountStartThrottleRequest/properties/pauseSeconds/minimum",
                crate::shared_values::ACCOUNT_START_PAUSE_MIN_SECONDS,
            ),
            (
                "AccountStartThrottleRequest/properties/maxRunningTurns/minimum",
                crate::shared_values::ACCOUNT_RUNNING_TURNS_MIN,
            ),
            (
                "LinuxVMSettings/properties/cpus/maximum",
                crate::shared_values::MAX_SAFE_INTEGER as u64,
            ),
            ("BrowserForward/properties/port/maximum", u16::MAX as u64),
        ];
        for (path, expected) in additional {
            let pointer = format!("/components/schemas/{path}");
            assert_eq!(
                document.pointer(&pointer).and_then(|value| value.as_u64()),
                Some(*expected),
                "{pointer}"
            );
        }
        for (path, expected) in checks {
            let pointer = format!("/components/schemas/{path}");
            assert_eq!(
                document.pointer(&pointer).and_then(|value| value.as_u64()),
                Some(*expected),
                "{pointer}"
            );
        }
        assert_eq!(
            document
                .pointer("/components/schemas/BrowserForward/properties/bridgeId/pattern")
                .and_then(|value| value.as_str()),
            Some(format!("^[0-9a-f]{{{FORWARD_BRIDGE_ID_BYTES}}}$").as_str())
        );
    }
}
