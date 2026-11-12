import { execFileSync } from "node:child_process";
import path from "node:path";
import { discoverProviders, root } from "./discover.ts";
const [id, ...args] = process.argv.slice(2);
const owner = discoverProviders().find((owner) => owner.id === id);
if (!owner) throw new Error("Unknown integration: " + id);
execFileSync(
  process.execPath,
  [path.join(owner.folder, owner.drift), ...args],
  { cwd: root, stdio: "inherit" },
);
