import { strict as assert } from "node:assert";
import { readFileSync } from "node:fs";
import { DatabaseSync } from "node:sqlite";
import { test } from "node:test";
import { queries } from "./extract.ts";

const schema =
  "CREATE TABLE publications(kind INTEGER,collection_order TEXT,entity_payload TEXT,source_rowid INTEGER)";
const insert = "INSERT INTO publications VALUES(?,?,?,?)";

test("extractor canonical SQL agrees with consolidation kind and Publications owners", () => {
  const kinds = readFileSync(
    new URL("../../runtime-store/src/consolidation_kinds.rs", import.meta.url),
    "utf8",
  );
  assert.match(
    kinds,
    /enum CanvasPublicationsKind\s*\{\s*SyncDocuments = 1,\s*SyncEntities = 2,/,
  );
  const wire = readFileSync(
    new URL("../../runtime-http/openapi.json", import.meta.url),
    "utf8",
  );
  for (const [name, label] of [
    ["Work", "work"],
    ["AgentSummary", "agentSummary"],
    ["Room", "room"],
  ]) {
    assert.match(
      wire,
      new RegExp(
        `"Sync${name}EntityPayload": \\{[\\s\\S]*?"const": "${label}"`,
      ),
    );
  }
  const collections = readFileSync(
    new URL(
      "../../runtime-store/src/publication_collection.rs",
      import.meta.url,
    ),
    "utf8",
  );
  assert.match(collections, /const TRANSCRIPT_FAMILY: &str = "transcript"/);
  const owner = readFileSync(
    new URL("../../runtime-store/src/publication_schema.rs", import.meta.url),
    "utf8",
  );
  for (const column of [
    "kind",
    "collection_order",
    "entity_payload",
    "source_rowid",
  ]) {
    assert.match(
      owner,
      new RegExp(`\\n    ${column}:`),
      `Publications owns ${column}`,
    );
  }
});

test("extractor reads canonical entities preserving partition, NULLs and source rowid ties", () => {
  const db = new DatabaseSync(":memory:");
  try {
    db.exec(schema);
    const put = db.prepare(insert);
    for (const [name, query] of queries.slice(4)) {
      const collection =
        name === "work"
          ? "work"
          : name === "agent-summary"
            ? "agentSummary"
            : name === "room-tail"
              ? "room"
              : "transcript:agent";
      const raw =
        name === "transcript"
          ? ` {"raw":"${"x".repeat(100001)}"} `
          : ' {"raw":1e+02} ';
      // Insert physical-first in reverse source order; byte spelling must survive.
      put.run(2, collection, raw.replace("raw", "old"), 20);
      put.run(2, collection, raw, -7);
      put.run(2, collection, null, 0);
      put.run(1, collection, "x".repeat(raw.length + 10), -100);
      assert.equal(db.prepare(query).get()?.json, raw, name);
    }
  } finally {
    db.close();
  }
});
