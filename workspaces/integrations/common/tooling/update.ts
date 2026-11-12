import { execFileSync } from "node:child_process";
import path from "node:path";
import { discoverProviders, root } from "./discover.ts";
const [id, ...args] = process.argv.slice(2);
const owner = discoverProviders().find((owner) => owner.id === id);
if (!owner) throw new Error("Unknown integration: " + id);
try {
  execFileSync(
    process.execPath,
    [path.join(owner.folder, owner.update), ...args],
    { cwd: root, stdio: "inherit" },
  );
} finally {
  for (const args of [
    ["diff", "--name-only", "HEAD"],
    ["ls-files", "--others", "--exclude-standard"],
  ])
    execFileSync("git", args, { cwd: root, stdio: "inherit" });
}
