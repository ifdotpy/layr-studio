import { execFileSync } from "node:child_process";
import path from "node:path";
import { discoverProviders, root } from "./discover.ts";
if (process.argv.includes("--cargo-inputs"))
  for (const file of [
    "workspaces/integrations/common/tooling/contracts.ts",
    "workspaces/integrations/common/tooling/discover.ts",
    ...discoverProviders().map((owner) =>
      path.relative(root, path.join(owner.folder, "package.json")),
    ),
  ])
    process.stdout.write(
      "cargo:rerun-if-changed=" + path.join(root, file) + "\n",
    );
for (const owner of discoverProviders())
  execFileSync(
    process.execPath,
    [path.join(owner.folder, owner.contracts), ...process.argv.slice(2)],
    { cwd: root, stdio: "inherit" },
  );
