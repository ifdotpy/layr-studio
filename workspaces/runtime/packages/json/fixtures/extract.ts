// Run against a private SQLite backup, never the live database:
// node extract.ts /path/to/canvas-copy.sqlite3
import { DatabaseSync } from "node:sqlite";
import { writeFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { sanitize } from "./sanitize.ts";

export const queries: [string, string][] = [
  [
    "agent-small",
    "SELECT record AS json FROM runtime_agents ORDER BY length(record) LIMIT 1",
  ],
  [
    "agent-large",
    "SELECT record AS json FROM runtime_agents ORDER BY length(record) DESC LIMIT 1",
  ],
  [
    "monitor",
    "SELECT record AS json FROM runtime_monitors WHERE json_type(record,'$.outputMatchExcerpts')='object' ORDER BY length(record) DESC LIMIT 1",
  ],
  [
    "task",
    "SELECT record AS json FROM runtime_tasks ORDER BY length(record) DESC LIMIT 1",
  ],
  [
    "work",
    "SELECT entity_payload AS json FROM publications WHERE kind=2 AND collection_order='work' ORDER BY length(entity_payload) DESC,source_rowid LIMIT 1",
  ],
  [
    "agent-summary",
    "SELECT entity_payload AS json FROM publications WHERE kind=2 AND collection_order='agentSummary' ORDER BY length(entity_payload) DESC,source_rowid LIMIT 1",
  ],
  [
    "room-tail",
    "SELECT entity_payload AS json FROM publications WHERE kind=2 AND collection_order='room' ORDER BY length(entity_payload) DESC,source_rowid LIMIT 1",
  ],
  [
    "transcript",
    "SELECT entity_payload AS json FROM publications WHERE kind=2 AND collection_order LIKE 'transcript:%' AND length(entity_payload)>100000 ORDER BY length(entity_payload),source_rowid LIMIT 1",
  ],
];
if (process.argv[1] === fileURLToPath(import.meta.url)) {
  const path = process.argv[2];
  if (!path) throw new Error("pass a private read-only database copy");
  const db = new DatabaseSync(path, { readOnly: true });
  const directory = fileURLToPath(new URL(".", import.meta.url));
  for (const [name, query] of queries) {
    const row = db.prepare(query).get();
    if (!row || typeof row.json !== "string")
      throw new Error(`missing ${name} fixture`);
    writeFileSync(`${directory}${name}.json`, sanitize(row.json));
  }
  db.close();
}
