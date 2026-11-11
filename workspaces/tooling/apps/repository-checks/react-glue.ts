import { spawnSync } from "node:child_process";
import { withRatchetBase } from "./ratchet.ts";
import ts from "typescript";
import { readFileSync, readdirSync, writeFileSync, existsSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

export const WEB_SOURCE = "workspaces/client/apps/web/src/";
export const HOOK_MANIFEST =
  "workspaces/tooling/apps/repository-checks/react-hook-ceilings.json";
export const COUNTED_HOOKS = ["useMemo", "useCallback", "useRef"];
export type HookCounts = Record<string, number>;
export type HookCeilings = Record<string, HookCounts>;
export const GLUE_MANIFEST =
  "workspaces/tooling/apps/repository-checks/react-glue-exceptions.json";
// Owner: maximum grandfathered React glue source files; only lower this ceiling.
export const MAX_REACT_GLUE_EXCEPTIONS = 34;
export const REACT_GLUE_APIS = [
  "useEffect",
  "useLayoutEffect",
  "useSyncExternalStore",
  "useReducer",
  "useInsertionEffect",
];
export const GLUE_MESSAGE =
  "Use MobX owners/reactions; keep only DOM integration exceptions. See #994 effects-wave-brief.md.";
const excluded = [
  "**/*.test.*",
  "**/*.spec.*",
  "**/*.type-test.*",
  "**/*.testHelper.*",
  "**/*.fixture.*",
  "**/*.fixtures.*",
  "**/*.stories.*",
  "**/*Fixture.*",
  "**/testFixture.*",
  "**/tests/**",
  "**/fixtures/**",
  "**/*Fixtures.*",
  "**/*.test-entry.*",
];
export function isGlueSource(file: string): boolean {
  return (
    file.startsWith(WEB_SOURCE) &&
    /\.[cm]?[jt]sx?$/.test(file) &&
    !/(?:\.(?:test|spec|type-test|testHelper|fixture|fixtures|stories|test-entry)\.|Fixtures?\.|\/testFixture\.|\/(?:tests|fixtures)\/)/.test(
      file,
    )
  );
}

/** Syntax only: no compiler program or module resolution is needed. */
function inspectGlue(file: string, text: string) {
  const found = new Set<string>();
  const source = ts.createSourceFile(file, text, ts.ScriptTarget.Latest, true);
  const counts: HookCounts = {};
  const aliases = new Map<string, string>();
  const namespaces = new Set<string>();
  for (const node of source.statements) {
    if (
      !ts.isImportDeclaration(node) ||
      !ts.isStringLiteral(node.moduleSpecifier) ||
      node.moduleSpecifier.text !== "react" ||
      node.importClause?.isTypeOnly
    )
      continue;
    const clause = node.importClause;
    if (clause?.name) namespaces.add(clause.name.text);
    const bindings = clause?.namedBindings;
    if (bindings && ts.isNamespaceImport(bindings))
      namespaces.add(bindings.name.text);
    if (bindings && ts.isNamedImports(bindings))
      for (const item of bindings.elements) {
        const name = (item.propertyName ?? item.name).text;
        if (!item.isTypeOnly && name === "default")
          namespaces.add(item.name.text);
        if (!item.isTypeOnly && COUNTED_HOOKS.includes(name))
          aliases.set(item.name.text, name);
      }
  }
  const collectAliases = (node: ts.Node) => {
    if (
      ts.isVariableDeclaration(node) &&
      node.initializer &&
      ts.isIdentifier(node.initializer) &&
      namespaces.has(node.initializer.text) &&
      ts.isObjectBindingPattern(node.name)
    )
      for (const item of node.name.elements) {
        const name = item.propertyName ?? item.name;
        if (
          ts.isIdentifier(name) &&
          ts.isIdentifier(item.name) &&
          COUNTED_HOOKS.includes(name.text)
        )
          aliases.set(item.name.text, name.text);
      }
    ts.forEachChild(node, collectAliases);
  };
  collectAliases(source);
  const walk = (node: ts.Node) => {
    if (
      ts.isImportDeclaration(node) &&
      ts.isStringLiteral(node.moduleSpecifier) &&
      node.moduleSpecifier.text === "react" &&
      !node.importClause?.isTypeOnly
    ) {
      const bindings = node.importClause?.namedBindings;
      if (bindings && ts.isNamedImports(bindings))
        for (const item of bindings.elements)
          if (
            !item.isTypeOnly &&
            REACT_GLUE_APIS.includes((item.propertyName ?? item.name).text)
          )
            found.add((item.propertyName ?? item.name).text);
    }
    if (
      ts.isNewExpression(node) &&
      ts.isIdentifier(node.expression) &&
      node.expression.text === "Set"
    ) {
      const type = node.typeArguments?.[0];
      const callback =
        type &&
        ts.isFunctionTypeNode(type) &&
        type.parameters.length === 0 &&
        type.type.kind === ts.SyntaxKind.VoidKeyword;
      // Also catch untyped or aliased listener sets in a subscribe/notify implementation.
      const pubsub =
        /subscribe/i.test(text) &&
        /notify/i.test(text) &&
        /listener|subscriber|observer|callback/i.test(
          node.parent.getText(source),
        );
      if (callback || pubsub) found.add("listener pub-sub");
    }
    if (ts.isCallExpression(node)) {
      const expression = node.expression;
      const name = ts.isIdentifier(expression)
        ? aliases.get(expression.text)
        : ts.isPropertyAccessExpression(expression) &&
            ts.isIdentifier(expression.expression) &&
            namespaces.has(expression.expression.text) &&
            COUNTED_HOOKS.includes(expression.name.text)
          ? expression.name.text
          : undefined;
      if (name) counts[name] = (counts[name] ?? 0) + 1;
    }
    ts.forEachChild(node, walk);
  };
  walk(source);
  return { uses: [...found].sort(), counts };
}

export function glueUses(file: string, text: string): string[] {
  return inspectGlue(file, text).uses;
}

export function hookCounts(file: string, text: string): HookCounts {
  return inspectGlue(file, text).counts;
}

export function hookInventory(root: string, files: string[]): HookCeilings {
  return Object.fromEntries(
    files
      .filter(isGlueSource)
      .sort()
      .map(
        (file) =>
          [
            file,
            hookCounts(file, readFileSync(path.join(root, file), "utf8")),
          ] as const,
      )
      .filter(([, counts]) => Object.keys(counts).length),
  );
}

export function readHookCeilings(text: string): HookCeilings {
  const parsed: unknown = JSON.parse(text);
  if (!parsed || typeof parsed !== "object" || Array.isArray(parsed))
    throw new Error("Malformed React hook ceilings");
  const result: HookCeilings = {};
  for (const [file, counts] of Object.entries(parsed)) {
    if (
      !isGlueSource(file) ||
      !counts ||
      typeof counts !== "object" ||
      Array.isArray(counts)
    )
      throw new Error(`Malformed React hook ceilings in ${file}`);
    result[file] = {};
    for (const [hook, count] of Object.entries(counts)) {
      if (
        (!COUNTED_HOOKS.includes(hook) && hook !== "useState") ||
        typeof count !== "number" ||
        !Number.isSafeInteger(count) ||
        count <= 0
      )
        throw new Error(`Malformed React hook ceiling for ${file} ${hook}`);
      // Historical manifests predate the policy permitting trivial React state.
      if (hook !== "useState") result[file][hook] = count;
    }
  }
  return result;
}

export function hookCeilingErrors(
  actual: HookCeilings,
  ceilings: HookCeilings,
): string[] {
  return Object.entries(actual).flatMap(([file, counts]) =>
    Object.entries(counts)
      .filter(([hook, count]) => count > (ceilings[file]?.[hook] ?? 0))
      .map(
        ([hook, count]) =>
          `${file}: ${hook} count ${count} exceeds ceiling ${ceilings[file]?.[hook] ?? 0}. Keep trivial local UI state in React useState; move async/derived/shared logic into MobX stores. ${GLUE_MESSAGE}`,
      ),
  );
}

export function lowerHookCeilings(
  actual: HookCeilings,
  ceilings: HookCeilings,
): HookCeilings {
  const errors = hookCeilingErrors(actual, ceilings);
  if (errors.length) throw new Error(errors.join("\n"));
  return actual;
}

export function previousHookCeilings(root: string): HookCeilings {
  const historical = withRatchetBase(root, HOOK_MANIFEST, (snapshot) => {
    const manifest = path.join(snapshot, HOOK_MANIFEST);
    return existsSync(manifest)
      ? readHookCeilings(readFileSync(manifest, "utf8"))
      : hookInventory(snapshot, glueFiles(snapshot));
  });
  const head = spawnSync("git", ["show", `HEAD:${HOOK_MANIFEST}`], {
    cwd: root,
    encoding: "utf8",
  });
  if (head.status !== 0) return historical;
  const committed = readHookCeilings(head.stdout);
  return Object.fromEntries(
    Object.entries(historical).map(([file, counts]) => [
      file,
      Object.fromEntries(
        Object.entries(counts).map(([hook, count]) => [
          hook,
          Math.min(count, committed[file]?.[hook] ?? 0),
        ]),
      ),
    ]),
  );
}

export function hookManifestErrors(
  root: string,
  files: string[],
  previous?: HookCeilings,
): string[] {
  const ceilings = readHookCeilings(
    readFileSync(path.join(root, HOOK_MANIFEST), "utf8"),
  );
  return [
    ...hookCeilingErrors(hookInventory(root, files), ceilings),
    ...(previous ? hookCeilingErrors(ceilings, previous) : []),
  ];
}

export function glueFiles(root: string): string[] {
  return readdirSync(path.join(root, WEB_SOURCE), {
    recursive: true,
    encoding: "utf8",
  })
    .map((file) => WEB_SOURCE + file)
    .filter(isGlueSource)
    .sort();
}
export function glueManifestErrors(root: string, files: string[]): string[] {
  const manifest: Record<string, string> = JSON.parse(
    readFileSync(path.join(root, GLUE_MANIFEST), "utf8"),
  );
  const actual = files
    .filter(isGlueSource)
    .filter(
      (file) =>
        glueUses(file, readFileSync(path.join(root, file), "utf8")).length,
    );
  const errors = actual
    .filter((file) => !Object.hasOwn(manifest, file))
    .map((file) => `${file}: ${GLUE_MESSAGE}`);
  if (Object.keys(manifest).length > MAX_REACT_GLUE_EXCEPTIONS)
    errors.push("React glue exception count grew");
  for (const file of Object.keys(manifest))
    if (!actual.includes(file))
      errors.push(`${file}: remove stale React glue exception`);
  return errors;
}

type Override = {
  files: string[];
  excludeFiles?: string[];
  rules: Record<string, unknown>;
};
export type GlueConfig = { overrides: Override[] };
/** The last two overrides are generated from the single manifest owner. */
export function glueOverrides(
  config: GlueConfig,
  manifest: Record<string, string>,
): Override[] {
  const base = config.overrides.filter(
    (item) =>
      !item.rules["no-restricted-imports"] ||
      !JSON.stringify(item.rules["no-restricted-imports"] ?? null).includes(
        GLUE_MESSAGE,
      ),
  );
  const network = base.find((item) =>
    item.files.includes(WEB_SOURCE + "**/*.{ts,tsx,js,jsx}"),
  );
  if (!network) throw new Error("Missing network boundary owner");
  const exemptions = base
    .filter((item) => item.rules["no-restricted-imports"] === "off")
    .flatMap((item) => item.files)
    .filter((file) => file.startsWith(WEB_SOURCE));
  const react = {
    name: "react",
    importNames: REACT_GLUE_APIS,
    message: GLUE_MESSAGE,
  };
  const networkRule = network.rules["no-restricted-imports"];
  if (!Array.isArray(networkRule) || typeof networkRule[1] !== "object")
    throw new Error("Missing network import rule");
  const excludeFiles = [...excluded, ...Object.keys(manifest)];
  return [
    {
      files: [WEB_SOURCE + "**/*.{ts,tsx,js,jsx}"],
      excludeFiles,
      rules: {
        "no-restricted-imports": [
          "error",
          { ...networkRule[1], paths: [react] },
        ],
      },
    },
    {
      files: exemptions,
      excludeFiles: [
        ...excludeFiles,
        ...base.flatMap((item) => item.excludeFiles ?? []),
      ],
      rules: { "no-restricted-imports": ["error", { paths: [react] }] },
    },
  ];
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  const root = fileURLToPath(new URL("../../../..", import.meta.url));
  if (process.argv.includes("--update") || process.argv.includes("--check")) {
    const files = glueFiles(root);
    const previous = previousHookCeilings(root);
    const growth = hookManifestErrors(root, files, previous);
    if (growth.length) throw new Error(growth.join("\n"));
    if (process.argv.includes("--update")) {
      const ceilings = readHookCeilings(
        readFileSync(path.join(root, HOOK_MANIFEST), "utf8"),
      );
      const updated = lowerHookCeilings(hookInventory(root, files), ceilings);
      writeFileSync(
        path.join(root, HOOK_MANIFEST),
        JSON.stringify(updated, null, 2) + "\n",
      );
    } else {
      const errors = hookManifestErrors(root, files, previous);
      for (const error of errors) console.error(error);
      process.exitCode = errors.length ? 1 : 0;
    }
  } else {
    const config: GlueConfig = JSON.parse(
      readFileSync(path.join(root, ".oxlintrc.json"), "utf8"),
    );
    const manifest: Record<string, string> = JSON.parse(
      readFileSync(path.join(root, GLUE_MANIFEST), "utf8"),
    );
    config.overrides = config.overrides.filter(
      (item) =>
        !JSON.stringify(item.rules["no-restricted-imports"] ?? null).includes(
          GLUE_MESSAGE,
        ),
    );
    config.overrides.push(...glueOverrides(config, manifest));
    writeFileSync(
      path.join(root, ".oxlintrc.json"),
      JSON.stringify(config, null, 2) + "\n",
    );
  }
}
