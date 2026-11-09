import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import {
  mkdtempSync,
  readdirSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { expect, it } from "vitest";

it("keeps reporting opt-in without changing the default production bundle", () => {
  const directory = mkdtempSync(join(tmpdir(), "studio-perf-parity-"));
  const { STUDIO_PERF_REPORTING: _flag, ...environment } = process.env;
  try {
    const stub = join(directory, "reporting-stub.tsx");
    writeFileSync(
      stub,
      "export function PerformanceReporting() { return null; }\n",
    );
    const referenceConfig = join(directory, "reference.config.mts");
    writeFileSync(
      referenceConfig,
      `
      import original from ${JSON.stringify(resolve("vite.config.ts"))};
      export default { ...original, resolve: { ...original.resolve, alias: [
        { find: new RegExp("PerformanceReporting/PerformanceReporting[.]tsx$"), replacement: ${JSON.stringify(stub)} },
        ...(original.resolve.alias ?? []),
      ] } };
    `,
    );
    // Remove the reporting entry from this same tree to detect unconditional instrumentation.
    const build = (
      flag: string | undefined,
      outputName: string,
      config?: string,
    ) => {
      const output = join(directory, outputName);
      execFileSync(
        "pnpm",
        [
          "exec",
          "vite",
          "build",
          "--outDir",
          output,
          ...(config ? ["--config", config] : []),
        ],
        {
          env: {
            ...environment,
            NODE_ENV: "production",
            ...(flag === undefined ? {} : { STUDIO_PERF_REPORTING: flag }),
          },
          stdio: "pipe",
          maxBuffer: 32 * 1024 * 1024,
        },
      );
      const assets = join(output, "assets");
      const javascript = readdirSync(assets)
        .filter((name) => name.endsWith(".js"))
        .map((name) => readFileSync(join(assets, name), "utf8"));
      return {
        hashes: javascript
          .map((source) => createHash("sha256").update(source).digest("hex"))
          .sort(),
        reports: javascript.some((source) =>
          source.includes("/api/perf-reporting/client"),
        ),
      };
    };
    const production = build(undefined, "production");
    expect(production.reports).toBe(false);
    expect(production).toEqual(
      build(undefined, "uninstrumented", referenceConfig),
    );
    expect(production).toEqual(build("0", "disabled"));
    const enabled = build("1", "enabled");
    expect(enabled.reports).toBe(true);
    expect(enabled.hashes).not.toEqual(production.hashes);
  } finally {
    rmSync(directory, { recursive: true, force: true });
  }
}, 120_000);
