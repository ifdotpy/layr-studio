import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { fileURLToPath } from "node:url";
import test from "node:test";
import { isDeepStrictEqual } from "node:util";
import { RUST_WORKSPACE_BUILD_ARGS } from "../gate-config.ts";

const manifest = fileURLToPath(
  new URL("../../../../../Cargo.toml", import.meta.url),
);

// The `sweep` list of the root manifest. Cargo parses the TOML, so the test does not match text.
function sweepInManifest(): unknown {
  const metadata = JSON.parse(
    execFileSync(
      "cargo",
      [
        "metadata",
        "--no-deps",
        "--format-version",
        "1",
        "--locked",
        "--manifest-path",
        manifest,
      ],
      { encoding: "utf8", maxBuffer: 64 * 1024 * 1024 },
    ),
  );
  return metadata.metadata?.["layr-retest"]?.sweep;
}

// Undefined when the list holds exactly the gate's Rust workspace build; otherwise the difference.
export function sweepMismatch(sweep: unknown): string | undefined {
  if (!Array.isArray(sweep))
    return "missing [workspace.metadata.layr-retest] sweep list";
  const gate = [[...RUST_WORKSPACE_BUILD_ARGS]];
  if (!isDeepStrictEqual(sweep, gate))
    return `sweep ${JSON.stringify(sweep)} differs from the gate command ${JSON.stringify(gate)}`;
  return undefined;
}

test("the sweep list in Cargo.toml is the gate's Rust workspace build", () => {
  assert.equal(sweepMismatch(sweepInManifest()), undefined);
});

test("a sweep list without --locked fails the parity check", () => {
  const bad = [RUST_WORKSPACE_BUILD_ARGS.filter((arg) => arg !== "--locked")];
  assert.match(sweepMismatch(bad) ?? "", /differs from the gate command/);
});

test("a sweep list with an extra command fails the parity check", () => {
  const bad = [[...RUST_WORKSPACE_BUILD_ARGS], ["test", "--workspace"]];
  assert.match(sweepMismatch(bad) ?? "", /differs from the gate command/);
});
