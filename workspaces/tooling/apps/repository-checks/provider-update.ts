// Compose the existing provider owners; installation/delivery of a CLI stays explicit.
import { execFileSync } from "node:child_process";
import { readFileSync, writeFileSync } from "node:fs";
import { createRequire } from "node:module";
import { fileURLToPath } from "node:url";
import path from "node:path";
import { checkInstalledCodexDrift } from "./generate-provider-contracts.ts";
import { sdkExportDrift } from "../../../providers/apps/claude-bridge/sdk-drift.ts";

const root = fileURLToPath(new URL("../../../..", import.meta.url));
const bridge = "workspaces/providers/apps/claude-bridge";
const run = (command: string, args: string[]) =>
  execFileSync(command, args, { cwd: root, stdio: "inherit" });
const [provider, version, flag, sha, ...extra] = process.argv.slice(2);
const usage =
  "Usage: just provider-update <provider> <exact-version> [--reviewed-sha <sha>]";
if (
  !version ||
  !/^\d+\.\d+\.\d+(?:[-+][a-zA-Z0-9.-]+)?$/.test(version) ||
  extra.length ||
  (flag !== undefined &&
    (provider !== "codex" ||
      flag !== "--reviewed-sha" ||
      !sha ||
      !/^[a-f0-9]{64}$/.test(sha)))
)
  throw new Error(usage);
try {
  switch (provider) {
    case "codex":
      await checkInstalledCodexDrift(root, undefined, version, sha);
      if (!sha) break;
      run("pnpm", [
        "exec",
        "oxfmt",
        "workspaces/tooling/apps/repository-checks/provider-contract-sources.ts",
      ]);
      run("pnpm", ["contracts:vendor:refresh"]);
      run("pnpm", ["contracts:generate"]);
      run("pnpm", ["contracts:check"]);
      run("just", ["provider-drift"]);
      run("node", [
        "workspaces/tooling/apps/repository-checks/clippy-ratchet.ts",
        "--exclude-native",
      ]);
      break;
    case "claude": {
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
      break;
    }
    default:
      throw new Error(usage);
  }
} finally {
  console.log("Changed files (including pre-existing changes):");
  run("git", ["diff", "--name-only", "HEAD"]);
  run("git", ["ls-files", "--others", "--exclude-standard"]);
}
