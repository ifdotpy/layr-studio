import { availableParallelism, loadavg } from "node:os";
import { fileURLToPath } from "node:url";

// Maximum Cargo compilation jobs per gate, a job count (including nested builders).
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

// Load adaptation for a shared machine. Rule: factor = clamp(1 minute load / CPU count, 1, max).
// Parallelism (nextest test threads, Playwright workers) divides by the factor; time limits
// (nextest slow-timeout, Playwright test timeout, command deadline) multiply by it.
// Purpose: other sessions keep the load far above the CPU count, and fixed limits then fail.
// Unit: the factor is a unitless ratio.
export const GATE_LOAD_FACTOR_MAX = 4;
// Environment variable that carries the factor to the Playwright configuration.
export const GATE_LOAD_FACTOR_ENV = "STUDIO_GATE_LOAD_FACTOR";
// Nextest ci profile slow-timeout at factor 1: period in seconds and period count before
// termination. .config/nextest.toml must state the same values (checked by a test).
export const NEXTEST_SLOW_PERIOD_S = 30;
export const NEXTEST_TERMINATE_AFTER = 3;

export interface GateLoad {
  load: number;
  cpus: number;
  factor: number;
}

export function gateLoad(
  load = loadavg()[0]!,
  cpus = availableParallelism(),
): GateLoad {
  const ratio = load / cpus;
  const factor = Number.isFinite(ratio)
    ? Math.min(GATE_LOAD_FACTOR_MAX, Math.max(1, ratio))
    : 1;
  return { load, cpus, factor };
}

export function scaledByLoad(base: number, { factor }: GateLoad) {
  return Math.ceil(base * factor);
}

export function dividedByLoad(base: number, { factor }: GateLoad) {
  return Math.max(1, Math.floor(base / factor));
}

// One line per step, so a failure log shows the conditions.
export function gateLoadLine(label: string, gate: GateLoad, details: string[]) {
  return `gate load: ${label}: load=${gate.load.toFixed(1)} cpus=${gate.cpus} factor=${gate.factor.toFixed(2)}${details.map((d) => ` ${d}`).join("")}\n`;
}

// Tests inherit host tools, but agent session settings must come from their fixtures.
export function scrubAgentEnvironment<T extends NodeJS.ProcessEnv>(
  environment: T,
): T {
  for (const key of Object.keys(environment))
    if (/^(CLAUDE|STUDIO_|AI_AGENT$)/.test(key)) delete environment[key];
  return environment;
}

// Just and CI consume this owner instead of copying the compilation budget.
if (process.argv[1] === fileURLToPath(import.meta.url))
  process.stdout.write(`${CARGO_BUILD_JOBS}\n`);
