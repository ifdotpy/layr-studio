import { execFileSync } from "node:child_process";
import { readFileSync, writeFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import path from "node:path";
const root = fileURLToPath(new URL("../../../../..", import.meta.url));
const run = (command: string, args: string[]) =>
  execFileSync(command, args, { cwd: root, stdio: "inherit" });
import { createRequire } from "node:module";
import { sdkExportDrift } from "../bridge/sdk-drift.ts";
const bridge = "workspaces/integrations/providers/claude/bridge";
const [version, ...extra] = process.argv.slice(2);
if (
  !version ||
  !/^\d+\.\d+\.\d+(?:[-+][a-zA-Z0-9.-]+)?$/.test(version) ||
  extra.length
)
  throw new Error("Expected exact SDK version");
const manifest = path.join(root, bridge, "package.json");
const require = createRequire(manifest);
const declaration = require
  .resolve("@anthropic-ai/claude-agent-sdk")
  .replace(/sdk\.mjs$/, "sdk.d.ts");
const before = readFileSync(declaration, "utf8");
writeFileSync(
  manifest,
  readFileSync(manifest, "utf8").replace(
    /("@anthropic-ai\/claude-agent-sdk": ")[^"]+"/,
    `$1${version}"`,
  ),
);
run("pnpm", ["install", "--ignore-scripts"]);
// Resolve through the live dependency link after install, avoiding Node's
// cached realpath for the previous package version.
const candidate = path.join(
  path.dirname(manifest),
  "node_modules/@anthropic-ai/claude-agent-sdk/sdk.d.ts",
);
console.log(
  JSON.stringify(
    sdkExportDrift(before, readFileSync(candidate, "utf8")),
    null,
    2,
  ),
);
run("pnpm", ["contracts:generate"]);
run("pnpm", ["contracts:check"]);
run("just", ["claude-sdk-drift", candidate]);
run("pnpm", ["--filter", "studio-claude-bridge", "typecheck"]);
