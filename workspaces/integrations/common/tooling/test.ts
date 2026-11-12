import { execFileSync } from "node:child_process";
import path from "node:path";
import { discoverProviders, root } from "./discover.ts";
const files = discoverProviders().flatMap((owner) =>
  owner.contractTests.map((file) => path.join(owner.folder, file)),
);
execFileSync(process.execPath, ["--test", ...files], {
  cwd: root,
  stdio: "inherit",
});
