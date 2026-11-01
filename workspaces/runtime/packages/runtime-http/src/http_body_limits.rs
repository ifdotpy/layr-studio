//! HTTP API request-body limits.

/// Largest request body of an endpoint without its own limit, in bytes (256 KiB).
pub(crate) const DEFAULT_BODY_LIMIT: usize = 262_144;
/// Largest `/api/assets` upload, in bytes (28 MiB).
pub(crate) const ASSET_BODY_LIMIT: usize = 29_360_128;
/// Largest `/api/projects` request, in bytes (6 MiB).
pub(crate) const PROJECTS_BODY_LIMIT: usize = 6_291_456;
/// Largest command body, in bytes: a surrogate pair takes 12 escaped bytes per code point.
pub(crate) const COMMAND_BODY_LIMIT: usize =
    studio_runtime_schema::shared_values::MAX_DRAFT_TEXT_CODEPOINTS * 12 + 65_536;
/// Largest draft push in bytes: two versions, each needing 14 bytes per nested escaped code point.
pub(crate) const DRAFT_BODY_LIMIT: usize =
    studio_runtime_schema::shared_values::MAX_DRAFT_TEXT_CODEPOINTS * 14 * 2 + 65_536;
/// Deadline for reading one complete request body, in seconds.
pub(crate) const REQUEST_BODY_READ_TIMEOUT_SECONDS: u64 = 10;

pub(crate) fn request_limit(path: &str) -> usize {
    if studio_runtime_services::federation_limits::FEDERATION_PATHS.contains(&path) {
        return studio_runtime_services::federation_limits::FEDERATION_BODY_LIMIT;
    }
    match path {
        "/api/assets" => ASSET_BODY_LIMIT,
        "/api/projects" => PROJECTS_BODY_LIMIT,
        "/api/commands" => COMMAND_BODY_LIMIT,
        "/api/sync/drafts" => DRAFT_BODY_LIMIT,
        _ => DEFAULT_BODY_LIMIT,
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn draft_and_command_limits_cover_escaped_supplementary_characters() {
        let max = studio_runtime_schema::shared_values::MAX_DRAFT_TEXT_CODEPOINTS;
        let escaped = "\\ud83d\\ude00".repeat(max);
        let command = format!(r#"{{"text":"{escaped}"}}"#);
        let nested = serde_json::to_string(&command).expect("nested JSON");
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&command).expect("command")["text"]
                .as_str()
                .expect("text")
                .chars()
                .count(),
            max
        );
        assert!(super::COMMAND_BODY_LIMIT >= command.len());
        assert!(super::DRAFT_BODY_LIMIT >= 2 * nested.len());
    }
}
