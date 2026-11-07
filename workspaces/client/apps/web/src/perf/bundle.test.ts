import { execFileSync } from "node:child_process";
import { mkdtempSync, readdirSync, readFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { expect, it } from "vitest";

it("removes reporting from the default build and includes it in the opted-in build", () => {
  const directory = mkdtempSync(join(tmpdir(), "studio-perf-bundle-"));
  try {
    const { STUDIO_PERF_REPORTING: _flag, ...environment } = process.env;
    for (const enabled of [false, true]) {
      const output = join(directory, enabled ? "enabled" : "default");
      execFileSync(
        "pnpm",
        ["exec", "vite", "build", "--sourcemap", "--outDir", output],
        {
          env: {
            ...environment,
            NODE_ENV: "production",
            ...(enabled ? { STUDIO_PERF_REPORTING: "1" } : {}),
          },
          stdio: "pipe",
        },
      );
      const assets = join(output, "assets");
      const javascript = readdirSync(assets)
        .filter((name) => name.endsWith(".js"))
        .map((name) => readFileSync(join(assets, name), "utf8"))
        .join("\n");
      expect(javascript.includes("/api/perf-reporting/client")).toBe(enabled);
      expect(javascript.includes("durationThreshold:16")).toBe(enabled);
      expect(/id:[`"']composer[`"'],onRender:/.test(javascript)).toBe(enabled);
      expect(/id:[`"']history[`"'],onRender:/.test(javascript)).toBe(enabled);
      expect(javascript.includes("Since strict-mode is enabled")).toBe(enabled);
      // All bare imports, including mobx-react-lite's, must share one MobX copy.
      expect(
        javascript.split(
          "There are multiple, different versions of MobX active.",
        ).length - 1,
      ).toBe(enabled ? 1 : 0);
      expect(javascript.split("[MobX] minified error nr:").length - 1).toBe(
        enabled ? 0 : 1,
      );
      expect(javascript.includes("computedRequiresReaction:!0")).toBe(enabled);
      expect(javascript.includes("observableRequiresReaction:!0")).toBe(
        enabled,
      );
      expect(javascript.includes("reactionRequiresObservable:!0")).toBe(
        enabled,
      );
      expect(/enforceActions:[`"']observed[`"']/.test(javascript)).toBe(
        enabled,
      );
      // Source maps identify the actual React renderer included in the chunks.
      const sourceMaps = readdirSync(assets)
        .filter((name) => name.endsWith(".js.map"))
        .map((name) => readFileSync(join(assets, name), "utf8"))
        .join("\n");
      expect(sourceMaps.includes("react-dom-profiling.profiling.js")).toBe(
        enabled,
      );
      expect(sourceMaps.includes("react-dom-client.production.js")).toBe(
        !enabled,
      );
    }
  } finally {
    rmSync(directory, { recursive: true, force: true });
  }
}, 120_000);
