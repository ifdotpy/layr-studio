// These fixture files are sanitized copies selected by fixtures/extract.ts.
pub const FIXTURES: &[(&str, &[u8])] = &[
    (
        "synthetic-delta-small",
        include_bytes!("synthetic-delta-small.json"),
    ),
    (
        "synthetic-delta-large",
        include_bytes!("synthetic-delta-large.json"),
    ),
    ("codex-item", include_bytes!("codex-item.json")),
    ("codex-tool", include_bytes!("codex-tool.json")),
    ("codex-usage", include_bytes!("codex-usage.json")),
    ("claude-message", include_bytes!("claude-message.json")),
    ("claude-tool", include_bytes!("claude-tool.json")),
    ("claude-usage", include_bytes!("claude-usage.json")),
    ("agent-small", include_bytes!("agent-small.json")),
    ("agent-large", include_bytes!("agent-large.json")),
    ("monitor", include_bytes!("monitor.json")),
    ("task", include_bytes!("task.json")),
    ("work", include_bytes!("work.json")),
    ("agent-summary", include_bytes!("agent-summary.json")),
    ("room-tail", include_bytes!("room-tail.json")),
    ("transcript", include_bytes!("transcript.json")),
];
