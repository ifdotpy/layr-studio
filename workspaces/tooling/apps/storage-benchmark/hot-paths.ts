// Manual P0-C runner, following F0's pinned binary and external CPU/RSS accounting.
import { execFileSync, spawn } from "node:child_process";
import { createHash } from "node:crypto";
import {
  closeSync,
  mkdirSync,
  mkdtempSync,
  openSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { homedir, loadavg } from "node:os";
import { join, resolve } from "node:path";
import { withGateLock } from "../repository-checks/gate-lock.ts";

const [binaryArg, state, server, output, label] = process.argv.slice(2);
if (!binaryArg || !state || !server || !output || !label) {
  throw Error(
    "Usage: node hot-paths.ts BINARY COPIED_STATE SERVER_URL OUTPUT_JSON REVISION_LABEL [--smoke]",
  );
}
const binary = resolve(binaryArg);
await withGateLock(async () => {
  const load = loadavg();
  const provenance = {
    started: new Date().toISOString(),
    label,
    load_before: load,
    git_commit: execFileSync("git", ["rev-parse", "HEAD"], {
      encoding: "utf8",
    }).trim(),
    binary,
    binary_sha256: createHash("sha256")
      .update(readFileSync(binary))
      .digest("hex"),
    fixture: state === "-" ? "synthetic" : resolve(state),
  };
  const oneMinute = load[0];
  if (oneMinute === undefined) throw Error("Missing 1-minute load");
  if (oneMinute >= 8) {
    writeFileSync(
      output,
      JSON.stringify(
        {
          ...provenance,
          status: "deferred",
          reason: "1-minute load >= 8; no benchmark or SQLite open attempted",
        },
        null,
        2,
      ),
    );
    console.log(`Deferred: 1-minute load ${oneMinute}; recorded ${output}`);
    return;
  }
  console.error(
    "Manual P0-C run: <=60 measured seconds, one sequential producer, 100 fixed frames (8 for smoke), <=16 GiB database/WAL, <=32 GiB temporary copies. No providers. Exclusive ordinary-gate lock; timeout 10 minutes including copying.",
  );
  const cache = join(homedir(), ".cache/codex-studio/tmp");
  mkdirSync(cache, { recursive: true });
  const temporary = mkdtempSync(join(cache, "p0-c-"));
  const usage = join(temporary, "usage");
  const usageDescriptor = openSync(usage, "w");
  const reportPath = join(temporary, "report.json");
  const descriptor = openSync(reportPath, "w");
  try {
    const args = ["--hot-paths", "--server", server, "--label", label];
    if (state !== "-") args.push("--state", resolve(state));
    if (process.argv.includes("--smoke")) args.push("--smoke");
    const child = spawn(
      "zsh",
      [
        "-c",
        'TIMEFMT="P0_CPU user=%U sys=%S rss=%M"\ntime "$@"',
        "p0-c",
        binary,
        ...args,
      ],
      {
        env: { ...process.env, TMPDIR: temporary },
        detached: true,
        stdio: ["ignore", descriptor, usageDescriptor],
      },
    );
    const stop = () => {
      if (child.pid !== undefined) {
        try {
          process.kill(-child.pid, "SIGKILL");
        } catch (error) {
          if (
            !(
              error instanceof Error &&
              "code" in error &&
              error.code === "ESRCH"
            )
          )
            throw error;
        }
      }
    };
    const timer = setTimeout(stop, 600_000);
    process.once("SIGTERM", stop);
    process.once("SIGINT", stop);
    let status: number | null;
    try {
      status = await new Promise<number | null>((done, reject) => {
        child.once("error", reject);
        child.once("close", done);
      });
    } finally {
      clearTimeout(timer);
      process.removeListener("SIGTERM", stop);
      process.removeListener("SIGINT", stop);
    }
    if (status !== 0) {
      writeFileSync(
        output,
        JSON.stringify(
          { ...provenance, status: "failed", exit_code: status },
          null,
          2,
        ),
      );
      console.error(readFileSync(usage, "utf8"));
      throw Error(`Benchmark failed; recorded ${output}`);
    }
    const report: unknown = JSON.parse(readFileSync(reportPath, "utf8"));
    const timing = readFileSync(usage, "utf8");
    const match = timing.match(
      /P0_CPU user=([0-9.]+)s? sys=([0-9.]+)s? rss=([0-9]+)/,
    );
    if (!match) throw Error("Missing F0-style process accounting");
    const user_s = Number(match[1]);
    const system_s = Number(match[2]);
    const max_rss_kib = Number(match[3]);
    writeFileSync(
      output,
      JSON.stringify(
        {
          ...provenance,
          status: "measured",
          finished: new Date().toISOString(),
          load_after: loadavg(),
          process_including_copy_and_startup: { user_s, system_s, max_rss_kib },
          report,
        },
        null,
        2,
      ),
    );
    console.log(`Recorded ${output}`);
  } finally {
    closeSync(descriptor);
    closeSync(usageDescriptor);
    rmSync(temporary, { recursive: true, force: true });
  }
});
