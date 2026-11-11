import { createHash } from "node:crypto";
import { readFileSync, mkdtempSync, rmSync } from "node:fs";
import { execFileSync, spawnSync } from "node:child_process";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";

export const RATCHET_BASELINE = fileURLToPath(
  new URL("./ratchet-baseline.json", import.meta.url),
);

export type Violation = { file: string; fingerprint: string; message: string };
export type BaselineEntry = Pick<Violation, "file" | "fingerprint">;
export type Baseline = Record<string, BaselineEntry[]>;

// Call in source order. Occurrence indices distinguish identical matches in a file.
export function fingerprintMatches(
  matches: { file: string; text: string; message: string }[],
): Violation[] {
  const occurrences = new Map<string, number>();
  return matches.map(({ file, text, message }) => {
    const normalized = text.trim().replace(/\s+/g, " ");
    const digest = createHash("sha256").update(normalized).digest("hex");
    const key = `${file}\0${digest}`;
    const index = occurrences.get(key) ?? 0;
    occurrences.set(key, index + 1);
    return { file, fingerprint: `${digest}:${index}`, message };
  });
}

export function readBaseline(path = RATCHET_BASELINE): Baseline {
  return parseBaseline(readFileSync(path, "utf8"));
}

export function parseBaseline(text: string): Baseline {
  const parsed: unknown = JSON.parse(text);
  if (!parsed || typeof parsed !== "object" || Array.isArray(parsed))
    throw new Error("Malformed ratchet baseline: expected an object");
  const result: Baseline = {};
  for (const [rule, entries] of Object.entries(parsed)) {
    if (!Array.isArray(entries))
      throw new Error(`Malformed ratchet baseline for ${rule}`);
    result[rule] = entries.map((entry: unknown) => {
      if (
        !entry ||
        typeof entry !== "object" ||
        !("file" in entry) ||
        typeof entry.file !== "string" ||
        !("fingerprint" in entry) ||
        typeof entry.fingerprint !== "string"
      )
        throw new Error(`Malformed ratchet baseline entry for ${rule}`);
      return { file: entry.file, fingerprint: entry.fingerprint };
    });
  }
  return result;
}

export function checkRatchet(
  ruleId: string,
  violations: Violation[],
  baseline: Baseline = readBaseline(),
): string[] {
  const key = (entry: BaselineEntry) => `${entry.file}\0${entry.fingerprint}`;
  const expected = new Set((baseline[ruleId] ?? []).map(key));
  const actual = new Set(violations.map(key));
  return [
    ...violations
      .filter((entry) => !expected.has(key(entry)))
      .map(
        (entry) =>
          `${ruleId}: new violation in ${entry.file}: ${entry.message} (${entry.fingerprint})`,
      ),
    ...(baseline[ruleId] ?? [])
      .filter((entry) => !actual.has(key(entry)))
      .map(
        (entry) =>
          `${ruleId}: fixed violation in ${entry.file}; shrink the baseline (${entry.fingerprint})`,
      ),
  ];
}

// Owner: main source revision used to seed the web ratchets before their manifests existed.
export const WEB_RATCHET_SEED_BASE = "44bbf5237ecb947620f44d2bb3de8f6fca98c770";
export function withRatchetBase<T>(
  root: string,
  marker: string,
  check: (snapshot: string) => T,
): T {
  const git = (args: string[]) =>
    spawnSync("git", args, { cwd: root, encoding: "utf8" });
  const head = git(["rev-parse", "HEAD"]).stdout.trim();
  const local = git(["merge-base", "HEAD", "main"]);
  const mergeBase =
    local.status === 0 ? local : git(["merge-base", "HEAD", "origin/main"]);
  const parent = git(["rev-parse", "HEAD^"]);
  const base =
    mergeBase.status === 0 && mergeBase.stdout.trim() !== head
      ? mergeBase.stdout.trim()
      : parent.status === 0
        ? parent.stdout.trim()
        : undefined;
  if (!base)
    throw new Error(
      "Web ratchet checks require Git history with a main merge-base or HEAD parent. Fetch full history before running these checks.",
    );
  const revision =
    git(["cat-file", "-e", `${base}:${marker}`]).status === 0
      ? base
      : WEB_RATCHET_SEED_BASE;
  const paths = execFileSync(
    "git",
    [
      "ls-tree",
      "-d",
      "--name-only",
      revision,
      "--",
      "workspaces/client/apps/web/src",
      "workspaces/tooling/apps/repository-checks",
    ],
    { cwd: root, encoding: "utf8" },
  )
    .trim()
    .split("\n")
    .filter(Boolean);
  const snapshot = mkdtempSync(path.join(os.tmpdir(), "studio-ratchet-base-"));
  try {
    const archive = execFileSync("git", ["archive", revision, ...paths], {
      cwd: root,
      maxBuffer: 64 * 1024 * 1024,
    });
    execFileSync("tar", ["-x", "-C", snapshot], { input: archive });
    return check(snapshot);
  } finally {
    rmSync(snapshot, { recursive: true, force: true });
  }
}

export function baselineGrowthErrors(
  candidate: Baseline,
  previous: Baseline,
): string[] {
  return Object.entries(candidate)
    .filter(([rule]) => rule.startsWith("web-"))
    .flatMap(([rule, entries]) => {
      const allowed = new Set(
        (previous[rule] ?? []).map(
          (entry) => `${entry.file}\0${entry.fingerprint}`,
        ),
      );
      return entries
        .filter((entry) => !allowed.has(`${entry.file}\0${entry.fingerprint}`))
        .map(
          (entry) =>
            `${rule}: baseline allowance grew in ${entry.file}; existing allowances may only shrink`,
        );
    });
}
