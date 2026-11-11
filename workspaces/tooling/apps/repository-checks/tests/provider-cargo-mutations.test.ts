// Compile the real owning crate in a disposable source copy. This is a focused
// compiler witness, not a provider process or a second runtime/server.
import assert from "node:assert/strict";
import {
  cpSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  readdirSync,
  rmSync,
  existsSync,
  statSync,
  utimesSync,
  writeFileSync,
} from "node:fs";
import { spawnSync } from "node:child_process";
import { tmpdir } from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";
import test from "node:test";
import {
  PROVIDER_CONTRACT_CHECK_ARGS,
  PROVIDER_CONTRACT_TARGET_DIR,
} from "../gate-config.ts";

const root = fileURLToPath(new URL("../../../../..", import.meta.url));

// cpSync keeps the times of files, not of folders. A build script that watches a folder
// (runtime-schema watches src/schemas/codex) would then run again on every copy.
function keepFolderTimes(source: string, target: string) {
  if (!statSync(source).isDirectory()) return;
  for (const entry of readdirSync(source, { withFileTypes: true }))
    if (entry.isDirectory())
      keepFolderTimes(
        path.join(source, entry.name),
        path.join(target, entry.name),
      );
  const { atime, mtime } = statSync(source);
  utimesSync(target, atime, mtime);
}

/** Whole witness budget, in milliseconds. */
const WITNESS_TIMEOUT_MS = 900_000;
/** Part of the budget kept for the source copy and cleanup, in milliseconds. */
const CLEANUP_MARGIN_MS = 60_000;
test(
  "Cargo refuses stale sources and regenerated incompatible vendor types",
  { timeout: WITNESS_TIMEOUT_MS },
  async (context) => {
    // All commands share one deadline: without warm units (the `sweep` entry)
    // the first check compiles the copied crates cold and can take most of it
    // beside a gate build; the later checks are incremental and take seconds.
    const deadline = Date.now() + WITNESS_TIMEOUT_MS - CLEANUP_MARGIN_MS;
    const copy = mkdtempSync(path.join(tmpdir(), "studio-contract-compile-"));
    try {
      for (const file of [
        "Cargo.toml",
        "Cargo.lock",
        // The build flags of the checkout: the dependency units then match the ones that
        // the layr cache keeps warm (the `sweep` entry in Cargo.toml).
        ".cargo/config.toml",
        "rust-toolchain.toml",
        "package.json",
        "pnpm-lock.yaml",
        ".oxfmtrc.json",
        "workspaces/runtime/packages/runtime-schema",
        "workspaces/runtime/packages/codex-protocol",
        "workspaces/runtime/packages/db-api",
        "workspaces/runtime/packages/db-api-derive",
        "workspaces/runtime/packages/observable-codegen",
        "workspaces/runtime/packages/json",
        "workspaces/runtime/packages/request-signing",
        "workspaces/client/packages/client-domain",
        "workspaces/providers/apps/claude-bridge/codex-generated",
        "workspaces/providers/apps/claude-bridge/package.json",
        "workspaces/providers/apps/claude-bridge/rate-limit-input.ts",
        "workspaces/providers/apps/claude-bridge/usage-policy.ts",
        "workspaces/runtime/packages/runtime-http/openapi.json",
        "workspaces/tooling/apps/repository-checks/generate-provider-contracts.ts",
        "workspaces/tooling/apps/repository-checks/provider-contract-sources.ts",
      ]) {
        const target = path.join(copy, file);
        mkdirSync(path.dirname(target), { recursive: true });
        // Same file times as the checkout: cargo then finds the warm units of the unchanged
        // crates fresh, and only a mutation below makes it compile.
        cpSync(path.join(root, file), target, {
          recursive: true,
          preserveTimestamps: true,
        });
        keepFolderTimes(path.join(root, file), target);
      }
      assert.equal(existsSync(path.join(copy, "node_modules")), false);
      const manifest = readFileSync(
        path.join(copy, "Cargo.toml"),
        "utf8",
      ).replace(
        /members = \[[\s\S]*?\]/,
        'members = ["workspaces/runtime/packages/codex-protocol", "workspaces/runtime/packages/runtime-schema", "workspaces/runtime/packages/db-api", "workspaces/runtime/packages/db-api-derive", "workspaces/runtime/packages/observable-codegen", "workspaces/runtime/packages/json", "workspaces/runtime/packages/request-signing", "workspaces/client/packages/client-domain"]',
      );
      writeFileSync(path.join(copy, "Cargo.toml"), manifest);
      const env = {
        ...process.env,
        // Own target folder: the gate's Rust lane holds the lock of the shared one for
        // minutes, and each blocked check here waited out its whole timeout. It stays in
        // the checkout's target/ so dependency builds stay warm between runs, and the
        // `sweep` entry in Cargo.toml keeps them warm for a new line.
        CARGO_TARGET_DIR: path.join(
          root,
          "target",
          PROVIDER_CONTRACT_TARGET_DIR,
        ),
      };
      const run = (command: string, args: string[]) => {
        const result = spawnSync(command, args, {
          cwd: copy,
          env,
          encoding: "utf8",
          timeout: Math.max(1, deadline - Date.now()),
        });
        // A killed command has no exit code, and "not 0" would wrongly pass
        // a rejection check: every command must finish on its own.
        assert.ok(
          result.error === undefined && result.signal === null,
          `${command} ${args.join(" ")} did not finish within the witness budget: ${result.error?.message ?? result.signal}`,
        );
        return result;
      };
      const check = () =>
        run("cargo", [...PROVIDER_CONTRACT_CHECK_ARGS, "-j", "4"]);
      const generate = () => {
        const result = run(process.execPath, [
          "workspaces/tooling/apps/repository-checks/generate-provider-contracts.ts",
          "--write",
          "--rust-only",
        ]);
        assert.equal(result.status, 0, result.stdout + result.stderr);
      };
      const baseline = check();
      assert.equal(baseline.status, 0, baseline.stdout + baseline.stderr);
      for (const [name, relative, mutate] of [
        [
          "parser source",
          "workspaces/providers/apps/claude-bridge/rate-limit-input.ts",
          (text: string) => text + "\n// changed consumed source\n",
        ],
        [
          "generated validator",
          "workspaces/runtime/packages/runtime-schema/src/schemas/claude-rate-limit.schema.json",
          (text: string) => {
            const schema = JSON.parse(text);
            schema.properties.status.enum.push("future_status");
            return JSON.stringify(schema);
          },
        ],
      ] as const) {
        await context.test(
          `offline Cargo refuses stale Claude ${name} without node_modules`,
          () => {
            const file = path.join(copy, relative);
            const original = readFileSync(file, "utf8");
            try {
              writeFileSync(file, mutate(original));
              const result = check();
              assert.notEqual(
                result.status,
                0,
                "Offline Cargo must reject stale Claude evidence contracts",
              );
              assert.match(
                result.stdout + result.stderr,
                /Stale Claude rate provenance/,
              );
            } finally {
              writeFileSync(file, original);
            }
          },
        );
      }
      const lifecycle = path.join(
        copy,
        "workspaces/runtime/packages/runtime-schema/src/schemas/codex/lifecycle.schema.json",
      );
      const originalLifecycle = readFileSync(lifecycle, "utf8");
      const changed = JSON.parse(originalLifecycle);
      changed.definitions.TurnStatus.enum.push("futureStatus");
      writeFileSync(lifecycle, JSON.stringify(changed));
      const stale = check();
      assert.notEqual(stale.status, 0);
      assert.match(stale.stdout + stale.stderr, /Stale Codex contract/);
      // The vendor changes below go into one regenerated contract and one check: a check of
      // this large crate takes about a minute, and each change names its own item in the
      // errors, so every witness below still fails alone when its consumer stops rejecting it.
      changed.definitions.ThreadStatus.oneOf.push({
        type: "object",
        properties: {
          type: { type: "string", enum: ["futureActivity"] },
        },
        required: ["type"],
      });
      writeFileSync(lifecycle, JSON.stringify(changed));
      const usage = path.join(
        copy,
        "workspaces/runtime/packages/runtime-schema/src/schemas/codex/ThreadTokenUsageUpdatedNotification.json",
      );
      const originalUsage = readFileSync(usage, "utf8");
      const usageChange = JSON.parse(originalUsage);
      usageChange.definitions.TokenUsageBreakdown.properties.futureTokens = {
        type: "integer",
        format: "int64",
      };
      usageChange.definitions.TokenUsageBreakdown.required.push("futureTokens");
      usageChange.definitions.TokenUsageBreakdown.properties.inputTokens = {
        type: "string",
      };
      usageChange.definitions.ThreadTokenUsage.properties.modelContextWindow = {
        type: ["string", "null"],
      };
      writeFileSync(usage, JSON.stringify(usageChange));
      generate();
      const drift = check();
      assert.notEqual(drift.status, 0);
      for (const [witness, pattern] of [
        [
          "the lifecycle mapping covers every vendor turn status",
          /`CodexTurnStatus::FutureStatus` not covered/,
        ],
        [
          "the real idle decision rejects an added vendor activity discriminator",
          /`CodexThreadActivity::FutureActivity` not covered/,
        ],
        [
          "the usage reader names every vendor token field",
          /pattern does not mention field `future_tokens`/,
        ],
        [
          "the usage reader rejects an incompatible token count type",
          /`u64: TryFrom<std::string::String>` is not satisfied/,
        ],
        [
          "the real usage reader rejects a changed model context window type",
          /`u64: From<std::string::String>` is not satisfied/,
        ],
      ] as const)
        assert.match(drift.stderr, pattern, witness);
      writeFileSync(lifecycle, originalLifecycle);
      writeFileSync(usage, originalUsage);
      generate();
      const restored = check();
      assert.equal(restored.status, 0, restored.stdout + restored.stderr);
    } finally {
      rmSync(copy, { recursive: true, force: true });
    }
  },
);
