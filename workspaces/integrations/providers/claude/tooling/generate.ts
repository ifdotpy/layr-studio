import { readFileSync, writeFileSync, existsSync } from "node:fs";
import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import path from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
type ObjectValue = Record<string, unknown>;
type Reader = (file: string) => string;
interface Rendered {
  path: string;
  text: string;
}
const root = fileURLToPath(new URL("../../../../..", import.meta.url));
const claudeBridge = "workspaces/integrations/providers/claude/bridge";
const claudeSchema =
  "workspaces/integrations/providers/claude/vendor/schemas/claude-rate-limit.schema.json";
const claudeProvenance =
  "workspaces/integrations/providers/claude/vendor/schemas/claude-rate-limit.provenance.json";
const claudeInputs = [
  `${claudeBridge}/rate-limit-input.ts`,
  `${claudeBridge}/usage-policy.ts`,
  `${claudeBridge}/package.json`,
  "pnpm-lock.yaml",
  "workspaces/integrations/providers/claude/tooling/generate.ts",
];
const sha256 = (text: string) =>
  createHash("sha256").update(text).digest("hex");
const read: Reader = (file) => readFileSync(file, "utf8");
function writeGenerated(file: string, text: string): void {
  if (!existsSync(file) || read(file) !== text) writeFileSync(file, text);
}
function object(value: unknown): ObjectValue {
  if (!value || typeof value !== "object" || Array.isArray(value))
    throw new Error("Expected vendor schema object");
  return value as ObjectValue;
}
function parse(text: string): ObjectValue {
  return object(JSON.parse(text));
}
function claudeSourceHashes(directory: string) {
  return Object.fromEntries(
    claudeInputs.map((file) => [
      file,
      sha256(read(path.join(directory, file))),
    ]),
  );
}
function claudeVersion(directory: string): string {
  const version = object(
    parse(read(path.join(directory, claudeBridge, "package.json")))
      .dependencies,
  )["@anthropic-ai/claude-agent-sdk"];
  if (typeof version !== "string")
    throw new Error("Missing pinned Claude SDK version");
  return version;
}
// Cargo needs only committed inputs. Full generation/check additionally bind the
// installed declaration to this pin; offline checks never resolve node_modules.
export function checkClaudeRateFingerprint(directory = root): void {
  const saved = parse(read(path.join(directory, claudeProvenance)));
  const sdk = object(saved.sdk);
  if (
    JSON.stringify(saved.sources) !==
      JSON.stringify(claudeSourceHashes(directory)) ||
    saved.schemaSha256 !== sha256(read(path.join(directory, claudeSchema))) ||
    sdk.version !== claudeVersion(directory) ||
    typeof sdk.declarationsSha256 !== "string" ||
    !/^[a-f0-9]{64}$/.test(sdk.declarationsSha256)
  )
    throw new Error(
      "Stale Claude rate provenance; run pnpm run contracts:vendor:generate",
    );
}
function renderClaudeProvenance(
  directory: string,
  schema: Rendered,
  reader = read,
): Rendered {
  const sdkDirectory = path.join(
    directory,
    claudeBridge,
    "node_modules/@anthropic-ai/claude-agent-sdk",
  );
  const version = claudeVersion(directory);
  if (parse(read(path.join(sdkDirectory, "package.json"))).version !== version)
    throw new Error("Installed Claude SDK differs from its pinned source");
  return {
    path: claudeProvenance,
    text:
      JSON.stringify(
        {
          $comment:
            "Owner: claude/tooling/generate.ts. Declaration digest is checked by full generation; committed inputs and artifact are checked offline by Cargo.",
          sources: claudeSourceHashes(directory),
          sdk: {
            version,
            declarationsSha256: sha256(
              reader(path.join(sdkDirectory, "sdk.d.ts")),
            ),
          },
          schemaSha256: sha256(schema.text),
        },
        null,
        2,
      ) + "\n",
  };
}
async function renderClaudeRateContract(directory: string): Promise<Rendered> {
  const owner: typeof import("../bridge/rate-limit-input.ts") = await import(
    pathToFileURL(
      path.join(
        directory,
        "workspaces/integrations/providers/claude/bridge/rate-limit-input.ts",
      ),
    ).href
  );
  const version = claudeVersion(directory);
  const target = claudeSchema;
  const text =
    JSON.stringify(
      {
        ...owner.rateLimitJsonSchema(),
        $comment: `Owner: rate-limit-input.ts SDK projection, @anthropic-ai/claude-agent-sdk ${version}.`,
      },
      null,
      2,
    ) + "\n";
  return {
    path: target,
    text: execFileSync(
      path.join(directory, "node_modules/.bin/oxfmt"),
      [
        `--stdin-filepath=${target}`,
        `--config=${path.join(directory, ".oxfmtrc.json")}`,
      ],
      { input: text, encoding: "utf8" },
    ),
  };
}
export async function checkClaudeRateContract(
  directory = root,
  reader = read,
): Promise<void> {
  const entry = await renderClaudeRateContract(directory);
  if (reader(path.join(directory, entry.path)) !== entry.text)
    throw new Error(
      `Stale Claude rate contract: ${entry.path}; run pnpm run contracts:vendor:generate`,
    );
  const provenance = renderClaudeProvenance(directory, entry, reader);
  if (reader(path.join(directory, provenance.path)) !== provenance.text)
    throw new Error(
      "Stale Claude rate provenance; run pnpm run contracts:vendor:generate",
    );
}

if (
  process.argv[1] &&
  path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)
) {
  if (process.argv.includes("--cargo-inputs"))
    for (const file of [...claudeInputs, claudeSchema, claudeProvenance])
      process.stdout.write(
        "cargo:rerun-if-changed=" + path.join(root, file) + "\n",
      );
  if (!process.argv.includes("--rust-only")) {
    if (process.argv.includes("--write")) {
      const entry = await renderClaudeRateContract(root);
      writeGenerated(path.join(root, entry.path), entry.text);
      const provenance = renderClaudeProvenance(root, entry);
      writeGenerated(path.join(root, provenance.path), provenance.text);
    } else await checkClaudeRateContract(root);
  }
  checkClaudeRateFingerprint(root);
}
