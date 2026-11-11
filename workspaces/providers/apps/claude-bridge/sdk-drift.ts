import { z } from "zod";
// Compare published SDK exports without changing the installed dependency.
import { readFile, mkdtemp, rm } from "node:fs/promises";
import { createRequire } from "node:module";
import { tmpdir } from "node:os";
import path from "node:path";
import { execFile } from "node:child_process";
import { promisify } from "node:util";
import ts from "typescript";
import { CLAUDE_SDK_VERSION } from "./sdk-dispositions.ts";

const run = promisify(execFile);
const require = createRequire(import.meta.url);
const declaration = require
  .resolve("@anthropic-ai/claude-agent-sdk")
  .replace(/sdk\.mjs$/, "sdk.d.ts");

function exportsOf(source: string) {
  const ast = ts.createSourceFile(
    "sdk.d.ts",
    source,
    ts.ScriptTarget.Latest,
    true,
  );
  const printer = ts.createPrinter({ removeComments: true });
  return new Map(
    ast.statements.flatMap((node) => {
      if (
        ts.isExportDeclaration(node) &&
        node.exportClause &&
        ts.isNamedExports(node.exportClause)
      )
        return node.exportClause.elements.map((entry) => [
          entry.name.text,
          printer.printNode(ts.EmitHint.Unspecified, entry, ast),
        ]);
      if (
        !ts.canHaveModifiers(node) ||
        !ts
          .getModifiers(node)
          ?.some((modifier) => modifier.kind === ts.SyntaxKind.ExportKeyword)
      )
        return [];
      if (ts.isVariableStatement(node))
        return node.declarationList.declarations.flatMap((entry) =>
          ts.isIdentifier(entry.name)
            ? [
                [
                  entry.name.text,
                  printer.printNode(ts.EmitHint.Unspecified, entry, ast),
                ],
              ]
            : [],
        );
      if (
        !(
          ts.isTypeAliasDeclaration(node) ||
          ts.isInterfaceDeclaration(node) ||
          ts.isFunctionDeclaration(node) ||
          ts.isClassDeclaration(node) ||
          ts.isEnumDeclaration(node) ||
          ts.isModuleDeclaration(node)
        ) ||
        !node.name
      )
        return [];
      return [
        [node.name.text, printer.printNode(ts.EmitHint.Unspecified, node, ast)],
      ];
    }),
  );
}

export function sdkExportDrift(pinned: string, candidate: string) {
  const before = exportsOf(pinned),
    after = exportsOf(candidate);
  return {
    added: [...after.keys()].filter((name) => !before.has(name)),
    removed: [...before.keys()].filter((name) => !after.has(name)),
    changed: [...before.keys()].filter(
      (name) => after.has(name) && before.get(name) !== after.get(name),
    ),
  };
}

const packMetadata = z.object({
  filename: z.string().min(1),
  version: z.string().min(1),
});
export function sdkPackMetadata(value: unknown) {
  // npm emits either a one-package list or a receipt keyed by package name.
  const first: unknown = Array.isArray(value)
    ? z.array(z.unknown()).length(1).parse(value)[0]
    : z.object({ "@anthropic-ai/claude-agent-sdk": z.unknown() }).parse(value)[
        "@anthropic-ai/claude-agent-sdk"
      ];
  const result = packMetadata.parse(first);
  if (path.basename(result.filename) !== result.filename)
    throw new Error("Invalid SDK archive filename");
  return result;
}

async function main() {
  const pinned = await readFile(declaration, "utf8");
  const installed: unknown = JSON.parse(
    await readFile(declaration.replace(/sdk\.d\.ts$/, "package.json"), "utf8"),
  );
  if (
    !installed ||
    typeof installed !== "object" ||
    !("version" in installed) ||
    installed.version !== CLAUDE_SDK_VERSION
  )
    throw new Error(
      "Installed SDK differs from the recorded disposition version",
    );
  const candidatePath = process.argv[2];
  if (candidatePath && candidatePath !== "--latest") {
    console.log(
      JSON.stringify(
        {
          pinned: CLAUDE_SDK_VERSION,
          candidate: candidatePath,
          ...sdkExportDrift(pinned, await readFile(candidatePath, "utf8")),
        },
        null,
        2,
      ),
    );
    return;
  }
  const temporary = await mkdtemp(path.join(tmpdir(), "studio-sdk-drift-"));
  try {
    // npm pack resolves the registry's latest package without running lifecycle
    // scripts or touching the workspace manifest or lockfile.
    const { stdout } = await run(
      "npm",
      [
        "pack",
        "@anthropic-ai/claude-agent-sdk@latest",
        "--ignore-scripts",
        "--pack-destination",
        temporary,
        "--json",
      ],
      { maxBuffer: 8 * 1024 * 1024 },
    );
    const pack: unknown = JSON.parse(stdout);
    const first = sdkPackMetadata(pack);
    const { stdout: candidate } = await run(
      "tar",
      ["-xOf", path.join(temporary, first.filename), "package/sdk.d.ts"],
      { maxBuffer: 16 * 1024 * 1024 },
    );
    console.log(
      JSON.stringify(
        {
          pinned: CLAUDE_SDK_VERSION,
          candidate: first.version,
          ...sdkExportDrift(pinned, candidate),
        },
        null,
        2,
      ),
    );
  } finally {
    await rm(temporary, { recursive: true, force: true });
  }
}
if (
  process.argv[1] &&
  path.resolve(process.argv[1]) === new URL(import.meta.url).pathname
)
  await main();
