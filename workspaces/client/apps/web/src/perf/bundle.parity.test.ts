import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import {
  mkdirSync,
  mkdtempSync,
  readdirSync,
  readFileSync,
  rmSync,
  symlinkSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { expect, it } from "vitest";

it("keeps default production JavaScript identical to the base", () => {
  const repository = execFileSync("git", ["rev-parse", "--show-toplevel"], {
    encoding: "utf8",
  }).trim();
  expect(
    execFileSync("git", ["status", "--porcelain"], {
      cwd: repository,
      encoding: "utf8",
    }).trim(),
  ).toBe("");
  let base: string;
  try {
    base = execFileSync("git", ["merge-base", "HEAD", "origin/main"], {
      cwd: repository,
      encoding: "utf8",
    }).trim();
  } catch {
    base = "21f0d343a";
  }
  const directory = mkdtempSync(join(tmpdir(), "studio-perf-parity-"));
  const checkout = join(directory, "checkout");
  const { STUDIO_PERF_REPORTING: _flag, ...environment } = process.env;
  try {
    const build = (revision: string, outputName: string) => {
      if (process.env.LAYR_ROOT) {
        execFileSync(
          "layr",
          ["export", revision === "HEAD" ? "head" : revision, checkout],
          {
            cwd: repository,
          },
        );
      } else {
        const archive = join(directory, "source.tar");
        mkdirSync(checkout);
        execFileSync(
          "git",
          ["archive", "--format=tar", `--output=${archive}`, revision],
          { cwd: repository },
        );
        execFileSync("tar", ["-xf", archive, "-C", checkout]);
      }
      for (const name of [
        "node_modules",
        "workspaces/client/apps/web/node_modules",
        "workspaces/client/apps/web/src/wasm/generated",
      ]) {
        symlinkSync(resolve(repository, name), resolve(checkout, name), "dir");
      }
      const output = join(directory, outputName);
      execFileSync("pnpm", ["exec", "vite", "build", "--outDir", output], {
        cwd: join(checkout, "workspaces/client/apps/web"),
        env: environment,
        stdio: "pipe",
        maxBuffer: 32 * 1024 * 1024,
      });
      const assets = join(output, "assets");
      const chunks = Object.fromEntries(
        readdirSync(assets)
          .filter((name) => name.endsWith(".js"))
          .map((name) => [
            name,
            createHash("sha256")
              .update(readFileSync(join(assets, name)))
              .digest("hex"),
          ]),
      );
      rmSync(checkout, { recursive: true, force: true });
      return chunks;
    };
    expect(build("HEAD", "production")).toEqual(build(base, "baseline"));
  } finally {
    rmSync(directory, { recursive: true, force: true });
  }
}, 120_000);
