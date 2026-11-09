// Manual CLI boundary fixture; build the Rust example first. No workload runs.
import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import {
  mkdtemp,
  mkdir,
  readFile,
  readdir,
  rm,
  symlink,
  writeFile,
} from "node:fs/promises";
import { createServer } from "node:http";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import type { components } from "../../../client/apps/web/src/generated/api.ts";

const binary = resolve(
  process.argv[2] ?? "target/debug/examples/storage_benchmark",
);
const root = await mkdtemp(join(tmpdir(), "storage-benchmark-guard-"));
const live = join(root, "live");
const sentinel = "not a database: must be refused before SQLite opens it";
const names = [
  "canvas.sqlite3",
  "analytics.sqlite3",
  "canvas.sqlite3-wal",
  "analytics.sqlite3-shm",
];
await mkdir(live);
await mkdir(join(live, "child"));
for (const name of names) await writeFile(join(live, name), sentinel);
const desktop = { stateDir: live } satisfies Pick<
  components["schemas"]["DesktopResponse"],
  "stateDir"
>;
const server = createServer((request, response) => {
  assert.equal(request.url, "/api/desktop");
  response.setHeader("Content-Type", "application/json");
  response.end(JSON.stringify(desktop));
});
try {
  await new Promise<void>((done) => server.listen(0, "127.0.0.1", done));
  const address = server.address();
  assert(address && typeof address !== "string");
  const url = `http://127.0.0.1:${address.port}`;
  async function refused(label: string, state: string, discover = true) {
    const args = ["--smoke", "--state", state];
    if (discover) args.push("--server", url);
    const child = spawn(binary, args, {
      stdio: ["ignore", "pipe", "pipe"],
      signal: AbortSignal.timeout(5000),
    });
    let stderr = "";
    child.stderr.setEncoding("utf8").on("data", (chunk: string) => {
      stderr += chunk;
    });
    const code = await new Promise<number | null>((done, reject) => {
      child.on("error", reject);
      child.on("exit", done);
    });
    assert.notEqual(code, 0, label);
    assert(
      stderr.includes(discover ? "refusing live state" : "requires --server"),
      `${label}: ${stderr}`,
    );
    console.log(`${label}: refused before SQLite opened a source`);
  }
  await refused("equal", live);
  await refused("descendant", join(live, "child"));
  await refused("ancestor", root);
  await refused("unresolved", root, false);
  for (const [index, linked] of names.entries()) {
    const copy = join(root, `copy-${index}`);
    await mkdir(copy);
    for (const name of ["canvas.sqlite3", "analytics.sqlite3"]) {
      if (name !== linked) await writeFile(join(copy, name), sentinel);
    }
    await symlink(join(live, linked), join(copy, linked));
    await refused(`symlinked ${linked}`, copy);
  }
  assert.deepEqual((await readdir(live)).sort(), [...names, "child"].sort());
  for (const name of names)
    assert.equal(await readFile(join(live, name), "utf8"), sentinel);
  console.log("live fixture unchanged");
} finally {
  await new Promise<void>((done) => server.close(() => done()));
  await rm(root, { recursive: true, force: true });
}
