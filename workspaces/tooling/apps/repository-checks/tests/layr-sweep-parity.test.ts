import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import test from "node:test";
import { isDeepStrictEqual } from "node:util";
import {
  CARGO_BUILD_JOBS,
  CLIPPY_CONF_DIR,
  PROVIDER_CONTRACT_CHECK_ARGS,
  PROVIDER_CONTRACT_TARGET_DIR,
  RUST_CLIPPY_ARGS,
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

// The sweep list of the gate: its Rust workspace build, its clippy and the provider contract
// compile witness in its own target folder.
// The cargo arguments of the web client's release WASM build (build-client-wasm.ts reads the same file).
const CLIENT_WASM_BUILD_ARGS: string[] = JSON.parse(
  readFileSync(
    new URL(
      "../../../../client/apps/web/scripts/client-wasm-build-args.json",
      import.meta.url,
    ),
    "utf8",
  ),
);

const GATE_SWEEP = [
  [...RUST_WORKSPACE_BUILD_ARGS],
  [...RUST_CLIPPY_ARGS],
  {
    target_dir: PROVIDER_CONTRACT_TARGET_DIR,
    args: [...PROVIDER_CONTRACT_CHECK_ARGS],
  },
  [...CLIENT_WASM_BUILD_ARGS],
];

// Undefined when the list holds exactly the gate's sweep commands; otherwise the difference.
export function sweepMismatch(sweep: unknown): string | undefined {
  if (!Array.isArray(sweep))
    return "missing [workspace.metadata.layr-retest] sweep list";
  if (!isDeepStrictEqual(sweep, GATE_SWEEP))
    return `sweep ${JSON.stringify(sweep)} differs from the gate command ${JSON.stringify(GATE_SWEEP)}`;
  return undefined;
}

test("the sweep list in Cargo.toml is the gate's Rust workspace build, clippy, contract check and WASM build", () => {
  assert.equal(sweepMismatch(sweepInManifest()), undefined);
});

test("a sweep list without the contract check fails the parity check", () => {
  assert.match(
    sweepMismatch([[...RUST_WORKSPACE_BUILD_ARGS], [...RUST_CLIPPY_ARGS]]) ??
      "",
    /differs from the gate command/,
  );
});

// Undefined when the retest env gives clippy the ratchet's configuration, so the sweep listing
// sees the gate's clippy units as fresh.
export function clippyConfMismatch(env: unknown): string | undefined {
  const value = (env as Record<string, unknown> | undefined)?.CLIPPY_CONF_DIR;
  if (value !== CLIPPY_CONF_DIR)
    return `retest env CLIPPY_CONF_DIR ${JSON.stringify(value)} differs from ${CLIPPY_CONF_DIR}`;
  return undefined;
}

test("the retest env gives clippy the ratchet configuration", () => {
  assert.equal(clippyConfMismatch(retestMetadata().env), undefined);
});

test("a retest env without the ratchet configuration fails the parity check", () => {
  assert.match(clippyConfMismatch({}) ?? "", /differs from/);
});

test("a sweep list without the clippy command fails the parity check", () => {
  assert.match(
    sweepMismatch([[...RUST_WORKSPACE_BUILD_ARGS], GATE_SWEEP[2]]) ?? "",
    /differs from the gate command/,
  );
});

test("a sweep list without --locked fails the parity check", () => {
  const bad = [
    RUST_WORKSPACE_BUILD_ARGS.filter((arg) => arg !== "--locked"),
    [...RUST_CLIPPY_ARGS],
    GATE_SWEEP[2],
  ];
  assert.match(sweepMismatch(bad) ?? "", /differs from the gate command/);
});

test("a sweep list with an extra command fails the parity check", () => {
  const bad = [...GATE_SWEEP, ["test", "--workspace"]];
  assert.match(sweepMismatch(bad) ?? "", /differs from the gate command/);
});
