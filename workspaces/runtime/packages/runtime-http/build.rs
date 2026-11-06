#![allow(
    clippy::disallowed_methods,
    reason = "build script runs outside the HTTP runtime"
)]
use std::{env, error::Error, fs, path::PathBuf};

fn main() -> Result<(), Box<dyn Error>> {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR")?);
    // Clippy does not merge parent configs: runtime/clippy.toml owns the shared policy.
    let shared_clippy = manifest_dir.join("../../clippy.toml");
    let http_clippy = manifest_dir.join("clippy.toml");
    // Relative paths: absolute ones differ in every layr line and force a rebuild.
    println!("cargo:rerun-if-changed=../../clippy.toml");
    println!("cargo:rerun-if-changed=clippy.toml");
    let shared = fs::read_to_string(shared_clippy)?;
    let http = fs::read_to_string(http_clippy)?;
    let inherited = http
        .lines()
        .filter(|line| {
            !line.contains("HTTP-only blocking filesystem guards")
                && !line.contains("HTTP handlers must use ")
        })
        .collect::<Vec<_>>()
        .join("\n")
        + "\n";
    assert_eq!(
        shared, inherited,
        "HTTP Clippy config must preserve the policy owned by runtime/clippy.toml"
    );
    Ok(())
}
