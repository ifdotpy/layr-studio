// Run against a private SQLite backup, never the live database:
// node extract.ts /path/to/canvas-copy.sqlite3
import { DatabaseSync } from "node:sqlite";
import { writeFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { sanitize } from "./sanitize.ts";

const path = process.argv[2];
if (!path) throw new Error("pass a private read-only database copy");
const db = new DatabaseSync(path, { readOnly: true });
const directory = fileURLToPath(new URL(".", import.meta.url));

const queries: [string, string][] = [
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
    "SELECT payload AS json FROM sync_entities WHERE collection='work' ORDER BY length(payload) DESC LIMIT 1",
  ],
  [
    "agent-summary",
    "SELECT payload AS json FROM sync_entities WHERE collection='agentSummary' ORDER BY length(payload) DESC LIMIT 1",
  ],
  [
    "room-tail",
    "SELECT payload AS json FROM sync_entities WHERE collection='room' ORDER BY length(payload) DESC LIMIT 1",
  ],
  [
    "transcript",
    "SELECT payload AS json FROM sync_entities WHERE collection LIKE 'transcript:%' AND length(payload)>100000 ORDER BY length(payload) LIMIT 1",
  ],
];
for (const [name, query] of queries) {
  const row = db.prepare(query).get();
  if (!row || typeof row.json !== "string")
    throw new Error(`missing ${name} fixture`);
  writeFileSync(`${directory}${name}.json`, sanitize(row.json));
}
db.close();
