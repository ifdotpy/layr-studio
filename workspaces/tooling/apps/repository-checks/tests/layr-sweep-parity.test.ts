import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { readFileSync } from "node:fs";
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

// Undefined when the retest env sets no job count: jobs do not change the unit hashes, and a
// focused `layr retest run` must keep its own count.
export function retestEnvJobsMismatch(env: unknown): string | undefined {
  if (Object.hasOwn((env ?? {}) as object, "CARGO_BUILD_JOBS"))
    return "retest env sets CARGO_BUILD_JOBS; a focused run would take the gate count";
  return undefined;
}

// Undefined when worker-env.sh exports the owner CARGO_BUILD_JOBS (gate-config.ts).
export function workerEnvJobsMismatch(script: string): string | undefined {
  const exported = /CARGO_BUILD_JOBS=(\d+)/.exec(script)?.[1];
  if (exported !== String(CARGO_BUILD_JOBS))
    return `worker-env.sh CARGO_BUILD_JOBS ${exported ?? "missing"} differs from gate-config ${CARGO_BUILD_JOBS}`;
  return undefined;
}

test("the retest env in Cargo.toml sets no job count", () => {
  assert.equal(retestEnvJobsMismatch(retestMetadata().env), undefined);
});

test("worker-env.sh exports the focused CARGO_BUILD_JOBS owner", () => {
  const script = readFileSync(
    fileURLToPath(new URL("../../../worker-env.sh", import.meta.url)),
    "utf8",
  );
  assert.equal(workerEnvJobsMismatch(script), undefined);
});

test("a retest env with a job count fails the parity check", () => {
  assert.match(
    retestEnvJobsMismatch({ CARGO_BUILD_JOBS: "8" }) ?? "",
    /sets CARGO_BUILD_JOBS/,
  );
});

test("a worker-env job count other than the owner fails the parity check", () => {
  assert.match(
    workerEnvJobsMismatch(`CARGO_BUILD_JOBS=${CARGO_BUILD_JOBS + 1}`) ?? "",
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
