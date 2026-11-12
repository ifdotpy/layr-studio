import assert from "node:assert/strict";
import test from "node:test";
import {
  mkdtempSync,
  mkdirSync,
  writeFileSync,
  readFileSync,
  rmSync,
} from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import {
  checkSqlRatchet,
  REVIEWED_CALLBACK_SQL_ALLOWANCES,
  REVIEWED_P2_SQL_ALLOWANCES,
  WRITER_PLAN_WITNESS_SQL_ALLOWANCES,
  REVIEWED_WAVE_C_SQL_ALLOWANCES,
  fingerprint,
  formatSqlBaseline,
  ownerFor,
  tablesIn,
  scanRustNodes,
  scanSql,
  scanTypeScript,
  SQLITE_DRIVERS,
  compilerWitnessErrors,
  COMPILER_SQL_WITNESS_FILE,
  readSqlBaseline,
  sqlDiagnostic,
  constructorErrors,
  QUERY_CONSTRUCTOR_OWNER,
} from "../sql-ratchet.ts";

test("reviewed SQL remains limited to its exact sites and counts", () => {
  const allowances = [
    ...REVIEWED_CALLBACK_SQL_ALLOWANCES,
    ...REVIEWED_P2_SQL_ALLOWANCES,
    ...WRITER_PLAN_WITNESS_SQL_ALLOWANCES,
    ...REVIEWED_WAVE_C_SQL_ALLOWANCES,
  ];
  const identity = (row: (typeof allowances)[number]) =>
    `${row.file}:${row.api}:${row.fingerprint}`;
  for (const allowance of new Map(
    allowances.map((row) => [identity(row), row]),
  ).values()) {
    const budget = allowances.filter(
      (row) => identity(row) === identity(allowance),
    ).length;
    const site = { ...allowance, symbol: "reviewed callback lookup" };
    assert.deepEqual(
      checkSqlRatchet(
        Array.from({ length: budget }, () => site),
        [],
      ),
      [],
    );
    assert.equal(
      checkSqlRatchet(
        Array.from({ length: budget + 1 }, () => site),
        [],
      ).length,
      1,
    );
    assert.equal(
      checkSqlRatchet([{ ...site, file: "other.rs" }], []).length,
      1,
    );
    assert.equal(
      checkSqlRatchet([{ ...site, fingerprint: "changed" }], []).length,
      1,
    );
  }
});

test("scanner rejects additional SQL in the reviewed callback file", () => {
  const directory = mkdtempSync(
    path.join(tmpdir(), "studio-callback-sql-ratchet-"),
  );
  try {
    const file =
      "workspaces/runtime/packages/runtime-services/src/callback_reply.rs";
    const target = path.join(directory, file);
    mkdirSync(path.dirname(target), { recursive: true });
    const allowed = readFileSync(file, "utf8")
      .split("\n")
      .find((line) => line.startsWith("const IMMEDIATE:"));
    assert.ok(allowed);
    const sites = () => {
      const nodes = scanRustNodes([target], directory);
      return nodes
        .filter((node) => node.ruleId !== "symbol")
        .map((node) => {
          const owner = nodes.find(
            (candidate) =>
              candidate.ruleId === "symbol" &&
              candidate.start <= node.start &&
              candidate.end >= node.end,
          );
          const symbol =
            owner?.text.match(/(?:fn|const)\s+(\w+)/)?.[1] ?? "<module>";
          const api =
            node.ruleId === "call"
              ? "rust::unresolved-sql-candidate"
              : node.ruleId === "boundary"
                ? "rust::persistence-boundary"
                : "rust::sql-template";
          return {
            file,
            symbol,
            api,
            fingerprint: fingerprint(node.text, symbol),
          };
        });
    };
    writeFileSync(target, allowed + "\n");
    assert.equal(sites().length, 1, "the real reviewed query is scanned");
    assert.deepEqual(checkSqlRatchet(sites(), []), []);
    const execute = ["exec", "ute"].join("");
    const query = ["DELETE", "FROM", "unexpected_callback_rows"].join(" ");
    writeFileSync(
      target,
      `${allowed}\nuse rusqlite::Connection;
fn unexpected() { db.${execute}("${query}", []); }
`,
    );
    const errors = checkSqlRatchet(sites(), []);
    assert.equal(
      errors.length,
      3,
      "extra import, executor and SQL string are all rejected",
    );
    for (const api of [
      "rust::persistence-boundary",
      "rust::unresolved-sql-candidate",
      "rust::sql-template",
    ]) {
      assert.ok(
        errors.some((error) => error.includes(`(${api})`)),
        api,
      );
    }
  } finally {
    rmSync(directory, { recursive: true, force: true });
  }
});

test("SQL ratchet rejects additions and duplicate calls and permits removals", () => {
  const site = {
    file: "caller.rs",
    symbol: "load",
    api: "rusqlite::Connection::query_row",
    fingerprint: fingerprint("db.query_row(sql, [], decode)", "load"),
  };
  const baseline = [site];
  assert.deepEqual(checkSqlRatchet([site], baseline), []);
  assert.deepEqual(checkSqlRatchet([], baseline), []);
  assert.equal(checkSqlRatchet([site, site], baseline).length, 1);
  assert.equal(
    checkSqlRatchet([{ ...site, file: "new-caller.rs" }], baseline).length,
    1,
  );
  assert.equal(
    checkSqlRatchet(
      [
        {
          ...site,
          fingerprint: fingerprint(
            "db.query_row(other_sql, [], decode)",
            "load",
          ),
        },
      ],
      baseline,
    ).length,
    1,
  );
});

test("ownership is exclusive and multi-family callers belong to F1-shared", () => {
  assert.equal(ownerFor("src/reader.rs", ["F2"]), "F2");
  assert.equal(ownerFor("src/reader.rs", ["F2", "F3"]), "F1-shared");
  assert.equal(
    ownerFor("workspaces/runtime/packages/db-api/src/query.rs", ["F2"]),
    "F1",
  );
  assert.equal(
    ownerFor("workspaces/tooling/apps/installed-smoke/usage.ts", ["F2"]),
    "F1-shared",
  );
});

test("table evidence retains qualified names and unresolved dynamic identifiers", () => {
  const names = ["runtime_agents", "runtime_events"];
  assert.deepEqual(
    tablesIn(
      'SELECT a.id FROM "runtime_agents" a JOIN runtime_events e ON e.agent=a.id',
      names,
    ),
    names,
  );
  assert.deepEqual(tablesIn("SELECT * FROM {table}", names), ["<dynamic>"]);
});

test("Rust AST inventory sees generic/dynamic calls and ignores comments and non-SQL method names", () => {
  const dir = mkdtempSync(path.join(tmpdir(), "studio-sql-inventory-"));
  const file = path.join(dir, "witness.rs");
  try {
    writeFileSync(
      file,
      `fn caller() {
      // db.execute("DELETE FROM hidden", []);
      db.query::<Row>(sql, args);
      tx.execute(dynamic_sql, params);
      result.unwrap_or_else(|| db.query_row(SQL, [], decode));
      Statement::declare("SELECT * FROM runtime_agents");
    }`,
    );
    const nodes = scanRustNodes([file]);
    assert.equal(nodes.filter((node) => node.ruleId === "call").length, 3);
    assert.equal(nodes.filter((node) => node.ruleId === "boundary").length, 1);
    assert.equal(nodes.filter((node) => node.ruleId === "sql").length, 1);
  } finally {
    rmSync(dir, { recursive: true });
  }
});

test("compiler resolution distinguishes rusqlite methods from unrelated method collisions", () => {
  const diagnostic = (api: string) =>
    JSON.stringify({
      reason: "compiler-message",
      message: {
        code: { code: "clippy::disallowed_methods" },
        message: `use of a disallowed method \`${api}\``,
        spans: [
          {
            is_primary: true,
            file_name: "caller.rs",
            byte_start: 12,
            byte_end: 21,
          },
        ],
      },
    });
  assert.equal(
    sqlDiagnostic(diagnostic("rusqlite::Connection::query_row"))?.api,
    "rusqlite::Connection::query_row",
  );
  assert.equal(
    sqlDiagnostic(diagnostic("other_crate::Connection::query_row")),
    undefined,
  );
});

test("repository SQL sites stay within the frozen file/symbol baseline and file ownership is exclusive", () => {
  const scan = scanSql();
  const sites = scan.sites.filter(
    (site) =>
      !/^workspaces\/runtime\/packages\/db-api(?:-derive)?\//.test(site.file) &&
      site.api !== "rust::typed-query-macro",
  );
  assert.deepEqual(checkSqlRatchet(sites, readSqlBaseline()), []);
  assert.deepEqual(scan.forbiddenConstructors, []);
  assert.equal(
    new Set(scan.manifest.map((row) => row.file)).size,
    scan.manifest.length,
  );
  assert.ok(
    scan.manifest.every((row) => /^(F[1-8]|F1-shared)$/.test(row.owner)),
  );
  assert.ok(
    scan.sites.some((site) =>
      site.file.endsWith("upgrade-canary/studio-claude-profile.ts"),
    ),
  );
  assert.ok(
    scan.sites.some((site) => site.file.endsWith("installed-smoke/usage.ts")),
  );
  assert.ok(
    scan.sites.some((site) => site.file.endsWith("verify-rust-chat-scale.ts")),
  );
});

test("generated query constructors are forbidden in fabricated calls, comments, fixtures and prose", () => {
  const hidden = ["__generated", "declare"].join("_");
  const internal = ["Statement", "internal"].join("::");
  for (const file of [
    "caller.rs",
    "db-api-derive/src/named.rs",
    "docs/query.md",
    "tests/fixture.ts",
  ]) {
    for (const text of [
      `NamedQuery::${hidden}(sql)`,
      `// ${hidden}`,
      `const sql = "${internal}"`,
      `${["Statement", "internal"].join(" :: ")}(sql)`,
    ]) {
      assert.equal(constructorErrors([{ file, text }]).length, 1);
    }
  }
  assert.deepEqual(
    constructorErrors([
      { file: QUERY_CONSTRUCTOR_OWNER, text: hidden + internal },
    ]),
    [],
  );
});

test("new SQLite-capable modules fail regardless of receiver or import form", () => {
  const sources = [
    `import {DatabaseSync} from "node:sqlite"; const db = new DatabaseSync(":memory:"); db["exec"](["CREATE", "TABLE t(id)"].join(" "));`,
    `import type {DatabaseSync} from "node:sqlite"; function write(c: DatabaseSync, sql: string) { c.exec(sql); }`,
    `const driver = await import("node:sqlite");`,
    `export {DatabaseSync} from "node:sqlite";`,
  ];
  for (const source of sources) {
    const file = "new-sqlite-module.ts";
    const sites = scanTypeScript(file, source)
      .filter((site) => site.driver)
      .map(() => ({
        file,
        symbol: "<module>",
        api: "typescript::sqlite-driver",
        fingerprint: fingerprint("file", "<module>"),
      }));
    assert.equal(sites.length, 1, source);
    assert.equal(checkSqlRatchet(sites, []).length, 1);
    assert.deepEqual(
      checkSqlRatchet(
        sites,
        sites.map((site) => ({ ...site, reason: "frozen SQLite file" })),
      ),
      [],
    );
    assert.equal(
      checkSqlRatchet(
        sites,
        sites.map((site) => ({
          ...site,
          file: "other.ts",
          reason: "other frozen file",
        })),
      ).length,
      1,
    );
  }
});

test("all SQLite specifier literals freeze JavaScript and TypeScript files", () => {
  for (const driver of SQLITE_DRIVERS)
    for (const file of ["new.ts", "new.js", "new.cjs", "new.mjs"]) {
      const source = `const specifier = ${JSON.stringify(driver)}; const loader = require; loader(specifier);`;
      assert.equal(
        scanTypeScript(file, source).filter((site) => site.driver).length,
        1,
      );
    }
});

test("SQL fingerprints preserve spaces inside string literals", () => {
  assert.notEqual(fingerprint(`SELECT 'a b'`), fingerprint(`SELECT 'a  b'`));
});

test("landing compiler SQL check requires both alias and macro witnesses", () => {
  const sites = [
    "compiler_detects_sql_function_alias",
    "compiler_detects_sql_in_macro_argument",
  ].map((symbol) => ({
    file: COMPILER_SQL_WITNESS_FILE,
    symbol,
    api: "rusqlite::Connection::execute",
    fingerprint: "fixture",
  }));
  assert.equal(compilerWitnessErrors([]).length, 2);
  assert.equal(compilerWitnessErrors(sites.slice(0, 1)).length, 1);
  assert.deepEqual(compilerWitnessErrors(sites), []);
  assert.equal(
    checkSqlRatchet(
      sites.map((site) => ({ ...site, file: "caller.rs" })),
      [],
    ).length,
    2,
  );
});

test("compact baseline loads grouped fingerprints and preserves duplicate budgets", () => {
  const dir = mkdtempSync(path.join(tmpdir(), "studio-sql-baseline-"));
  const file = path.join(dir, "baseline.json");
  try {
    const fp = fingerprint("statement");
    assert.match(fp, /^[a-f0-9]{16}$/);
    writeFileSync(
      file,
      JSON.stringify({ "caller.rs": { "sqlite::execute": [fp, fp] } }),
    );
    const baseline = readSqlBaseline(file);
    const site = {
      file: "caller.rs",
      symbol: "load",
      api: "sqlite::execute",
      fingerprint: fp,
    };
    assert.equal(baseline.length, 2);
    assert.deepEqual(checkSqlRatchet([site, site], baseline), []);
    assert.equal(checkSqlRatchet([site, site, site], baseline).length, 1);
  } finally {
    rmSync(dir, { recursive: true });
  }
});

test("generated baseline is compact, sorted and one line per file", () => {
  const baseline = readSqlBaseline();
  const source = readFileSync(
    new URL("../sql-baseline.json", import.meta.url),
    "utf8",
  );
  assert.equal(source, formatSqlBaseline(baseline));
  assert.ok(Buffer.byteLength(source) < 1_000_000);
  assert.equal(
    source.trim().split("\n").length,
    new Set(baseline.map((row) => row.file)).size + 2,
  );
});

test("identical SQL moved to a different symbol exceeds its frozen allowance", () => {
  const text = "db.query_row(sql, [], decode)";
  const call = (symbol: string) => ({
    file: "caller.rs",
    symbol,
    api: "rusqlite::Connection::query_row",
    fingerprint: fingerprint(text, symbol),
  });
  const original = call("original");
  const moved = call("moved");
  assert.notEqual(moved.fingerprint, original.fingerprint);
  assert.deepEqual(checkSqlRatchet([original], [original]), []);
  assert.equal(checkSqlRatchet([moved], [original]).length, 1);
});

test("only derive-bound DbQuery SQL arguments are declared queries", () => {
  const dir = mkdtempSync(path.join(tmpdir(), "studio-db-query-classifier-"));
  const file = path.join(dir, "witness.rs");
  const sql = ["SEL", "ECT * FR", "OM named_rows"].join("");
  try {
    writeFileSync(
      file,
      `
#[derive(Clone, studio_db_api::DbQuery)]
#[cfg(test)]
#[db_query(sql="${sql}")]
struct Declared;
const OUTSIDE: &str = "${sql}";
#[db_query(sql="${sql}")]
struct MissingDerive;
#[derive(DbQuery)]
struct Previous;
#[db_query(sql="${sql}")]
struct Next;
#[derive(DbQuery)]
#[other(sql="${sql}")]
struct OtherAttribute;
#[derive(DbQuery)]
#[db_query(note="${sql}", sql="${sql}")]
struct OtherArgument;
#[db_query(sql="${sql}")]
#[derive(DbQuery)]
struct Reversed;
`,
    );
    const nodes = scanRustNodes([file]);
    const declared = nodes.filter((node) => node.ruleId === "declared-query");
    const raw = nodes.filter((node) => node.ruleId === "sql");
    assert.equal(declared.length, 3);
    assert.equal(raw.length, 5);
    assert.ok(declared.every((node) => node.text === JSON.stringify(sql)));
    assert.ok(
      raw.every((node) => node.text === JSON.stringify(sql)),
      "the identical literal remains raw outside the bound SQL argument",
    );
  } finally {
    rmSync(dir, { recursive: true });
  }
});

test("sealed statement calls are not compiler SQL sinks but string sinks at the same site remain flagged", () => {
  const diagnostic = (api: string) =>
    JSON.stringify({
      reason: "compiler-message",
      message: {
        code: { code: "clippy::disallowed_methods" },
        message: `use of a disallowed method \`${api}\``,
        spans: [
          {
            is_primary: true,
            file_name: "same-site.rs",
            byte_start: 12,
            byte_end: 40,
          },
        ],
      },
    });
  assert.equal(
    sqlDiagnostic(diagnostic("studio_db_api::query::Read::query")),
    undefined,
  );
  const raw = sqlDiagnostic(diagnostic("rusqlite::Connection::query_row"));
  assert.deepEqual(raw, {
    file: "same-site.rs",
    start: 12,
    end: 40,
    api: "rusqlite::Connection::query_row",
  });
  assert.ok(raw);
  assert.equal(
    checkSqlRatchet(
      [
        {
          ...raw,
          symbol: "caller",
          fingerprint: fingerprint("query(sql)", "caller"),
        },
      ],
      [],
    ).length,
    1,
  );
});

test("JSX data labels are presentation while query callbacks remain SQL", () => {
  const label = ["Select", "an", "agent"].join(" ");
  const query = ["SELECT", "name", "FROM", "agents"].join(" ");
  assert.deepEqual(
    scanTypeScript(
      "view.tsx",
      `<NativeSelect data={[{ value: "", label: "${label}" }]} />`,
    ),
    [],
  );
  const found = scanTypeScript(
    "view.tsx",
    `<View onClick={() => db.exec("${query}")} />`,
  );
  assert.equal(found.length, 1);
  assert.equal(found[0]?.text, JSON.stringify(query));
});

for (const kind of ["template substitution", "property access"] as const) {
  test(`SQL in a JSX data label ${kind} remains executable`, () => {
    const query = ["SELECT", "name", "FROM", "agents"].join(" ");
    const expression =
      kind === "template substitution"
        ? "`" + "${db.exec(" + JSON.stringify(query) + ")}" + "`"
        : "({ label: " + JSON.stringify(query) + " }).label";
    const found = scanTypeScript(
      "view.tsx",
      "<NativeSelect data={[{ label: " + expression + " }]} />",
    );
    assert.equal(
      found.filter((site) => site.text === JSON.stringify(query)).length,
      1,
    );
  });
}

test("generated Codex open-JSON fields keep imports, calls and SQL visible to the ratchet", () => {
  const directory = mkdtempSync(
    path.join(tmpdir(), "studio-vendor-value-ratchet-"),
  );
  try {
    const vendor = path.join(
      directory,
      "workspaces/runtime/packages/codex-protocol/src/generated/mod.rs",
    );
    const sibling = path.join(path.dirname(vendor), "storage.rs");
    const ordinary = path.join(directory, "ordinary.rs");
    mkdirSync(path.dirname(vendor), { recursive: true });
    // Assemble Rust-only executor syntax so the SQL audit sees the fixture, not a TypeScript executor.
    const execute = ["exec", "ute"].join("");
    const query = ["SELECT", "*", "FROM", "runtime_agents"].join(" ");
    const source = `struct Params { opaque: studio_json::Value }
enum Payload { Json(studio_json::Value), Named { opaque: studio_json::Value } }
use studio_json::Value;
fn call(argument: studio_json::Value) { consume(studio_json::Value::Null); db.${execute}("${query}", []); }
`;
    writeFileSync(vendor, source);
    writeFileSync(sibling, source);
    writeFileSync(ordinary, source);
    const unknownQuery = [
      "SELECT",
      "unknown_payload",
      "FROM",
      "vendor_wire",
    ].join(" ");
    const unknown = `impl<'de> serde::Deserialize<'de> for UnknownPayload {
fn deserialize() { let value: studio_json::Value = studio_json::Value::Null; db.${execute}("${unknownQuery}", []); }
}
`;
    writeFileSync(vendor, source + unknown);
    writeFileSync(sibling, source + unknown);
    writeFileSync(ordinary, source + unknown);
    const codec = path.join(
      directory,
      "workspaces/runtime/packages/runtime-providers/src/codex_codec.rs",
    );
    mkdirSync(path.dirname(codec), { recursive: true });
    writeFileSync(
      codec,
      `use studio_json::Value;
fn unparsed_known() { let value: studio_json::Value = studio_json::Value::Null; db.${execute}("${query}", []); }
fn ordinary() { let value: studio_json::Value; }
`,
    );
    const codecNodes = scanRustNodes([codec], directory);
    assert.equal(
      codecNodes.filter((node) => node.ruleId === "boundary").length,
      3,
    );
    assert.ok(
      codecNodes.some(
        (node) => node.ruleId === "call" && node.text.includes("execute"),
      ),
    );
    assert.ok(
      codecNodes.some(
        (node) => node.ruleId === "sql" && node.text.includes("SELECT"),
      ),
    );
    const nodes = scanRustNodes([vendor], directory);
    assert.equal(nodes.filter((node) => node.ruleId === "boundary").length, 4);
    const unknownOwner = nodes.find(
      (node) =>
        node.ruleId === "symbol" &&
        node.text.startsWith(
          "impl<'de> serde::Deserialize<'de> for UnknownPayload",
        ),
    );
    assert.ok(unknownOwner);
    for (const ruleId of ["call", "sql"]) {
      const insideUnknown: typeof nodes = nodes.filter(
        (node) =>
          node.ruleId === ruleId &&
          node.start >= unknownOwner.start &&
          node.end <= unknownOwner.end,
      );
      assert.equal(
        insideUnknown.length,
        1,
        `the Unknown serde owner retains its ${ruleId} node`,
      );
      assert.ok(insideUnknown[0]?.text.includes(unknownQuery));
    }
    assert.ok(
      nodes.some(
        (node) =>
          node.ruleId === "boundary" &&
          node.text.includes("use studio_json::Value"),
      ),
    );
    assert.ok(
      nodes.some(
        (node) => node.ruleId === "call" && node.text.includes("execute"),
      ),
    );
    assert.ok(
      nodes.some(
        (node) => node.ruleId === "sql" && node.text.includes("SELECT"),
      ),
    );
    assert.equal(
      scanRustNodes([sibling], directory).filter(
        (node) => node.ruleId === "boundary",
      ).length,
      9,
    );
    assert.equal(
      scanRustNodes([ordinary], directory).filter(
        (node) => node.ruleId === "boundary",
      ).length,
      9,
    );
  } finally {
    rmSync(directory, { recursive: true, force: true });
  }
});
