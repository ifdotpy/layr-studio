import { test } from "node:test";
import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import {
  cpSync,
  readFileSync,
  mkdtempSync,
  mkdirSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import os from "node:os";
import path from "node:path";
import {
  checkRatchet,
  fingerprintMatches,
  readBaseline,
  WEB_RATCHET_SEED_BASE,
} from "../ratchet.ts";
import { scan, webPatternErrors } from "../ast-grep.ts";

const rules = "workspaces/tooling/apps/repository-checks/ast-grep/rules";
const fixtures: readonly (readonly [string, string, string, string])[] = [
  [
    "web-run-in-action",
    "ts",
    "runInAction(() => owner.value++);",
    "flow(function* () { yield load(); });",
  ],
  [
    "web-listener-set",
    "ts",
    "const listeners = new Set<() => void>();",
    "const values = new Set<string>();",
  ],
  [
    "web-make-observable",
    "ts",
    "makeObservable(this, {});",
    "makeAutoObservable(this);",
  ],
  [
    "web-store-effect",
    "ts",
    "useEffect(() => store.start(), [store]);",
    "useEffect(() => node.focus(), [node]);",
  ],
  [
    "rust-json-value-roundtrip",
    "rs",
    'let x = studio_json::from_value(json!({"a": 1}));',
    "let x = studio_json::from_value(value);",
  ],
  [
    "rust-json-string-roundtrip",
    "rs",
    "let x = studio_json::from_str(&studio_json::to_string(value)?);",
    "let x = studio_json::from_str(&source);",
  ],
  [
    "rust-json-dispatch",
    "rs",
    'let x = v["status"].as_str() == Some("ok");',
    'let x = v["name"].as_str() == Some("ok");',
  ],
  [
    "rust-serde-default-field",
    "rs",
    "struct S { #[serde(default)] id: String }",
    "struct S { #[serde(default)] name: String }",
  ],
  [
    "ts-json-parse-cast",
    "ts",
    "const x = JSON.parse(raw) as Thing;",
    "const x: unknown = JSON.parse(raw);",
  ],
  [
    "ts-double-cast",
    "ts",
    "const x = value as unknown as Thing;",
    "const x = value as Thing;",
  ],
  ["ts-as-any", "ts", "const x = value as any;", "const x = value as unknown;"],
];
const cases = fixtures.flatMap((fixture) =>
  fixture[1] === "ts"
    ? [fixture, [`${fixture[0]}-tsx`, "tsx", fixture[2], fixture[3]] as const]
    : [fixture],
);
for (const [rule, ext, invalid, valid] of cases) {
  test(`${rule} catches its violation and accepts a nearby valid form`, () => {
    const dir = mkdtempSync(path.join(os.tmpdir(), "studio-ast-test-"));
    try {
      const file = path.join(dir, `fixture.${ext}`);
      const match = (source: string) => {
        writeFileSync(file, source);
        const output: unknown = JSON.parse(
          execFileSync(
            "node_modules/.bin/ast-grep",
            ["scan", "--rule", `${rules}/${rule}.yml`, "--json=compact", file],
            { encoding: "utf8" },
          ),
        );
        assert.ok(Array.isArray(output));
        return output.length;
      };
      assert.equal(match(invalid), 1);
      assert.equal(match(valid), 0);
    } finally {
      rmSync(dir, { recursive: true, force: true });
    }
  });
}
test("JSON dispatch also catches matches macro", () => {
  const dir = mkdtempSync(path.join(os.tmpdir(), "studio-ast-test-"));
  try {
    const file = path.join(dir, "fixture.rs");
    writeFileSync(
      file,
      'fn f() { let x = matches!(v["kind"].as_str(), Some("ok")); }',
    );
    const output: unknown = JSON.parse(
      execFileSync(
        "node_modules/.bin/ast-grep",
        [
          "scan",
          "--rule",
          `${rules}/rust-json-dispatch.yml`,
          "--json=compact",
          file,
        ],
        { encoding: "utf8" },
      ),
    );
    assert.ok(Array.isArray(output));
    assert.equal(output.length, 1);
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
});
test("ratchet rejects new and removed matches until baseline shrinks", () => {
  const [one, two] = fingerprintMatches([
    { file: "a.ts", text: "x  as any", message: "fix" },
    { file: "a.ts", text: "x as any", message: "fix" },
  ]);
  assert.ok(one && two);
  assert.notEqual(one.fingerprint, two.fingerprint);
  assert.equal(checkRatchet("rule", [one, two], { rule: [one] }).length, 1);
  assert.equal(checkRatchet("rule", [], { rule: [one] }).length, 1);
  assert.deepEqual(checkRatchet("rule", [], { rule: [] }), []);
});
test("baseline decoder rejects malformed entries", () => {
  const dir = mkdtempSync(path.join(os.tmpdir(), "studio-ratchet-test-"));
  try {
    const file = path.join(dir, "baseline.json");
    writeFileSync(
      file,
      JSON.stringify({ rule: [{ file: "a.ts", fingerprint: 42 }] }),
    );
    assert.throws(() => readBaseline(file), /Malformed ratchet baseline/);
    writeFileSync(
      file,
      JSON.stringify({ rule: [{ file: "a.ts", fingerprint: "abc:0" }] }),
    );
    assert.deepEqual(readBaseline(file), {
      rule: [{ file: "a.ts", fingerprint: "abc:0" }],
    });
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
});
test("web architecture ast-grep rule cases pass", () => {
  const output = execFileSync(
    "node_modules/.bin/ast-grep",
    ["test", "--skip-snapshot-tests"],
    { encoding: "utf8" },
  );
  assert.match(output, /8 passed/);
  assert.doesNotMatch(output, /Configuration not found/);
});

test("baseline growth rejects added allowances and accepts removals", async () => {
  const { baselineGrowthErrors } = await import("../ratchet.ts");
  const entry = { file: "a.ts", fingerprint: "hash:0" };
  assert.match(
    baselineGrowthErrors({ "web-rule": [entry] }, {}).join("\n"),
    /allowance/,
  );
  assert.deepEqual(baselineGrowthErrors({}, { "web-rule": [entry] }), []);
  assert.deepEqual(
    baselineGrowthErrors({ "web-rule": [entry] }, { "web-rule": [entry] }),
    [],
  );
});

test("generated observable metadata can change without consuming handwritten allowances", () => {
  const dir = mkdtempSync(path.join(os.tmpdir(), "studio-generated-rule-"));
  try {
    const generated =
      "workspaces/client/apps/web/src/generated/observableResources.ts";
    const manual = "workspaces/client/apps/web/src/stores/NewStore.ts";
    for (const file of [generated, manual]) {
      mkdirSync(path.dirname(path.join(dir, file)), { recursive: true });
      writeFileSync(
        path.join(dir, file),
        "makeObservable(this, { newField: observable });",
      );
    }
    const found = scan(dir, [generated, manual], "^web-make-observable");
    assert.deepEqual(
      found["web-make-observable"]?.map((entry) => entry.file),
      [manual],
    );
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
});

test("ratchets refuse a checkout with no usable base history", async () => {
  const { withRatchetBase } = await import("../ratchet.ts");
  const dir = mkdtempSync(path.join(os.tmpdir(), "studio-missing-history-"));
  try {
    const git = (args: string[]) =>
      execFileSync("git", args, { cwd: dir, encoding: "utf8" });
    git(["init", "--quiet", "-b", "fixture"]);
    const marker =
      "workspaces/tooling/apps/repository-checks/react-hook-ceilings.json";
    mkdirSync(path.dirname(path.join(dir, marker)), { recursive: true });
    writeFileSync(path.join(dir, marker), "{}");
    git(["add", "--", marker]);
    git([
      "-c",
      "user.name=Fixture",
      "-c",
      "user.email=fixture@example.invalid",
      "commit",
      "--quiet",
      "-m",
      "Only available commit",
    ]);
    assert.throws(
      () => withRatchetBase(dir, marker, () => {}),
      /history.*main|main.*history/i,
    );
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
});

test("historical seed scanning uses the staged rules snapshot", () => {
  const dir = mkdtempSync(path.join(os.tmpdir(), "studio-staged-seed-"));
  const sourceRoot = process.cwd();
  try {
    const working = path.join(dir, "working");
    const staged = path.join(dir, "staged");
    mkdirSync(working);
    mkdirSync(staged);
    // Use private refs at the pre-manifest seed with a read-only shared object pool.
    const commonGitDir = execFileSync(
      "git",
      ["rev-parse", "--path-format=absolute", "--git-common-dir"],
      {
        encoding: "utf8",
      },
    ).trim();
    execFileSync("git", ["init", "--quiet", "-b", "fixture"], { cwd: working });
    writeFileSync(
      path.join(working, ".git/objects/info/alternates"),
      path.join(commonGitDir, "objects") + "\n",
    );
    const tree = execFileSync("git", ["rev-parse", "HEAD^{tree}"], {
      cwd: sourceRoot,
      encoding: "utf8",
    }).trim();
    const candidate = execFileSync(
      "git",
      [
        "-c",
        "user.name=Fixture",
        "-c",
        "user.email=fixture@example.invalid",
        "commit-tree",
        tree,
        "-p",
        WEB_RATCHET_SEED_BASE,
        "-m",
        "Candidate with a pre-manifest parent",
      ],
      { cwd: working, encoding: "utf8" },
    ).trim();
    execFileSync("git", ["update-ref", "HEAD", candidate], { cwd: working });
    writeFileSync(
      path.join(working, "sgconfig.yml"),
      "ruleDirs: [missing-unstaged-rules]\n",
    );
    const baselineFile =
      "workspaces/tooling/apps/repository-checks/ratchet-baseline.json";
    const baseline = readBaseline();
    const files = [
      ...new Set(
        Object.entries(baseline)
          .filter(([rule]) => rule.startsWith("web-"))
          .flatMap(([, entries]) => entries.map((entry) => entry.file)),
      ),
    ];
    for (const file of [...files, baselineFile, "sgconfig.yml"]) {
      mkdirSync(path.dirname(path.join(staged, file)), { recursive: true });
      writeFileSync(
        path.join(staged, file),
        readFileSync(path.join(sourceRoot, file)),
      );
    }
    cpSync(path.join(sourceRoot, rules), path.join(staged, rules), {
      recursive: true,
    });
    assert.deepEqual(webPatternErrors(staged, files, working), []);
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
});
