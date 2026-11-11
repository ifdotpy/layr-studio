import { fileURLToPath } from "node:url";

// Cargo compilation jobs (a job count, including nested builders) for the gate steps that
// facade.ts runs. One gate runs per machine (layr retest gate, studio-ops wrappers); measured
// 2026-11-01: 8 jobs save 20 to 30 s against 4.
export const GATE_CARGO_BUILD_JOBS = 8;
// Cargo compilation jobs (a job count) for everything else: focused builds and tooling. The
// justfile exports it to every recipe, worker-env.sh repeats it (tests/layr-sweep-parity.test.ts
// checks that). AGENTS.md: focused builds use -j 4.
export const CARGO_BUILD_JOBS = 4;
// Grace for an owned process group to exit after TERM before KILL, milliseconds.
export const PROCESS_STOP_GRACE_MS = 500;
// Hard command deadline for ordinary gate nodes, milliseconds (browser has its own cap too).
export const GATE_COMMAND_TIMEOUT_MS = 1_800_000;
// Machine-wide lock for the full gates (test-fast, test-comprehensive): a folder with a
// pid file inside. The wrapper scripts in studio-ops use the same path. Purpose: two full
// gates on one machine push the load over 120 and both fail on timeouts.
export const GATE_LOCK_PATH = "/tmp/studio-gate.lock";
// Interval between progress lines while a gate waits for the lock, milliseconds.
export const GATE_LOCK_NOTICE_MS = 60_000;
// Interval between lock checks while a gate waits, milliseconds.
export const GATE_LOCK_POLL_MS = 1_000;
// Environment variable that sets the maximum wait for the lock, in seconds. Unset: wait forever.
export const GATE_LOCK_MAX_WAIT_ENV = "STUDIO_GATE_LOCK_MAX_WAIT_S";

// Opt-in switch, "1" or unset (read by the facade only). "1" runs the Rust workspace
// nextest step through layr-retest, which runs only tests whose executed code changed.
export const RETEST_ENV = "STUDIO_RETEST";
// Real layr store root, kept for layr-retest's own layr calls while tests see an empty one.
export const RETEST_REAL_ROOT_ENV = "LAYR_GATE_REAL_ROOT";

// Tests inherit host tools, but agent session settings must come from their fixtures.
export function scrubAgentEnvironment<T extends NodeJS.ProcessEnv>(
  environment: T,
  bin = "",
  args: string[] = [],
): T {
  const testProcess =
    bin === "cargo" && (args[0] === "test" || args[0] === "nextest");
  for (const key of Object.keys(environment))
    if (
      /^(CLAUDE|STUDIO_|AI_AGENT$)/.test(key) ||
      (testProcess && /^LAYR_(SOCKET|DIRECT|ROOT)$/.test(key))
    )
      delete environment[key];
  return environment;
}

// Just and CI consume this owner instead of copying the compilation budget.
if (process.argv[1] === fileURLToPath(import.meta.url))
  process.stdout.write(`${CARGO_BUILD_JOBS}\n`);
// Native WebView tests run separately on the prepared native CI builder.
export const NATIVE_PACKAGE = "studio-native-client";
// Rust workspace build of the gate, as cargo arguments. Owner of the command: the `sweep` list
// in the root Cargo.toml must hold it (tests/layr-sweep-parity.test.ts). `--tests` prepares
// library test harnesses too. Explicit `--lib` also builds proc macros as normal root units,
// changing their dependants from the host units used by nextest. Keep normal binaries and
// examples for the supervisor, contracts and draft HTTP fixtures.
export const RUST_WORKSPACE_BUILD_ARGS = [
  "build",
  "--workspace",
  "--exclude",
  NATIVE_PACKAGE,
  "--tests",
  "--bins",
  "--examples",
  "--locked",
] as const;
// Clippy of the gate (clippy-ratchet.ts), as cargo arguments without the job count and the
// message format: these do not change the units. The `sweep` list in the root Cargo.toml holds
// it too, so the layr cache keeps the checked units and a new line checks only its own crates.
export const RUST_CLIPPY_ARGS = [
  "clippy",
  "--workspace",
  "--exclude",
  NATIVE_PACKAGE,
  "--all-targets",
  "--locked",
  "--",
  "-D",
  "warnings",
  "--cap-lints",
  "warn",
  "-W",
  "clippy::struct_excessive_bools",
  "-W",
  "clippy::option_option",
  "-W",
  "clippy::redundant_clone",
  "-W",
  "clippy::needless_pass_by_value",
  "-W",
  "clippy::disallowed_methods",
] as const;
// The Clippy configuration of the ratchet, relative to the repository root (cargo runs clippy
// there). Clippy records the variable in each unit: an absolute path names one checkout, so a
// unit taken into another line would be checked again. The retest env in Cargo.toml sets it too.
export const CLIPPY_CONF_DIR =
  "workspaces/tooling/apps/repository-checks/clippy-ratchet";
// The compile witness of the provider contracts (tests/provider-cargo-mutations.test.ts) checks
// runtime-schema in a copy of the repository, as these cargo arguments, into its own folder below
// target. The `sweep` list in the root Cargo.toml holds both, so the layr cache keeps the
// dependency units warm and a new line does not compile them for the first run.
export const PROVIDER_CONTRACT_TARGET_DIR = "provider-contract-compile";
export const PROVIDER_CONTRACT_CHECK_ARGS = [
  "check",
  "--offline",
  "-p",
  "studio-runtime-schema",
] as const;
