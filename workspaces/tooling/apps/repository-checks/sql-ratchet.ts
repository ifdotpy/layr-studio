import { createHash } from "node:crypto";
import { spawnSync } from "node:child_process";
import { readFileSync, writeFileSync, existsSync } from "node:fs";
import { fileURLToPath } from "node:url";
import path from "node:path";
import ts from "typescript";

// db-api execution methods consume sealed Statement tokens, so they are not raw-SQL sinks.
export const SQL_METHODS = [
  ...[
    "execute",
    "execute_batch",
    "query_row",
    "query_one",
    "query_row_and_then",
    "prepare",
    "prepare_cached",
    "prepare_with_flags",
    "pragma_query_value",
    "pragma_query",
    "pragma",
    "pragma_update",
    "pragma_update_and_check",
  ].map((name) => `rusqlite::Connection::${name}`),
  ...[
    "execute",
    "query",
    "query_map",
    "query_and_then",
    "query_row",
    "query_one",
    "raw_execute",
    "raw_query",
  ].map((name) => `rusqlite::Statement::${name}`),
];
// Method spellings are shared by the compiler policy and the structural candidate scan.
const sqlMethodPattern = [
  ...new Set([
    ...SQL_METHODS.map((method) => method.split("::").at(-1)),
    "connection",
  ]),
].join("|");
export const SQL_CLIPPY_CONFIG = SQL_METHODS.map(
  (method) =>
    `  { path = "${method.replace("studio_db_api::", "studio_db_api::query::")}", reason = "sql-inventory: use a typed adapter" },`,
).join("\n");
const root = fileURLToPath(new URL("../../../..", import.meta.url));
const baselinePath = path.join(
  root,
  "workspaces/tooling/apps/repository-checks/sql-baseline.json",
);
const sqlWords =
  /\b(?:SELECT\s|INSERT\s+INTO|REPLACE\s+INTO|UPDATE\s+\w+\s+SET|DELETE\s+FROM|CREATE\s+(?:TEMP\s+|VIRTUAL\s+)?(?:TABLE|TRIGGER|INDEX)|ALTER\s+TABLE|DROP\s+(?:TABLE|TRIGGER|INDEX)|PRAGMA\s|WITH\s+(?:RECURSIVE\s+)?\w+\s+AS\s*\()/i;
export type Family = "F1" | "F2" | "F3" | "F4" | "F5" | "F6" | "F7" | "F8";
export type Owner = Family | "F1-shared";
export type Identity = {
  file: string;
  symbol: string;
  api: string;
  fingerprint: string;
};
export type Allowance = Pick<Identity, "file" | "api" | "fingerprint">;
export type Site = Identity & {
  line: number;
  start: number;
  end: number;
  tables: string[];
  resolution: "compiler" | "structural";
  text: string;
};
type Node = {
  file: string;
  ruleId: string;
  text: string;
  start: number;
  end: number;
  line: number;
};
const FINGERPRINT_HEX_LENGTH = 16;
export function fingerprint(text: string, symbol = ""): string {
  const input = symbol ? `${symbol}\0${text}` : text;
  const hash = createHash("sha256").update(input).digest("hex");
  return hash.slice(0, FINGERPRINT_HEX_LENGTH);
}
const identity = (site: Allowance) =>
  [site.file, site.api, site.fingerprint].join("\0");
// Generated Rust is absent from source. Only the constructor definition owner may spell these names.
export const QUERY_CONSTRUCTOR_OWNER =
  "workspaces/runtime/packages/db-api/src/named.rs";
export function constructorErrors(
  files: { file: string; text: string }[],
): string[] {
  const generated = ["__generated", "declare"].join("_");
  const internal = new RegExp(["Statement", "internal"].join("\\s*::\\s*"));
  return files
    .filter(
      ({ file, text }) =>
        file !== QUERY_CONSTRUCTOR_OWNER &&
        (text.includes(generated) || internal.test(text)),
    )
    .map(
      ({ file }) =>
        `Forbidden generated-query constructor spelling in ${file}; only ${QUERY_CONSTRUCTOR_OWNER} owns constructors`,
    );
}
function counts(rows: Allowance[]): Map<string, number> {
  const found = new Map<string, number>();
  for (const row of rows)
    found.set(identity(row), (found.get(identity(row)) ?? 0) + 1);
  return found;
}
function consume(remaining: Map<string, number>, row: Allowance): boolean {
  const key = identity(row),
    count = remaining.get(key) ?? 0;
  if (count > 0) remaining.set(key, count - 1);
  return count > 0;
}
export function checkSqlRatchet(
  actual: Identity[],
  baseline: Allowance[],
): string[] {
  const remaining = counts(baseline);
  return actual
    .filter((site) => !consume(remaining, site))
    .map(
      (site) =>
        `New raw SQL: ${site.file}::${site.symbol} (${site.api}). Use a typed adapter; infrastructure additions require an explicit reviewed allowance.`,
    );
}
export function tablesIn(text: string, names: string[]): string[] {
  const found = names.filter((name) => new RegExp(`\\b${name}\\b`).test(text));
  return found.length ? found : ["<dynamic>"];
}
export function ownerFor(file: string, families: Family[]): Owner {
  if (/workspaces\/runtime\/packages\/db-api(?:-derive)?\//.test(file))
    return "F1";
  if (
    /\/(?:upgrade-canary|installed-smoke)\/|verify-rust-chat-scale/.test(file)
  )
    return "F1-shared";
  if (/\/(?:stored_text|text_blobs|item_monitor_terminal)\.rs$/.test(file))
    return "F4";
  if (
    /\/(?:supervisor_schema|supervisor_journal|supervisor_protocol|supervisor_contract|supervisor_identity|supervisor_daemon)\.(?:rs|sql)$/.test(
      file,
    )
  )
    return "F8";
  if (
    /\/(?:clock_schema|settings_schema|online_migration|durable_seq|storage_catalog|canvas_schema|feature_schema)\.rs$/.test(
      file,
    ) ||
    /runtime-(?:schema|store)\/src\/(?:lib|contracts|persistence|records|schema)\.rs$/.test(
      file,
    ) ||
    file.endsWith("/migrations/mod.rs")
  )
    return families.length > 1 ? "F1-shared" : "F1";
  const unique = [...new Set(families)];
  if (unique.length > 1) return "F1-shared";
  return unique[0] ?? "F1";
}
function command(bin: string, args: string[]): string {
  const result = spawnSync(bin, args, {
    cwd: root,
    encoding: "utf8",
    maxBuffer: 256 * 1024 * 1024,
  });
  if (
    result.error ||
    (result.status !== 0 && !(bin.endsWith("ast-grep") && result.status === 1))
  )
    throw result.error ?? new Error(`${bin}: ${result.stderr}`);
  return result.stdout;
}
function writeJson(file: string, value: unknown): void {
  writeFileSync(file, JSON.stringify(value, null, 2) + "\n");
}
function object(value: unknown): Record<string, unknown> {
  if (!value || typeof value !== "object" || Array.isArray(value))
    throw new Error("Expected object");
  return Object.fromEntries(Object.entries(value));
}
function string(value: unknown): string {
  if (typeof value !== "string") throw new Error("Expected string");
  return value;
}
function number(value: unknown): number {
  if (typeof value !== "number" || !Number.isInteger(value))
    throw new Error("Expected integer");
  return value;
}
function array(value: unknown): unknown[] {
  if (!Array.isArray(value)) throw new Error("Expected array");
  return value;
}
export function readSqlBaseline(file = baselinePath): Allowance[] {
  const parsed: unknown = JSON.parse(readFileSync(file, "utf8"));
  return Object.entries(object(parsed)).flatMap(([file, apis]) =>
    Object.entries(object(apis)).flatMap(([api, fps]) =>
      array(fps).map((value) => {
        const fp = string(value);
        if (fp.length !== FINGERPRINT_HEX_LENGTH || !/^[a-f0-9]+$/.test(fp))
          throw new Error("Invalid compact SQL fingerprint");
        return { file, api, fingerprint: fp };
      }),
    ),
  );
}
export function formatSqlBaseline(rows: Allowance[]): string {
  const files = Object.groupBy(rows, (row) => row.file);
  const lines = Object.keys(files)
    .sort()
    .map((file) => {
      const apis = Object.groupBy(files[file] ?? [], (row) => row.api);
      const grouped = Object.fromEntries(
        Object.keys(apis)
          .sort()
          .map((api) => [
            api,
            (apis[api] ?? []).map((row) => row.fingerprint).sort(),
          ]),
      );
      return `  ${JSON.stringify(file)}: ${JSON.stringify(grouped)}`;
    });
  return `{\n${lines.join(",\n")}\n}\n`;
}
export function scanRustNodes(paths: string[]): Node[] {
  const attributeChainEnd = {
    not: {
      any: ["attribute_item", "line_comment", "block_comment"].map((kind) => ({
        kind,
      })),
    },
  };
  const dbQueryDerive = {
    all: [
      { pattern: "#[derive($$$DERIVES)]" },
      { has: { kind: "identifier", regex: "^DbQuery$", stopBy: "end" } },
    ],
    stopBy: attributeChainEnd,
  };
  const rules = [
    {
      id: "symbol",
      language: "Rust",
      rule: {
        any: [
          { kind: "function_item" },
          { kind: "const_item" },
          { kind: "static_item" },
          { kind: "impl_item" },
          { kind: "mod_item" },
          { kind: "macro_definition" },
        ],
      },
    },
    ...["$R.$M($$$A)", "$R.$M::<$$$T>($$$A)"].map((pattern) => ({
      id: "call",
      language: "Rust",
      rule: { pattern },
      constraints: {
        M: {
          regex: `^(?:${sqlMethodPattern})$`,
        },
      },
    })),
    {
      id: "call",
      language: "Rust",
      rule: { pattern: "$F($$$A)" },
      constraints: {
        F: {
          regex: `::(?:${sqlMethodPattern})$`,
        },
      },
    },
    {
      id: "typed-macro",
      language: "Rust",
      rule: {
        kind: "macro_invocation",
        regex: "^(?:studio_db_api::)?db_(?:query|update|delete)!",
      },
    },
    {
      id: "declared-query",
      language: "Rust",
      rule: {
        all: [
          { pattern: "#[db_query($$$BEFORE sql = $SQL $$$AFTER)]" },
          { precedes: { kind: "struct_item", stopBy: attributeChainEnd } },
          { any: [{ follows: dbQueryDerive }, { precedes: dbQueryDerive }] },
        ],
      },
      constraints: {
        SQL: {
          any: [{ kind: "string_literal" }, { kind: "raw_string_literal" }],
        },
      },
    },
    {
      id: "sql",
      language: "Rust",
      rule: {
        all: [
          { any: [{ kind: "string_literal" }, { kind: "raw_string_literal" }] },
          { regex: "(?i)" + sqlWords.source.replace(/\\b/g, "") },
        ],
      },
    },
    {
      id: "boundary",
      language: "Rust",
      rule: {
        all: [
          { kind: "use_declaration" },
          { regex: "rusqlite|(?:serde_json|studio_json)::.*Value" },
        ],
      },
    },
    {
      id: "boundary",
      language: "Rust",
      rule: { pattern: "$T::declare($$$A)" },
    },
    {
      id: "boundary",
      language: "Rust",
      rule: {
        all: [
          {
            any: [
              { kind: "scoped_type_identifier" },
              { kind: "scoped_identifier" },
            ],
          },
          { regex: "^(?:serde_json|studio_json)::Value$" },
        ],
      },
    },
  ]
    .map((rule) => JSON.stringify(rule))
    .join("\n---\n");
  const result: Node[] = [];
  for (let offset = 0; offset < paths.length; offset += 200) {
    const parsed: unknown = JSON.parse(
      command(path.join(root, "node_modules/.bin/ast-grep"), [
        "scan",
        "--inline-rules",
        rules,
        "--json=compact",
        ...paths.slice(offset, offset + 200),
      ]) || "[]",
    );
    for (const value of array(parsed)) {
      const row = object(value),
        ruleId = string(row.ruleId),
        selected =
          ruleId === "declared-query"
            ? object(object(object(row.metaVariables).single).SQL)
            : row,
        range = object(selected.range),
        bytes = object(range.byteOffset),
        start = object(range.start);
      result.push({
        file: string(row.file),
        text: string(selected.text),
        ruleId,
        start: number(bytes.start),
        end: number(bytes.end),
        line: number(start.line) + 1,
      });
    }
  }
  const key = (node: Node) => `${node.file}\0${node.start}\0${node.end}`;
  const declared = new Set(
    result.filter((node) => node.ruleId === "declared-query").map(key),
  );
  return result.filter(
    (node) => node.ruleId !== "sql" || !declared.has(key(node)),
  );
}
function scanPython(file: string): (Node & { symbol: string })[] {
  const rule = JSON.stringify({
    id: "python-sql",
    language: "Python",
    rule: {
      any: [
        { kind: "string", regex: "(?i)" + sqlWords.source.replace(/\\b/g, "") },
        {
          kind: "call",
          regex:
            "^(?:[a-zA-Z_][\\w]*\\.)*(?:execute|executemany|executescript)\\(",
        },
      ],
    },
  });
  const parsed: unknown = JSON.parse(
    command(path.join(root, "node_modules/.bin/ast-grep"), [
      "scan",
      "--inline-rules",
      rule,
      "--json=compact",
      file,
    ]) || "[]",
  );
  return array(parsed).map((value) => {
    const row = object(value),
      range = object(row.range),
      bytes = object(range.byteOffset),
      start = object(range.start);
    return {
      file,
      ruleId: "python-sql",
      text: string(row.text),
      start: number(bytes.start),
      end: number(bytes.end),
      line: number(start.line) + 1,
      symbol: "<python-helper>",
    };
  });
}
function family(database: string, source: string, targets: string[]): Family {
  if (database === "Analytics") return "F7";
  if (database === "Supervisor") return "F8";
  if (
    targets.some((target) => /\.(metadata|changes|publications)$/.test(target))
  )
    return "F1";
  if (
    targets.some((target) =>
      /\.(agents|agent_live|sessions|turns)$/.test(target),
    )
  )
    return "F2";
  if (
    targets.some((target) =>
      /\.(queue_events|attempts|limit_resets)$/.test(target),
    )
  )
    return "F3";
  if (
    targets.some((target) =>
      /\.(items|contents|search|monitors|terminals|terminal_chunks|streams)$/.test(
        target,
      ),
    ) ||
    source === "voice_records"
  )
    return "F4";
  if (
    targets.some((target) =>
      /\.(request_results|outbox|incidents)$/.test(target),
    )
  )
    return "F5";
  return "F6";
}
export const SQLITE_DRIVERS = new Set([
  "node:sqlite",
  "better-sqlite3",
  "sqlite3",
  "bun:sqlite",
]);
export function scanTypeScript(
  file: string,
  source: string,
): { text: string; offset: number; symbol: string; driver?: boolean }[] {
  const found: {
    text: string;
    offset: number;
    symbol: string;
    driver?: boolean;
  }[] = [];
  const tree = ts.createSourceFile(file, source, ts.ScriptTarget.Latest, true);
  function visit(node: ts.Node, symbol: string) {
    if (
      (ts.isFunctionDeclaration(node) ||
        ts.isMethodDeclaration(node) ||
        ts.isVariableDeclaration(node)) &&
      node.name
    )
      symbol = node.name.getText(tree);
    if (
      ts.isStringLiteralLike(node) &&
      SQLITE_DRIVERS.has(node.text) &&
      !found.some((site) => site.driver)
    )
      found.push({
        text: node.text,
        offset: 0,
        symbol: "<module>",
        driver: true,
      });
    if (ts.isStringLiteralLike(node) || ts.isTemplateExpression(node)) {
      const text = node.getText(tree);
      // Only static labels directly inside a JSX data array are presentation.
      const property = node.parent;
      if (
        ts.isStringLiteralLike(node) &&
        ts.isPropertyAssignment(property) &&
        property.name.getText(tree) === "label" &&
        ts.isObjectLiteralExpression(property.parent) &&
        ts.isArrayLiteralExpression(property.parent.parent) &&
        ts.isJsxExpression(property.parent.parent.parent) &&
        ts.isJsxAttribute(property.parent.parent.parent.parent) &&
        property.parent.parent.parent.parent.name.getText(tree) === "data"
      )
        return;
      if (
        !file.includes("/generated/") &&
        !file.endsWith("/sql-ratchet.ts") &&
        (sqlWords.test(text) ||
          /\b(?:execute|executemany|executescript)\s*\(/.test(text))
      )
        found.push({ text, offset: node.getStart(tree), symbol });
      // A template's substitutions can have independent SQL calls.
      if (ts.isStringLiteralLike(node)) return;
    }
    ts.forEachChild(node, (child) => visit(child, symbol));
  }
  visit(tree, "<module>");
  return found;
}
export function scanSql() {
  // Git's index is the repository file set; include newly authored source before saving it.
  const files = command("git", [
    "ls-files",
    "--cached",
    "--others",
    "--exclude-standard",
    "-z",
  ])
    .split("\0")
    .filter((file) => file && existsSync(path.join(root, file)))
    .sort();
  const markdown = readFileSync(path.join(root, "docs/storage/map.md"), "utf8");
  const tableFamilies = new Map<string, Set<Family>>();
  const ledgerTables = new Map<string, Set<string>>();
  for (const row of markdown
    .split("\n")
    .filter((line) => /^\| `(Canvas|Analytics|Supervisor)\./.test(line))) {
    const cells = row.split("|");
    const source = cells[1]?.match(
      /`(Canvas|Analytics|Supervisor)\.([a-z_0-9]+)`/,
    );
    if (!source?.[1] || !source[2]) continue;
    const targets = [
      ...(cells[2] ?? "").matchAll(
        /`((?:Canvas|Analytics|Supervisor)\.[a-z_]+)`/g,
      ),
    ].map((match) => string(match[1]));
    const owners = tableFamilies.get(source[2]) ?? new Set<Family>();
    owners.add(family(source[1], source[2], targets));
    tableFamilies.set(source[2], owners);
    for (const entry of row.matchAll(
      /`((?:S|V|H)\/[^`:\s]+\.rs|workspaces\/[^`:\s]+\.(?:rs|ts|py))(?::[^`]*)?`/g,
    )) {
      const short = string(entry[1]);
      const file = short
        .replace(/^S\//, "workspaces/runtime/packages/runtime-store/src/")
        .replace(/^V\//, "workspaces/runtime/packages/runtime-services/src/")
        .replace(/^H\//, "workspaces/runtime/packages/runtime-http/src/");
      const tables = ledgerTables.get(file) ?? new Set<string>();
      tables.add(source[2]);
      ledgerTables.set(file, tables);
    }
  }
  const names = [...tableFamilies.keys()].sort();
  const nodes = scanRustNodes(files.filter((file) => file.endsWith(".rs")));
  const symbols = new Map<string, Node[]>();
  for (const node of nodes.filter((node) => node.ruleId === "symbol")) {
    const entries = symbols.get(node.file) ?? [];
    entries.push(node);
    symbols.set(node.file, entries);
  }
  function symbolAt(file: string, start: number, end: number): string {
    return (
      (symbols.get(file) ?? [])
        .filter(
          (node) =>
            node.file === file && node.start <= start && node.end >= end,
        )
        .sort((a, b) => b.end - b.start - (a.end - a.start))
        .map(
          (node) =>
            node.text
              .match(
                /(?:fn|const|static|mod|macro_rules!)\s*([A-Za-z_][\w]*)|impl(?:<[^>]*>)?\s+([^\n{]+)/,
              )
              ?.slice(1)
              .find(Boolean) ?? "<item>",
        )
        .join("::") || "<module>"
    );
  }
  const descriptorTables = new Map<string, string[]>();
  for (const file of files.filter((file) => file.endsWith(".rs"))) {
    const source = readFileSync(path.join(root, file), "utf8");
    for (const declaration of source.matchAll(
      /#\[db\(table\s*=\s*"([a-z_0-9]+)"[\s\S]*?\bstruct\s+(\w+)/g,
    )) {
      if (declaration[1] && declaration[2])
        descriptorTables.set(declaration[2], [declaration[1]]);
    }
  }
  function referencedTables(text: string): string[] {
    const descriptors = [...descriptorTables]
      .filter(([name]) => new RegExp(`\\b${name}\\b`).test(text))
      .flatMap(([, tables]) => tables);
    return [
      ...new Set([
        ...tablesIn(text, names).filter((table) => table !== "<dynamic>"),
        ...descriptors,
      ]),
    ];
  }
  const sites: Site[] = nodes
    .filter((node) => node.ruleId !== "symbol")
    .map((node) => {
      const tables = referencedTables(node.text);
      const symbol = symbolAt(node.file, node.start, node.end);
      return {
        file: node.file,
        symbol,
        api:
          node.ruleId === "typed-macro" || node.ruleId === "declared-query"
            ? "rust::typed-query-macro"
            : node.ruleId === "call"
              ? "rust::unresolved-sql-candidate"
              : node.ruleId === "boundary"
                ? "rust::persistence-boundary"
                : "rust::sql-template",
        fingerprint: fingerprint(node.text, symbol),
        line: node.line,
        start: node.start,
        end: node.end,
        tables: tables.length ? tables : ["<dynamic>"],
        resolution: "structural",
        text: node.text,
      };
    });
  // TS string/template nodes include embedded Python; comments and generated descriptions are not executors.
  for (const file of files.filter((file) =>
    /\.(?:[cm]?[jt]sx?|py|sql)$/.test(file),
  )) {
    const source = readFileSync(path.join(root, file), "utf8");
    if (file.endsWith(".sql")) {
      sites.push({
        file,
        symbol: "<schema-batch>",
        api: "sqlite::schema",
        fingerprint: fingerprint(source, "<schema-batch>"),
        line: 1,
        start: 0,
        end: Buffer.byteLength(source),
        tables: tablesIn(source, names),
        resolution: "structural",
        text: source,
      });
      continue;
    }
    const add = (
      text: string,
      offset: number,
      symbol: string,
      api = "embedded::sql",
    ) => {
      const start = Buffer.byteLength(source.slice(0, offset));
      const input = api === "typescript::sqlite-driver" ? "file" : text;
      sites.push({
        file,
        symbol,
        api,
        fingerprint: fingerprint(input, symbol),
        line: source.slice(0, offset).split("\n").length,
        start,
        end: start + Buffer.byteLength(text),
        tables: tablesIn(text, names),
        resolution: "structural",
        text,
      });
    };
    if (file.endsWith(".py")) {
      // Python is parsed by the same installed tree-sitter engine as Rust below.
      for (const node of scanPython(file))
        add(
          node.text,
          source.indexOf(
            node.text,
            Buffer.from(source).subarray(0, node.start).toString().length,
          ),
          node.symbol,
        );
      continue;
    }
    for (const node of scanTypeScript(file, source))
      add(
        node.text,
        node.offset,
        node.symbol,
        node.driver ? "typescript::sqlite-driver" : "embedded::sql",
      );
  }
  for (const file of files.filter((file) => file.endsWith("Cargo.toml"))) {
    const source = readFileSync(path.join(root, file), "utf8");
    const text = source
      .split("\n")
      .filter((line) => /\brusqlite\s*[.=]/.test(line))
      .join("\n");
    if (/\brusqlite\s*[.=]/.test(source))
      sites.push({
        file,
        symbol: "<dependency>",
        api: "rust::rusqlite-dependency",
        fingerprint: fingerprint(text, "<dependency>"),
        line: 1,
        start: 0,
        end: Buffer.byteLength(source),
        tables: ["<dynamic>"],
        resolution: "structural",
        text,
      });
  }
  sites.sort(
    (a, b) =>
      a.file.localeCompare(b.file) ||
      a.start - b.start ||
      a.api.localeCompare(b.api),
  );
  const manifest = files.map((file) => {
    const source = /\.(rs|ts|tsx|py|sql)$/.test(file)
      ? readFileSync(path.join(root, file), "utf8")
      : "";
    const tables = [
      ...new Set([
        ...referencedTables(source),
        ...(ledgerTables.get(file) ?? []),
      ]),
    ].sort();
    const families = [
      ...new Set(
        tables.flatMap((table) => [...(tableFamilies.get(table) ?? [])]),
      ),
    ].sort();
    const owner = ownerFor(file, families);
    return {
      file,
      owner,
      families,
      tables,
      reason:
        owner === "F1-shared"
          ? "Shared source or diagnostic entry point; F1 extracts exclusive modules before family conversion"
          : families.length
            ? "Source-table evidence"
            : "F1 integration owner; no statically named source table",
    };
  });
  const forbiddenConstructors = constructorErrors(
    files.map((file) => ({
      file,
      text: readFileSync(path.join(root, file), "utf8"),
    })),
  );
  return { sites, manifest, tableFamilies, symbolAt, forbiddenConstructors };
}

// Clippy supplies resolved method identity, including aliases, deref Transaction and macro expansion.
export function sqlDiagnostic(
  line: string,
): { file: string; start: number; end: number; api: string } | undefined {
  if (!line) return undefined;
  const row = object(JSON.parse(line));
  if (row.reason !== "compiler-message") return undefined;
  const diagnostic = object(row.message);
  if (object(diagnostic.code ?? {}).code !== "clippy::disallowed_methods")
    return undefined;
  const api = string(diagnostic.message)
    .match(/`([^`]+)`/)?.[1]
    ?.replace("studio_db_api::query::", "studio_db_api::");
  if (!api || !SQL_METHODS.includes(api)) return undefined;
  const span = array(diagnostic.spans)
    .map(object)
    .find((span) => span.is_primary === true);
  if (!span) throw new Error("SQL diagnostic without a primary span");
  return {
    file: string(span.file_name),
    start: number(span.byte_start),
    end: number(span.byte_end),
    api,
  };
}

export const COMPILER_SQL_WITNESS_FILE =
  "workspaces/runtime/packages/db-api/tests/sql_inventory.rs";
export function compilerWitnessErrors(
  resolved: Pick<Identity, "file" | "symbol" | "api">[],
): string[] {
  const found = resolved
    .filter(
      (site) =>
        site.file === COMPILER_SQL_WITNESS_FILE &&
        site.api === "rusqlite::Connection::execute",
    )
    .map((site) => site.symbol);
  return [
    "compiler_detects_sql_function_alias",
    "compiler_detects_sql_in_macro_argument",
  ]
    .filter((symbol) => !found.includes(symbol))
    .map(
      (symbol) =>
        `SQL compiler coverage missing: ${symbol}; the landing ratchet must resolve aliases and macro arguments`,
    );
}
export function checkResolvedSql(
  lines: string[],
  destination?: string,
): string[] {
  const scan = scanSql();
  const unique = new Map<string, ReturnType<typeof sqlDiagnostic>>();
  for (const line of lines) {
    const diagnostic = sqlDiagnostic(line);
    if (diagnostic)
      unique.set(
        `${diagnostic.file}:${diagnostic.start}:${diagnostic.end}:${diagnostic.api}`,
        diagnostic,
      );
  }
  const resolved: Site[] = [];
  for (const diagnostic of unique.values()) {
    if (!diagnostic || !existsSync(path.join(root, diagnostic.file))) continue;
    const call = scan.sites
      .filter(
        (site) =>
          site.file === diagnostic.file &&
          site.start <= diagnostic.start &&
          site.end >= diagnostic.end &&
          site.api === "rust::unresolved-sql-candidate",
      )
      .sort((a, b) => a.end - a.start - (b.end - b.start))[0];
    const source = readFileSync(path.join(root, diagnostic.file));
    const text =
      call?.text ??
      source.subarray(diagnostic.start, diagnostic.end).toString("utf8");
    const symbol =
      call?.symbol ??
      scan.symbolAt(diagnostic.file, diagnostic.start, diagnostic.end);
    resolved.push({
      ...diagnostic,
      symbol,
      fingerprint: fingerprint(text, symbol),
      line:
        call?.line ??
        source.subarray(0, diagnostic.start).toString("utf8").split("\n")
          .length,
      tables: call?.tables ?? ["<dynamic>"],
      resolution: "compiler",
      text,
    });
  }
  const actual = resolved.filter((site) => !infrastructure(site.file));
  const baseline = readSqlBaseline();
  if (destination)
    writeJson(destination, {
      version: 1,
      sourceRevision: command("git", ["rev-parse", "HEAD"]).trim(),
      compilerScope: "workspace all-targets; native excluded",
      sites: [...scan.sites, ...resolved].map(
        ({ text: _text, ...site }) => site,
      ),
    });
  console.log(`SQL compiler inventory: ${resolved.length} resolved call sites`);
  return [
    ...checkSqlRatchet(actual, baseline),
    ...scan.forbiddenConstructors,
    ...compilerWitnessErrors(resolved),
  ];
}
function infrastructure(file: string): boolean {
  return /^workspaces\/runtime\/packages\/db-api(?:-derive)?\//.test(file);
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  const scan = scanSql();
  const actual = scan.sites.filter(
    (site) =>
      !infrastructure(site.file) && site.api !== "rust::typed-query-macro",
  );
  if (process.argv.includes("--shrink")) {
    const remaining = counts(actual);
    writeFileSync(
      baselinePath,
      formatSqlBaseline(
        readSqlBaseline().filter(
          (row) => SQL_METHODS.includes(row.api) || consume(remaining, row),
        ),
      ),
    );
  }
  const errors = checkSqlRatchet(actual, readSqlBaseline()).concat(
    scan.forbiddenConstructors,
  );
  for (const error of errors) console.error(error);
  for (const [flag, value] of [
    [
      "--inventory",
      { version: 1, sites: scan.sites.map(({ text: _text, ...site }) => site) },
    ],
    ["--owners", { version: 1, files: scan.manifest }],
  ] satisfies [string, unknown][]) {
    const index = process.argv.indexOf(flag);
    if (index < 0) continue;
    const destination = process.argv[index + 1];
    if (!destination) throw new Error(`${flag} requires an output path`);
    writeJson(destination, value);
  }
  console.log(
    `SQL ratchet: ${scan.sites.length} sites, ${scan.manifest.length} exclusively owned files, ${errors.length} additions`,
  );
  process.exitCode = errors.length ? 1 : 0;
}
