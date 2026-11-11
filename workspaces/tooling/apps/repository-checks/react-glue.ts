import ts from "typescript";
import { readFileSync, readdirSync, writeFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

export const WEB_SOURCE = "workspaces/client/apps/web/src/";
export const GLUE_MANIFEST =
  "workspaces/tooling/apps/repository-checks/react-glue-exceptions.json";
// Owner: maximum grandfathered React glue source files; only lower this ceiling.
const MAX_REACT_GLUE_EXCEPTIONS = 36;
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
export function glueUses(file: string, text: string): string[] {
  const found = new Set<string>();
  const source = ts.createSourceFile(file, text, ts.ScriptTarget.Latest, true);
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
    ts.forEachChild(node, walk);
  };
  walk(source);
  return [...found].sort();
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
