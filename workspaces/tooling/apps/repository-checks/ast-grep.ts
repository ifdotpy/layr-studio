import { isGlueSource, HOOK_MANIFEST, glueFiles } from "./react-glue.ts";
import path from "node:path";
import { execFileSync } from "node:child_process";
import { readdirSync, writeFileSync, existsSync } from "node:fs";
import { fileURLToPath } from "node:url";
import {
  checkRatchet,
  baselineGrowthErrors,
  withRatchetBase,
  parseBaseline,
  fingerprintMatches,
  readBaseline,
  RATCHET_BASELINE,
  type Baseline,
  type Violation,
} from "./ratchet.ts";

const root = fileURLToPath(new URL("../../../..", import.meta.url));
const sg = fileURLToPath(
  new URL("../../../../node_modules/.bin/ast-grep", import.meta.url),
);

export function scan(
  scanRoot = root,
  files?: string[],
  filter?: string,
  rulesRoot = root,
): Record<string, Violation[]> {
  const paths =
    files ??
    execFileSync("git", ["ls-files", "-z"], {
      cwd: scanRoot,
      encoding: "utf8",
    })
      .split("\0")
      .filter((file) => /\.(rs|ts|tsx)$/.test(file));
  if (!paths.length) return {};
  const output: unknown = JSON.parse(
    execFileSync(
      sg,
      [
        "scan",
        "--config",
        path.join(rulesRoot, "sgconfig.yml"),
        "--json=compact",
        ...(filter ? ["--filter", filter] : []),
        ...paths,
      ],
      {
        cwd: scanRoot,
        encoding: "utf8",
        maxBuffer: 32 * 1024 * 1024,
      },
    ),
  );
  if (!Array.isArray(output)) throw new Error("Invalid ast-grep output");
  const matches: {
    ruleId: string;
    file: string;
    text: string;
    message: string;
    offset: number;
  }[] = output.map((value: unknown) => {
    if (
      !value ||
      typeof value !== "object" ||
      !("ruleId" in value) ||
      typeof value.ruleId !== "string" ||
      !("file" in value) ||
      typeof value.file !== "string" ||
      !("text" in value) ||
      typeof value.text !== "string" ||
      !("message" in value) ||
      typeof value.message !== "string" ||
      !("range" in value) ||
      !value.range ||
      typeof value.range !== "object" ||
      !("byteOffset" in value.range) ||
      !value.range.byteOffset ||
      typeof value.range.byteOffset !== "object" ||
      !("start" in value.range.byteOffset) ||
      typeof value.range.byteOffset.start !== "number"
    )
      throw new Error("Invalid ast-grep match");
    return {
      ruleId: value.ruleId,
      file: value.file,
      // Keep the existing JSON round-trip baseline stable across the facade rename.
      text: value.ruleId.startsWith("rust-json-")
        ? value.text.replaceAll("studio_json::", "serde_json::")
        : value.text,
      message: value.message,
      offset: value.range.byteOffset.start,
    };
  });
  const scoped = matches.filter(
    (match) =>
      (!match.ruleId.startsWith("web-") || isGlueSource(match.file)) &&
      !(
        match.ruleId.startsWith("web-make-observable") &&
        match.file ===
          "workspaces/client/apps/web/src/generated/observableResources.ts"
      ),
  );
  scoped.sort((a, b) => a.file.localeCompare(b.file) || a.offset - b.offset);
  const grouped: Record<string, Violation[]> = {};
  for (const rule of new Set(scoped.map((match) => match.ruleId)))
    grouped[rule] = fingerprintMatches(
      scoped.filter((match) => match.ruleId === rule),
    );
  return grouped;
}

function previousBaseline(
  repositoryRoot: string,
  rulesRoot = repositoryRoot,
): Baseline {
  return withRatchetBase(repositoryRoot, HOOK_MANIFEST, (snapshot) => {
    const previous = readBaseline(
      path.join(
        snapshot,
        "workspaces/tooling/apps/repository-checks/ratchet-baseline.json",
      ),
    );
    if (!existsSync(path.join(snapshot, HOOK_MANIFEST))) {
      const initial = scan(snapshot, glueFiles(snapshot), "^web-", rulesRoot);
      for (const [rule, entries] of Object.entries(initial))
        previous[rule] = entries;
    }
    return previous;
  });
}

export function webPatternErrors(
  scanRoot: string,
  files: string[],
  repositoryRoot = scanRoot,
): string[] {
  const found = scan(scanRoot, files.filter(isGlueSource), "^web-", scanRoot);
  const baseline = readBaseline(
    path.join(
      scanRoot,
      "workspaces/tooling/apps/repository-checks/ratchet-baseline.json",
    ),
  );
  const rules = new Set(
    [...Object.keys(baseline), ...Object.keys(found)].filter((rule) =>
      rule.startsWith("web-"),
    ),
  );
  const committed = parseBaseline(
    execFileSync(
      "git",
      [
        "show",
        "HEAD:workspaces/tooling/apps/repository-checks/ratchet-baseline.json",
      ],
      { cwd: repositoryRoot, encoding: "utf8" },
    ),
  );
  return [
    ...baselineGrowthErrors(baseline, committed),
    ...baselineGrowthErrors(
      baseline,
      previousBaseline(repositoryRoot, scanRoot),
    ),
    ...[...rules].flatMap((rule) =>
      checkRatchet(rule, found[rule] ?? [], baseline),
    ),
  ];
}

export function run(shrink = false): number {
  const found = scan();
  const baseline = readBaseline();
  const growth = baselineGrowthErrors(baseline, previousBaseline(root));
  if (growth.length) {
    for (const error of growth) console.error(error);
    return 1;
  }
  const rules = new Set(
    readdirSync(fileURLToPath(new URL("./ast-grep/rules", import.meta.url)))
      .filter((file) => file.endsWith(".yml"))
      .map((file) => file.slice(0, -4)),
  );
  if (shrink) {
    const updated: Baseline = { ...baseline };
    for (const rule of rules) {
      const actual = new Set(
        (found[rule] ?? []).map(
          (entry) => `${entry.file}\0${entry.fingerprint}`,
        ),
      );
      updated[rule] = (baseline[rule] ?? [])
        .filter((entry) => actual.has(`${entry.file}\0${entry.fingerprint}`))
        .sort(
          (a, b) =>
            a.file.localeCompare(b.file) ||
            a.fingerprint.localeCompare(b.fingerprint),
        );
    }
    writeFileSync(RATCHET_BASELINE, `${JSON.stringify(updated, null, 2)}\n`);
  }
  const errors = [...rules].flatMap((rule) =>
    checkRatchet(rule, found[rule] ?? [], shrink ? readBaseline() : baseline),
  );
  for (const error of errors) console.error(error);
  return errors.length ? 1 : 0;
}
if (process.argv[1] === fileURLToPath(import.meta.url))
  process.exitCode = run(process.argv[2] === "--shrink");
