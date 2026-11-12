import { execFileSync } from "node:child_process";
import { fileURLToPath } from "node:url";
const root = fileURLToPath(new URL("../../../../..", import.meta.url));
const run = (command: string, args: string[]) =>
  execFileSync(command, args, { cwd: root, stdio: "inherit" });
import { checkInstalledCodexDrift } from "./generate.ts";
const [version, flag, sha, ...extra] = process.argv.slice(2);
if (
  !version ||
  !/^\d+\.\d+\.\d+(?:[-+][a-zA-Z0-9.-]+)?$/.test(version) ||
  extra.length ||
  (flag !== undefined &&
    (flag !== "--reviewed-sha" || !sha || !/^[a-f0-9]{64}$/.test(sha)))
)
  throw new Error("Expected exact version [--reviewed-sha <sha>]");
await checkInstalledCodexDrift(root, undefined, version, sha);
if (!sha) process.exit(0);
run("pnpm", [
  "exec",
  "oxfmt",
  "workspaces/integrations/providers/codex/vendor/source.ts",
]);
run("pnpm", ["contracts:vendor:refresh"]);
run("pnpm", ["contracts:generate"]);
run("pnpm", ["contracts:check"]);
run("just", ["provider-drift"]);
run("node", [
  "workspaces/tooling/apps/repository-checks/clippy-ratchet.ts",
  "--exclude-native",
]);
