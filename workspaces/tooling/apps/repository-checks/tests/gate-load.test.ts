import test from "node:test";
import assert from "node:assert/strict";
import { mkdtempSync, readFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import {
  GATE_LOAD_FACTOR_MAX,
  dividedByLoad,
  gateLoad,
  gateLoadLine,
  scaledByLoad,
} from "../gate-config.ts";
import { NEXTEST_SLOW_TIMEOUT_LINE, adaptNextestArgs } from "../facade.ts";
import { NEXTEST_SLOW_PERIOD_S } from "../gate-config.ts";

const root = path.resolve(import.meta.dirname, "../../../../..");

test("load below and at the CPU count leaves the factor at 1", () => {
  assert.equal(gateLoad(3, 10).factor, 1);
  assert.equal(gateLoad(10, 10).factor, 1);
  assert.equal(dividedByLoad(10, gateLoad(3, 10)), 10);
  assert.equal(scaledByLoad(30, gateLoad(10, 10)), 30);
});

test("load above the CPU count lowers threads and raises limits", () => {
  const gate = gateLoad(20, 10);
  assert.equal(gate.factor, 2);
  assert.equal(dividedByLoad(10, gate), 5);
  assert.equal(scaledByLoad(30, gate), 60);
  assert.equal(dividedByLoad(1, gate), 1);
});

test("the factor is capped far above the CPU count", () => {
  const gate = gateLoad(165, 10);
  assert.equal(gate.factor, GATE_LOAD_FACTOR_MAX);
  assert.equal(dividedByLoad(10, gate), Math.floor(10 / GATE_LOAD_FACTOR_MAX));
  assert.equal(scaledByLoad(30, gate), 30 * GATE_LOAD_FACTOR_MAX);
});

test("the printed line shows load, factor, threads and timeouts", () => {
  assert.equal(
    gateLoadLine("Rust nextest", gateLoad(40, 10), [
      "test-threads=2",
      "slow-timeout=120s x3",
    ]),
    "gate load: Rust nextest: load=40.0 cpus=10 factor=4.00 test-threads=2 slow-timeout=120s x3\n",
  );
});

test("nextest args get threads and a scaled config copy", () => {
  const dir = mkdtempSync(path.join(tmpdir(), "gate-load-"));
  const file = path.join(dir, "nextest.toml");
  const config = `[profile.ci]\n${NEXTEST_SLOW_TIMEOUT_LINE(NEXTEST_SLOW_PERIOD_S)}\n`;
  const adapted = adaptNextestArgs(
    ["nextest", "run", "--workspace", "--profile", "ci"],
    gateLoad(20, 10),
    config,
    file,
  );
  assert.deepEqual(adapted.args.slice(0, 2), ["nextest", "run"]);
  assert.deepEqual(adapted.args.slice(-3), ["--workspace", "--profile", "ci"]);
  assert.ok(adapted.args.includes("--config-file"));
  assert.match(
    readFileSync(file, "utf8"),
    /period = "60s", terminate-after = 3/,
  );
  assert.match(adapted.details.join(" "), /slow-timeout=60s x3/);
});

test(".config/nextest.toml states the base slow-timeout of the gate owner", () => {
  assert.ok(
    readFileSync(path.join(root, ".config/nextest.toml"), "utf8").includes(
      NEXTEST_SLOW_TIMEOUT_LINE(NEXTEST_SLOW_PERIOD_S),
    ),
  );
});
