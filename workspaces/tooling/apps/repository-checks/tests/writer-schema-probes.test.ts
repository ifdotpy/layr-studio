import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";
const root = new URL("../../../../../", import.meta.url);
test("system receipt readers trust the startup schema instead of probing on every write", () => {
  const source = readFileSync(
    new URL(
      "workspaces/runtime/packages/runtime-store/src/repositories/system_reads.rs",
      root,
    ),
    "utf8",
  );
  assert.doesNotMatch(source, /if !exists\(connection,/);
});
import { DatabaseSync } from "node:sqlite";
const migration = new URL(
  "workspaces/runtime/packages/runtime-store/src/migrations/m2026_11_11_writer_receipt_indexes.sql",
  root,
);
test("alias index preserves duplicate aliases and tracks receipt insert, update and delete", () => {
  const db = new DatabaseSync(":memory:");
  db.exec(
    "CREATE TABLE runtime_tool_requests(id TEXT PRIMARY KEY, agent TEXT,record TEXT);",
  );
  const put = db.prepare("INSERT INTO runtime_tool_requests VALUES(?,?,?)");
  put.run("one", "agent", JSON.stringify({ aliases: ["same", "same", "one"] }));
  put.run("two", "agent", JSON.stringify({ aliases: ["same", "two"] }));
  put.run("other", "other", JSON.stringify({ aliases: ["same"] }));
  put.run(
    "mixed",
    "agent",
    JSON.stringify({ aliases: ["42", 42, true, null, { x: 1 }, ["x"]] }),
  );
  put.run("scalar", "agent", JSON.stringify({ aliases: "scalar-alias" }));
  put.run(
    "object",
    "agent",
    JSON.stringify({ aliases: { old: "object-alias", ignored: 3 } }),
  );
  const catalog: unknown = JSON.parse(
    readFileSync(
      new URL(
        "workspaces/runtime/packages/runtime-store/src/runtime-state-schema.json",
        root,
      ),
      "utf8",
    ),
  );
  assert.ok(
    catalog && typeof catalog === "object" && "canvas.sqlite3" in catalog,
  );
  const canvas = catalog["canvas.sqlite3"];
  assert.ok(canvas && typeof canvas === "object" && "sqlite_master" in canvas);
  const objects = canvas.sqlite_master;
  assert.ok(Array.isArray(objects));
  for (const name of [
    "runtime_tool_request_alias_index",
    "runtime_tool_request_alias_index_owner",
  ]) {
    const object: unknown = objects.find(
      (candidate: unknown) =>
        candidate &&
        typeof candidate === "object" &&
        "name" in candidate &&
        candidate.name === name,
    );
    assert.ok(
      object &&
        typeof object === "object" &&
        "sql" in object &&
        typeof object.sql === "string",
    );
    db.exec(object.sql);
  }
  db.exec(readFileSync(migration, "utf8"));
  const expected = () =>
    db
      .prepare(
        "SELECT DISTINCT r.agent,j.value alias,r.id request_id FROM runtime_tool_requests r,json_each(r.record,'$.aliases') j WHERE typeof(j.value)='text' AND r.agent IS NOT NULL ORDER BY 1,2,3",
      )
      .all();
  const actual = () =>
    db
      .prepare(
        "SELECT agent,alias,request_id FROM runtime_tool_request_alias_index ORDER BY 1,2,3",
      )
      .all();
  assert.deepEqual(actual(), expected());
  put.run("new", "agent", JSON.stringify({ aliases: ["same", "new"] }));
  db.prepare(
    "UPDATE runtime_tool_requests SET agent=?, record=? WHERE id=?",
  ).run("moved", JSON.stringify({ aliases: ["changed"] }), "one");
  assert.deepEqual(actual(), expected());
  db.prepare("DELETE FROM runtime_tool_requests WHERE id=?").run("two");
  assert.deepEqual(actual(), expected());
  const plan = db
    .prepare(
      "EXPLAIN QUERY PLAN SELECT request_id FROM runtime_tool_request_alias_index WHERE agent=? AND alias=? LIMIT 2",
    )
    .all("agent", "same");
  assert.ok(
    plan.every(
      (row) => typeof row.detail === "string" && row.detail.includes("SEARCH"),
    ),
    JSON.stringify(plan),
  );
  db.close();
});

test("durable window query uses the descriptor covering index", () => {
  const db = new DatabaseSync(":memory:");
  db.exec(
    "CREATE TABLE durable_rows(tbl TEXT,id TEXT,deleted INTEGER,PRIMARY KEY(tbl,id))",
  );
  const descriptor = readFileSync(
    new URL(
      "workspaces/runtime/packages/runtime-store/src/clock_schema.rs",
      root,
    ),
    "utf8",
  );
  const index = descriptor.match(
    /legacy_index = "(CREATE INDEX durable_rows_window_members[^"\n]+)"/,
  )?.[1];
  assert.ok(index);
  db.exec(index);
  const query = "SELECT id FROM durable_rows WHERE tbl=?1 AND deleted=0";
  const plan = db.prepare("EXPLAIN QUERY PLAN " + query).all("monitor_window");
  assert.ok(
    plan.every(
      (row) =>
        typeof row.detail === "string" &&
        row.detail.includes("SEARCH") &&
        row.detail.includes("COVERING INDEX"),
    ),
    JSON.stringify(plan),
  );
  db.close();
});
