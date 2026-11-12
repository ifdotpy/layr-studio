// Offline anonymized fixture export from a sqlite backup copy; never runtime.
import { DatabaseSync, backup } from "node:sqlite";
import { mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";

const directory = fileURLToPath(new URL(".", import.meta.url));
// Reviewed public persistence vocabulary. Unknown fields and wire methods never pass through.
const contract: unknown = JSON.parse(
  await readFile(new URL("public-contract.json", import.meta.url), "utf8"),
);
if (
  !contract ||
  typeof contract !== "object" ||
  !("keys" in contract) ||
  !("literals" in contract) ||
  !("methods" in contract) ||
  !("jsonFields" in contract) ||
  !("dynamicMaps" in contract) ||
  !("placeholderPattern" in contract) ||
  typeof contract.placeholderPattern !== "string" ||
  !("freeTextPattern" in contract) ||
  typeof contract.freeTextPattern !== "string"
)
  throw new Error("invalid public fixture contract");
function strings(value: unknown): Set<string> {
  if (!Array.isArray(value))
    throw new Error("invalid public fixture vocabulary");
  return new Set(
    value.map((item: unknown) => {
      if (typeof item !== "string")
        throw new Error("invalid public fixture word");
      return item;
    }),
  );
}
const keys = strings(contract.keys),
  literals = strings(contract.literals),
  methods = strings(contract.methods);
const jsonFields = strings(contract.jsonFields),
  dynamicMaps = strings(contract.dynamicMaps);
const placeholderPattern = new RegExp(contract.placeholderPattern);
const freeTextPattern = new RegExp(contract.freeTextPattern, "i");
const databasePath = process.argv[2];
if (!databasePath) throw new Error("Usage: node export.ts <database-path>");
const placeholders = new Map<string, string>();
function placeholder(value: string): string {
  let mapped = placeholders.get(value);
  if (!mapped) {
    mapped =
      /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i.test(
        value,
      )
        ? `00000000-0000-4000-8000-${String(placeholders.size + 1).padStart(12, "0")}`
        : `fixture-${placeholders.size + 1}`;
    if (!placeholderPattern.test(mapped))
      throw new Error("invalid generated placeholder");
    placeholders.set(value, mapped);
  }
  return mapped;
}
function anonymize(value: unknown, field = ""): unknown {
  if (typeof value === "string") {
    // JSON stored in text columns stays a string, including nested JSON text.
    if (jsonFields.has(field) && /^[[{]/.test(value)) {
      try {
        return JSON.stringify(anonymize(JSON.parse(value)));
      } catch {
        /* ordinary text */
      }
    }
    if (value === "") return value;
    if (/(?:path|cwd|home|directory|log|file)$/i.test(field))
      return `/fixture/${placeholder(value)}`;
    if (
      /^(?:id|agent|root|account|connection|thread|turn|operation|owner_id|request_id|stream_id)$|(?:Id|AccountKey|StreamId)$/.test(
        field,
      ) ||
      field === "accountKey"
    ) {
      if (
        value === "default" &&
        /account|AccountKey|stream|Stream|owner_id/.test(field)
      )
        return value;
      return placeholder(value);
    }
    if (field === "method")
      return methods.has(value) ? value : placeholder(value);
    if (freeTextPattern.test(field)) return "redacted";
    if (literals.has(value)) return value;
    return placeholder(value);
  }
  if (Array.isArray(value))
    return value.slice(0, 2).map((item) => anonymize(item, field));
  if (value && typeof value === "object")
    return Object.fromEntries(
      Object.entries(value)
        .filter((_, index) => field !== "layrTurnSaves" || index < 1)
        .map(([key, item]) => [
          !dynamicMaps.has(field) && keys.has(key) ? key : placeholder(key),
          anonymize(item, key),
        ]),
    );
  return value;
}

const temporary = await mkdtemp(join(tmpdir(), "studio-live-restart-"));
try {
  const live = new DatabaseSync(databasePath, { readOnly: true });
  try {
    // Pin a read snapshot so ongoing live writes cannot restart the copy indefinitely.
    live.exec("BEGIN");
    live.prepare("SELECT name FROM sqlite_schema LIMIT 1").get();
    await backup(live, join(temporary, "canvas.sqlite3"), { rate: 16_384 });
  } finally {
    live.close();
  }
  const copy = new DatabaseSync(join(temporary, "canvas.sqlite3"), {
    readOnly: true,
  });
  try {
    const fixture: Record<string, unknown[]> = {};
    const recordTables = [
      "runtime_account_transfers",
      "runtime_monitors",
      "runtime_tasks",
      "runtime_rules",
    ];
    for (const table of recordTables) {
      // One small row for each persisted lifecycle state, capped at three rows.
      fixture[table] = copy
        .prepare(
          `SELECT record FROM ${table} GROUP BY coalesce(json_extract(record,'$.status'),json_extract(record,'$.stage'),json_extract(record,'$.kind')) ORDER BY length(record) LIMIT 1`,
        )
        .all()
        .map((row) => {
          if (typeof row.record !== "string")
            throw new Error(`missing record: ${table}`);
          return JSON.parse(row.record);
        });
    }
    // Always include the raw worker-default nesting that caused the transfer incident.
    const transfer = copy
      .prepare(
        "SELECT record FROM runtime_account_transfers WHERE record LIKE '%workerDefaults%' ORDER BY length(record) LIMIT 1",
      )
      .get();
    if (!transfer || typeof transfer.record !== "string")
      throw new Error("no live workerDefaults transfer");
    fixture.runtime_account_transfers = [JSON.parse(transfer.record)];
    fixture.runtime_tool_requests = [];
    const callbacks = copy
      .prepare(
        "SELECT record FROM runtime_tool_requests WHERE kind='provider_callback' AND status IN ('queued','running','unknown') ORDER BY length(record),id LIMIT 1",
      )
      .all();
    for (const row of callbacks) {
      if (typeof row.record !== "string")
        throw new Error("callback owner record");
      fixture.runtime_tool_requests?.push(JSON.parse(row.record));
    }

    for (const table of [
      "runtime_execution_runs",
      "runtime_execution_attempts",
      "runtime_execution_nodes",
      "runtime_execution_effects",
      "runtime_execution_inputs",
    ]) {
      fixture[table] = copy.prepare(`SELECT * FROM ${table} LIMIT 1`).all();
    }
    fixture.runtime_account_state = copy
      .prepare("SELECT * FROM runtime_account_state ORDER BY account LIMIT 1")
      .all();
    fixture.runtime_monitors = copy
      .prepare(
        "SELECT record FROM runtime_monitors WHERE json_type(record,'$.operation')='object' ORDER BY length(record),id LIMIT 1",
      )
      .all()
      .map((row) => {
        if (typeof row.record !== "string") throw new Error("monitor record");
        return JSON.parse(row.record);
      });
    fixture.runtime_outbox = [];
    const lead: unknown = JSON.parse(transfer.record).leadId;
    if (typeof lead !== "string") throw new Error("transfer lead");
    fixture.runtime_events = copy
      .prepare(
        "SELECT id,agent,kind,text,status,created,epoch,turn_id,error FROM runtime_events WHERE agent=? ORDER BY length(text),id LIMIT 1",
      )
      .all(lead);
    fixture.approvals = copy
      .prepare("SELECT * FROM approvals GROUP BY state LIMIT 1")
      .all();
    for (const kind of ["provider_call", "provider_callback"]) {
      const rows =
        kind === "provider_call"
          ? copy
              .prepare(
                "SELECT * FROM runtime_outbox WHERE kind=? AND status IN ('pending','unknown') AND json_extract(payload,'$.family')='retained' AND json_type(payload,'$.payload.request')='object' AND json_extract(payload,'$.payload.written')=1 ORDER BY length(payload),id LIMIT 1",
              )
              .all(kind)
          : copy
              .prepare(
                "SELECT * FROM runtime_outbox WHERE kind=? AND status IN ('pending','unknown','submitted') ORDER BY length(payload),id LIMIT 2",
              )
              .all(kind);
      if (rows.length === 0)
        throw new Error(`no representative ${kind} obligation`);
      const existing: unknown[] = fixture.runtime_outbox ?? [];
      fixture.runtime_outbox = [...existing, ...rows].filter(
        (row, index, all) => {
          if (!row || typeof row !== "object" || !("id" in row))
            throw new Error("outbox id");
          return (
            all.findIndex(
              (candidate) =>
                candidate &&
                typeof candidate === "object" &&
                "id" in candidate &&
                candidate.id === row.id,
            ) === index
          );
        },
      );
      if (kind === "provider_callback")
        for (const row of rows) {
          if (typeof row.owner_id !== "string")
            throw new Error("callback owner");
          const owner = copy
            .prepare("SELECT record FROM runtime_tool_requests WHERE id=?")
            .get(row.owner_id);
          if (owner && typeof owner.record === "string")
            fixture.runtime_tool_requests?.push(JSON.parse(owner.record));
        }
    }
    const ids = new Set<string>();
    const accounts = new Set<string>();
    function owners(value: unknown): void {
      if (Array.isArray(value)) {
        for (const item of value.slice(0, 2)) owners(item);
        return;
      }
      if (!value || typeof value !== "object") return;
      for (const [key, item] of Object.entries(value)) {
        if (["agent", "leadId"].includes(key) && typeof item === "string")
          ids.add(item);
        if (
          [
            "account",
            "accountKey",
            "sourceAccountKey",
            "targetAccountKey",
          ].includes(key) &&
          typeof item === "string"
        )
          accounts.add(item);
        if (key === "members" && item && typeof item === "object")
          for (const id of Object.keys(item)) ids.add(id);
        if (typeof item === "string" && /^[[{]/.test(item)) {
          try {
            owners(JSON.parse(item));
          } catch {
            /* text */
          }
        }
        owners(item);
      }
    }
    for (const rows of Object.values(fixture))
      for (const row of rows) owners(row);
    fixture.runtime_account_state = [...accounts].flatMap((account) =>
      copy
        .prepare("SELECT * FROM runtime_account_state WHERE account=?")
        .all(account),
    );
    fixture.runtime_tool_requests = (
      fixture.runtime_tool_requests ?? []
    ).filter((row, index, all) => {
      if (!row || typeof row !== "object" || !("id" in row))
        throw new Error("tool owner id");
      return (
        all.findIndex(
          (candidate) =>
            candidate &&
            typeof candidate === "object" &&
            "id" in candidate &&
            candidate.id === row.id,
        ) === index
      );
    });
    fixture.runtime_agents = [...ids].flatMap((id) => {
      const row = copy
        .prepare("SELECT record FROM runtime_agents WHERE id=?")
        .get(id);
      return row && typeof row.record === "string"
        ? [JSON.parse(row.record)]
        : [];
    });
    const output =
      JSON.stringify(
        Object.fromEntries(
          Object.entries(fixture).map(([table, rows]) => [
            table,
            rows.map((row) => anonymize(row)),
          ]),
        ),
        null,
        2,
      ) + "\n";
    await writeFile(join(directory, "rows.json"), output, {
      mode: 0o600,
    });
    console.log(
      `Exported ${Object.values(fixture).reduce((sum, rows) => sum + rows.length, 0)} rows (${Buffer.byteLength(output)} bytes)`,
    );
  } finally {
    copy.close();
  }
} finally {
  await rm(temporary, { recursive: true, force: true });
}
