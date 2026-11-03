import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { once } from "node:events";
import { mkdtempSync, readdirSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";

import {
  fixtureProcessGroupExists,
  reapFixtureProcessGroups,
  registerFixtureProcessGroup,
} from "./fixture-process-group.ts";

test("stopping a fixture process group reaps its descendant processes", async () => {
  if (process.platform === "win32") return;

  const registryDirectory = mkdtempSync(join(tmpdir(), "fixture-groups-"));
  const child = spawn(
    process.execPath,
    [
      fileURLToPath(
        new URL("./fixture-process-group.child.ts", import.meta.url),
      ),
      "parent",
    ],
    { detached: true, stdio: ["ignore", "pipe", "ignore"] },
  );

  try {
    const [output] = await once(child.stdout, "data");
    const grandchildPid = Number(String(output));
    assert.ok(Number.isSafeInteger(grandchildPid));
    assert.equal(fixtureProcessGroupExists(child.pid!), true);
    registerFixtureProcessGroup(registryDirectory, child.pid!);

    await reapFixtureProcessGroups(registryDirectory);

    assert.equal(fixtureProcessGroupExists(child.pid!), false);
    assert.throws(() => process.kill(grandchildPid, 0), { code: "ESRCH" });
    assert.deepEqual(readdirSync(registryDirectory), []);
  } finally {
    if (child.pid !== undefined && fixtureProcessGroupExists(child.pid!))
      await reapFixtureProcessGroups(registryDirectory);
    rmSync(registryDirectory, { recursive: true, force: true });
  }
});

test("reaper ignores a registry entry for a reused process id", async () => {
  if (process.platform !== "linux") return;

  const registryDirectory = mkdtempSync(
    join(tmpdir(), "fixture-groups-stale-"),
  );
  try {
    writeFileSync(
      join(registryDirectory, `${process.pid}.json`),
      JSON.stringify({ pid: process.pid, startTime: "not-this-process" }),
    );

    await reapFixtureProcessGroups(registryDirectory);

    assert.equal(process.kill(process.pid, 0), true);
    assert.deepEqual(readdirSync(registryDirectory), []);
  } finally {
    rmSync(registryDirectory, { recursive: true, force: true });
  }
});

test("reaper stops fixture descendants after the direct process exits", async () => {
  if (process.platform === "win32") return;

  const registryDirectory = mkdtempSync(
    join(tmpdir(), "fixture-groups-orphaned-descendant-"),
  );
  const child = spawn(
    process.execPath,
    [
      fileURLToPath(
        new URL("./fixture-process-group.child.ts", import.meta.url),
      ),
      "orphan",
    ],
    { detached: true, stdio: ["ignore", "pipe", "ignore"] },
  );

  try {
    await once(child, "spawn");
    registerFixtureProcessGroup(registryDirectory, child.pid!);
    const [output] = await once(child.stdout, "data");
    const grandchildPid = Number(String(output));
    await once(child, "close");
    assert.equal(fixtureProcessGroupExists(child.pid!), true);

    await reapFixtureProcessGroups(registryDirectory);

    assert.equal(fixtureProcessGroupExists(child.pid!), false);
    assert.throws(() => process.kill(grandchildPid, 0), { code: "ESRCH" });
    assert.deepEqual(readdirSync(registryDirectory), []);
  } finally {
    if (child.pid !== undefined && fixtureProcessGroupExists(child.pid!))
      await reapFixtureProcessGroups(registryDirectory);
    rmSync(registryDirectory, { recursive: true, force: true });
  }
});

test("a fixture provider exits when the server that started it closes its stdin", async () => {
  if (process.platform === "win32") return;

  // The control socket keeps a provider alive after its stdin ends, as in the leak.
  const control = mkdtempSync(join(tmpdir(), "fixture-provider-control-"));
  const provider = spawn(
    process.execPath,
    [
      fileURLToPath(
        new URL("./fixtures/fake-codex-app-server.ts", import.meta.url),
      ),
    ],
    {
      env: { ...process.env, FAKE_CODEX_CONTROL: control },
      stdio: ["pipe", "ignore", "ignore"],
    },
  );

  try {
    await once(provider, "spawn");
    const pid = provider.pid!;
    assert.equal(process.kill(pid, 0), true);

    // The server exits and its end of stdin closes.
    provider.stdin!.end();
    await once(provider, "exit", { signal: AbortSignal.timeout(10_000) });

    assert.throws(() => process.kill(pid, 0), { code: "ESRCH" });
  } finally {
    if (provider.exitCode === null && provider.signalCode === null)
      provider.kill("SIGKILL");
    rmSync(control, { recursive: true, force: true });
  }
});
