mod openapi_build;

// The TypeScript generator owns freshness; Cargo must never compile stale wire
// types after a committed vendor schema changes. No CLI, network or state is used.
// Cargo stores rerun-if-changed paths as printed. Absolute paths differ in every layr
// line and mark this script dirty, so all paths here are relative to the package.
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let manifest = std::path::PathBuf::from(std::env::var("CARGO_MANIFEST_DIR")?);
    let root = manifest
        .ancestors()
        .nth(4)
        .ok_or("invalid manifest directory")?;
    let relative_root = "../../../..";
    let generator = "workspaces/integrations/common/tooling/contracts.ts";
    for file in [
        generator,
        "workspaces/integrations/providers/codex/vendor/source.ts",
        "workspaces/runtime/packages/runtime-schema/src/codex_generated.rs",
        "workspaces/integrations/providers/codex/vendor/schemas/selected",
    ] {
        println!("cargo:rerun-if-changed={relative_root}/{file}");
    }
    let mut child = std::process::Command::new("node")
        .arg(root.join(generator))
        .args(["--check", "--rust-only", "--cargo-inputs"])
        .stdout(std::process::Stdio::piped())
        .spawn()?;
    // Cargo runs this check on its dedicated build-script process, outside the
    // runtime. Bound the checker without introducing a runtime dependency.
    const CHECK_DEADLINE: std::time::Duration = std::time::Duration::from_secs(30);
    const CHECK_INTERVAL: std::time::Duration = std::time::Duration::from_millis(10);
    let started = std::time::Instant::now();
    let status = loop {
        if let Some(status) = child.try_wait()? {
            break status;
        }
        if started.elapsed() >= CHECK_DEADLINE {
            child.kill()?;
            child.wait()?;
            return Err("Provider wire contract freshness check timed out".into());
        }
        std::thread::park_timeout(CHECK_INTERVAL);
    };
    // The generator prints absolute paths; re-emit them relative to the package.
    let mut inputs = String::new();
    std::io::Read::read_to_string(
        &mut child.stdout.take().ok_or("missing generator output")?,
        &mut inputs,
    )?;
    // The generator reports its own path, so that path gives the root it printed.
    let paths = inputs
        .lines()
        .map(|line| line.strip_prefix("cargo:rerun-if-changed="))
        .collect::<Option<Vec<_>>>()
        .ok_or("unexpected generator output")?;
    let printed_root = paths
        .iter()
        .find_map(|path| path.strip_suffix(generator))
        .ok_or("generator did not report its own path")?;
    for path in paths {
        let file = path
            .strip_prefix(printed_root)
            .ok_or("generator input outside the repository")?;
        println!("cargo:rerun-if-changed={relative_root}/{file}");
    }
    if !status.success() {
        return Err(
            "Provider wire contracts are stale; run pnpm run contracts:vendor:generate".into(),
        );
    }
    for (source, target) in [
        (
            "workspaces/integrations/providers/codex/vendor/schemas/selected/ThreadTokenUsageUpdatedNotification.json",
            "codex-usage.json",
        ),
        (
            "workspaces/integrations/providers/codex/vendor/schemas/selected/CodexErrorInfo.json",
            "codex-errors.json",
        ),
        (
            "workspaces/integrations/providers/codex/vendor/schemas/selected/lifecycle.schema.json",
            "codex-lifecycle.json",
        ),
        (
            "workspaces/integrations/providers/claude/vendor/schemas/claude-rate-limit.schema.json",
            "claude-rate-limit.json",
        ),
    ] {
        println!("cargo:rerun-if-changed={relative_root}/{source}");
        std::fs::copy(
            root.join(source),
            std::path::PathBuf::from(std::env::var("OUT_DIR")?).join(target),
        )?;
    }
    openapi_build::generate()?;
    Ok(())
}
