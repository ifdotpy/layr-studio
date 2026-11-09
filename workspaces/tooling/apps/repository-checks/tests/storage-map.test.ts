import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import path from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";

const root = path.resolve(
  path.dirname(fileURLToPath(import.meta.url)),
  "../../../../..",
);
const catalogPath =
  "workspaces/runtime/packages/runtime-store/src/runtime-state-schema.json";

function catalogTables(): string[] {
  const catalog: unknown = JSON.parse(
    readFileSync(path.join(root, catalogPath), "utf8"),
  );
  assert.ok(catalog && typeof catalog === "object");
  const tables: string[] = [];
  for (const [database, schema] of Object.entries(catalog)) {
    if (!database.endsWith(".sqlite3")) continue;
    assert.ok(
      schema && typeof schema === "object" && "sqlite_master" in schema,
    );
    assert.ok(Array.isArray(schema.sqlite_master));
    const owner =
      database === "canvas.sqlite3"
        ? "Canvas"
        : database === "analytics.sqlite3"
          ? "Analytics"
          : "Supervisor";
    for (const entry of schema.sqlite_master) {
      assert.ok(entry && typeof entry === "object" && "type" in entry);
      if (entry.type !== "table") continue;
      assert.ok("name" in entry && typeof entry.name === "string");
      tables.push(`${owner}.${entry.name}`);
    }
  }
  return tables;
}

function mappedTables(markdown: string): Set<string> {
  return new Set(
    [
      ...markdown.matchAll(
        /^### `(Canvas|Analytics|Supervisor)\.([a-z_][a-z0-9_]*)` → /gm,
      ),
    ].map((match) => `${match[1]}.${match[2]}`),
  );
}

function missingTables(markdown: string): string[] {
  const mapped = mappedTables(markdown);
  return catalogTables().filter((table) => !mapped.has(table));
}

type ManifestRow = [
  string,
  string,
  string,
  string,
  string,
  string,
  string,
  string,
  string,
];

function manifestRows(markdown: string): ManifestRow[] {
  const section = markdown.split("## Wave-3 source-to-target manifest\n")[1];
  assert.ok(section, "missing wave-3 manifest");
  return section
    .split("\n")
    .filter((line) => /^\| `(Canvas|Analytics|Supervisor)\./.test(line))
    .map((line): ManifestRow => {
      const cells = line
        .split("|")
        .slice(1, -1)
        .map((cell) => cell.trim());
      assert.equal(cells.length, 9, line);
      const cell = (index: number): string => {
        const value = cells[index];
        assert.ok(value !== undefined);
        return value;
      };
      return [
        cell(0),
        cell(1),
        cell(2),
        cell(3),
        cell(4),
        cell(5),
        cell(6),
        cell(7),
        cell(8),
      ];
    });
}

function assertManifest(markdown: string): void {
  const expected = catalogTables().sort();
  const headings = [
    ...markdown.matchAll(
      /^### `((?:Canvas|Analytics|Supervisor)\.[a-z_][a-z0-9_]*)` → /gm,
    ),
  ]
    .map((match) => match[1])
    .sort();
  assert.deepEqual(headings, expected, "exactly one contract per source");
  const rows = manifestRows(markdown);
  assert.deepEqual(
    rows.map((row) => row[0].replaceAll("`", "")).sort(),
    expected,
    "exactly one manifest row per source",
  );
  const targets = new Set<string>();
  for (const row of rows) {
    assert.equal(row.length, 9, row[0]);
    assert.ok(
      row.every((cell) => cell.length > 0),
      row[0],
    );
    const routes = [
      ...row[1].matchAll(/`((?:Canvas|Analytics|Supervisor)\.[a-z_]+)`/g),
    ];
    assert.ok(routes.length > 0, `missing target: ${row[0]}`);
    for (const route of routes) {
      assert.ok(route[1] !== undefined);
      targets.add(route[1]);
    }
    assert.match(
      row[2],
      /(?:[A-Z][A-Za-z]+Kind::[A-Z][A-Za-z]+|unpartitioned)/,
      row[0],
    );
    assert.match(row[8], /^(?:none|E\d+(?:, E\d+)*)$/, row[0]);
  }
  assert.deepEqual(
    [...targets].sort(),
    [
      ..."entities agents agent_live sessions turns queue_events attempts tasks items contents tool_requests request_results outbox streams changes publications metadata relations history terminal_chunks search monitors terminals incidents servers limit_resets"
        .split(" ")
        .map((name) => `Canvas.${name}`),
      ..."facts usage cost_rows cost_groups changes metadata dictionaries"
        .split(" ")
        .map((name) => `Analytics.${name}`),
      ..."handles operations events recovery state"
        .split(" ")
        .map((name) => `Supervisor.${name}`),
    ].sort(),
    "all 38 targets, no extra destination",
  );
}

test("wave-3 manifest maps every source exactly once to the 38 targets", () => {
  assertManifest(readFileSync(path.join(root, "docs/storage/map.md"), "utf8"));
});

test("wave-3 coverage rejects duplicate and omitted rows and contracts", () => {
  const markdown = readFileSync(path.join(root, "docs/storage/map.md"), "utf8");
  assertManifest(markdown);
  const rows = markdown
    .split("\n")
    .filter((line) => /^\| `(Canvas|Analytics|Supervisor)\./.test(line));
  for (const row of rows) {
    assert.throws(() => assertManifest(markdown.replace(row, "")), row);
    assert.throws(
      () => assertManifest(markdown.replace(row, `${row}\n${row}`)),
      row,
    );
  }
  for (const heading of markdown.matchAll(
    /^### `((?:Canvas|Analytics|Supervisor)\.[a-z_][a-z0-9_]*)` → .*$/gm,
  )) {
    assert.throws(() =>
      assertManifest(
        markdown.replace(heading[0], `${heading[0]}\n${heading[0]}`),
      ),
    );
  }
});

test("storage map covers every catalog table with a dedicated mapping", () => {
  const markdown = readFileSync(path.join(root, "docs/storage/map.md"), "utf8");
  assert.deepEqual(missingTables(markdown), [], "unmapped catalog tables");
});

test("storage coverage rejects an omitted table even when it is mentioned in prose", () => {
  const tables = catalogTables();
  assert.ok(tables.length > 0);
  const omitted = tables[0];
  const markdown = tables
    .slice(1)
    .map((table) => `### \`${table}\` → target`)
    .join("\n");
  assert.deepEqual(missingTables(`${markdown}\nMentioned: ${omitted}`), [
    omitted,
  ]);
});

test("storage coverage rejects each omitted catalog table", () => {
  const tables = catalogTables();
  for (const omitted of tables) {
    const markdown = tables
      .filter((table) => table !== omitted)
      .map((table) => `### \`${table}\` → target`)
      .join("\n");
    assert.deepEqual(missingTables(markdown), [omitted], omitted);
  }
});

test("wave-3 coverage rejects a source absent from the catalog", () => {
  const markdown = readFileSync(path.join(root, "docs/storage/map.md"), "utf8");
  assert.throws(() =>
    assertManifest(
      markdown.replaceAll("Canvas.runtime_agents", "Canvas.unknown_table"),
    ),
  );
});
