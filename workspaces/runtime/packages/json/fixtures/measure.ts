// Run from the repository root: node workspaces/runtime/packages/json/fixtures/measure.ts
// Callgrind instructions and DHAT total allocation blocks include process setup.
import { execFileSync } from "node:child_process";
import { copyFileSync, mkdtempSync, readFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

const temp = mkdtempSync(join(tmpdir(), "studio-json-measure-"));
const fixtures = [
  "agent-small",
  "agent-large",
  "monitor",
  "task",
  "work",
  "agent-summary",
  "room-tail",
  "transcript",
  "codex-item",
  "codex-tool",
  "codex-usage",
  "claude-message",
  "claude-tool",
  "claude-usage",
  "synthetic-delta-small",
  "synthetic-delta-large",
];
try {
  console.log("backend\tfixture\toperation\tinstructions\tallocations");
  for (const backend of ["serde", "sonic", "simd"]) {
    execFileSync(
      "cargo",
      [
        "build",
        "--release",
        "-j",
        "4",
        "-p",
        "studio-json",
        "--example",
        "measure",
        "--no-default-features",
        "--features",
        `${backend}-backend`,
      ],
      { stdio: "inherit" },
    );
    const binary = join(temp, `measure-${backend}`);
    copyFileSync("target/release/examples/measure", binary);
    for (const fixture of fixtures) {
      for (const operation of backend === "serde"
        ? ["direct", "value", "typed", "serialize", "raw-direct", "raw-facade"]
        : ["value", "typed", "serialize"]) {
        const callgrind = join(temp, "callgrind");
        const dhat = join(temp, "dhat");
        try {
          execFileSync(
            "valgrind",
            [
              "--quiet",
              "--tool=callgrind",
              `--callgrind-out-file=${callgrind}`,
              binary,
              fixture,
              operation,
            ],
            { stdio: "inherit" },
          );
          execFileSync(
            "valgrind",
            [
              "--quiet",
              "--tool=dhat",
              `--dhat-out-file=${dhat}`,
              binary,
              fixture,
              operation,
            ],
            { stdio: "inherit" },
          );
        } catch {
          console.log(`${backend}\t${fixture}\t${operation}\tERROR\tERROR`);
          continue;
        }
        const instructions = /^summary: (\d+)$/m.exec(
          readFileSync(callgrind, "utf8"),
        )?.[1];
        const report: unknown = JSON.parse(readFileSync(dhat, "utf8"));
        if (
          !instructions ||
          typeof report !== "object" ||
          report === null ||
          !("pps" in report) ||
          !Array.isArray(report.pps)
        )
          throw new Error("invalid Valgrind report");
        let allocations = 0;
        for (const point of report.pps) {
          if (
            typeof point !== "object" ||
            point === null ||
            !("tbk" in point) ||
            typeof point.tbk !== "number"
          )
            throw new Error("invalid DHAT point");
          allocations += point.tbk;
        }
        console.log(
          `${backend}\t${fixture}\t${operation}\t${instructions}\t${allocations}`,
        );
      }
    }
  }
} finally {
  rmSync(temp, { recursive: true, force: true });
}
