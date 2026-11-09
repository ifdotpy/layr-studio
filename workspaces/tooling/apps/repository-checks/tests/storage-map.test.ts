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

// Current code-created application tables absent from the cold-state catalog.
// docs/storage/map.md records their DDL owners and classifications.
const codeCreatedTables = [
  "Canvas.approvals",
  "Canvas.task_window_state",
  "Canvas.agent_publication_state",
  "Canvas.project_grants",
  "Canvas.runtime_work_results",
  "Canvas.runtime_work_decisions",
  "Canvas.runtime_work_releases",
  "Canvas.runtime_peer_update_settings",
  "Canvas.runtime_online_migration_dirty",
  "Analytics.analytics_publication_overflow_receipts",
  "Supervisor.handles",
  "Supervisor.operations",
  "Supervisor.events",
  "Supervisor.child_identities",
  "Supervisor.recovery_events",
  "Supervisor.supervisor_state",
  "Supervisor.degraded_handles",
  "Supervisor.operation_writes",
];

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
    const owner = database === "canvas.sqlite3" ? "Canvas" : "Analytics";
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
  return [...catalogTables(), ...codeCreatedTables].filter(
    (table) => !mapped.has(table),
  );
}

test("storage map covers every catalog and code-created table with a dedicated mapping", () => {
  const markdown = readFileSync(path.join(root, "docs/storage/map.md"), "utf8");
  assert.deepEqual(missingTables(markdown), [], "unmapped catalog tables");
});

test("storage coverage rejects an omitted table even when it is mentioned in prose", () => {
  const tables = [...catalogTables(), ...codeCreatedTables];
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

test("storage coverage rejects each omitted code-created table", () => {
  const tables = [...catalogTables(), ...codeCreatedTables];
  for (const omitted of codeCreatedTables) {
    const markdown = tables
      .filter((table) => table !== omitted)
      .map((table) => `### \`${table}\` → target`)
      .join("\n");
    assert.deepEqual(missingTables(markdown), [omitted], omitted);
  }
});
