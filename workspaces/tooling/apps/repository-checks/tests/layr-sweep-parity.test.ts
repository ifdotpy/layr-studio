import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { fileURLToPath } from "node:url";
import test from "node:test";
import { isDeepStrictEqual } from "node:util";
import {
  CARGO_BUILD_JOBS,
  RUST_WORKSPACE_BUILD_ARGS,
} from "../gate-config.ts";

const manifest = fileURLToPath(
  new URL("../../../../../Cargo.toml", import.meta.url),
);

// The [workspace.metadata.layr-retest] table of the root manifest. Cargo parses the TOML.
function retestMetadata(): Record<string, unknown> {
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
  return metadata.metadata?.["layr-retest"] ?? {};
}

function sweepInManifest(): unknown {
  return retestMetadata().sweep;
}

// Undefined when the env job count equals the gate owner CARGO_BUILD_JOBS (gate-config.ts).
export function jobsMismatch(env: unknown): string | undefined {
  const jobs = (env as Record<string, unknown> | undefined)?.CARGO_BUILD_JOBS;
  if (jobs !== String(CARGO_BUILD_JOBS))
    return `env CARGO_BUILD_JOBS ${JSON.stringify(jobs)} differs from gate-config ${CARGO_BUILD_JOBS}`;
  return undefined;
}

test("the env job count in Cargo.toml is the gate-config CARGO_BUILD_JOBS", () => {
  assert.equal(jobsMismatch(retestMetadata().env), undefined);
});

test("an env job count other than the owner fails the parity check", () => {
  assert.match(
    jobsMismatch({ CARGO_BUILD_JOBS: String(CARGO_BUILD_JOBS + 1) }) ?? "",
    /differs from gate-config/,
  );
});

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
