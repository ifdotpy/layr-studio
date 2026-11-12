import { execFileSync } from "node:child_process";
import { readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = fileURLToPath(new URL("../../../..", import.meta.url));
const sg = path.join(root, "node_modules/.bin/ast-grep");
const requestConstructors =
  "ProviderRequest|TransferProviderRequest|RequestSpec|RequestFrame|VoiceRpcCall";

// Match AST expressions and macro token-tree send boundaries, not strings or imports.
// Plain session.request is scoped to account_session_runtime.rs and receiver session;
// other .request APIs are unrelated. Aliases outside a recognized send boundary
// remain undetected; this deliberately does not resolve arbitrary imports/re-exports.
const rules = (scopedSession = false) => `
id: internal-sender
language: Rust
rule:
  all:
    - any:
        - all:
            - kind: struct_expression
            - has:
                field: name
                regex: '^(::)?([a-z_][a-z0-9_]*::)*(${requestConstructors})$|(^|::)ClientRequest::'
        # Rust macro arguments are token trees, rather than struct_expression nodes.
        - all:
            - kind: identifier
            - regex: '^(${requestConstructors})$'
            - inside:
                kind: macro_invocation
                stopBy: end
            - not:
                follows:
                  regex: '^::$'
                  follows:
                    kind: identifier
                    regex: '^[A-Z]'
            - precedes:
                kind: token_tree
                regex: '^[{]'
        - all:
            - kind: identifier
            - any:
                - regex: '^(provider_request|transfer_provider_request)$'
${
  scopedSession
    ? `                - all:
                    - regex: '^request$'
                    - follows:
                        regex: '^\\.$'
                        follows:
                          kind: identifier
                          regex: '^session$'`
    : ""
}
            - follows:
                regex: '^\\.$'
            - precedes:
                kind: token_tree
                regex: '^[(]'
            - inside:
                kind: macro_invocation
                stopBy: end
        - all:
            - kind: call_expression
            - has:
                field: function
                regex: '(^|::)Frame::(Request|CodexClientRequest)$|(::|\\.)(provider_request|transfer_provider_request|prepare_frame|prepare_callback_reply|operation_status)$|(^|::)write_frame$${scopedSession ? "|^session\\s*\\.\\s*request$" : ""}'
    - not:
        inside:
          kind: mod_item
          any:
            - has:
                field: name
                regex: '(^|_)tests$'
            - follows:
                kind: attribute_item
                regex: '^#\\[cfg\\(test\\)\\]'
                stopBy:
                  not:
                    kind: attribute_item
          stopBy: end
`;

export interface SenderSite {
  file: string;
  line: number;
  offset: number;
}

interface AstMatch {
  file: string;
  text: string;
  range: { byteOffset: { start: number }; start: { line: number } };
}

function matches(rule: string, files: string[], cwd: string): AstMatch[] {
  if (!files.length) return [];
  const output: unknown = JSON.parse(
    execFileSync(
      sg,
      ["scan", "--inline-rules", rule, "--json=compact", ...files],
      {
        cwd,
        encoding: "utf8",
        maxBuffer: 32 * 1024 * 1024,
      },
    ),
  );
  if (!Array.isArray(output)) throw new Error("Invalid sender AST matches");
  return output.map((item: unknown) => {
    if (
      !item ||
      typeof item !== "object" ||
      !("file" in item) ||
      typeof item.file !== "string" ||
      !("text" in item) ||
      typeof item.text !== "string" ||
      !("range" in item) ||
      !item.range ||
      typeof item.range !== "object" ||
      !("byteOffset" in item.range) ||
      !item.range.byteOffset ||
      typeof item.range.byteOffset !== "object" ||
      !("start" in item.range.byteOffset) ||
      typeof item.range.byteOffset.start !== "number" ||
      !("start" in item.range) ||
      !item.range.start ||
      typeof item.range.start !== "object" ||
      !("line" in item.range.start) ||
      typeof item.range.start.line !== "number"
    )
      throw new Error("Invalid sender AST match");
    return {
      file: item.file,
      text: item.text,
      range: {
        byteOffset: { start: item.range.byteOffset.start },
        start: { line: item.range.start.line },
      },
    };
  });
}

export function discoverSenderSites(files: string[], cwd = root): SenderSite[] {
  const eligible = files.filter(
    (file) =>
      file.endsWith(".rs") &&
      !/(^|\/)(tests|tests\.rs)|_tests\.rs$/.test(file) &&
      (!file.startsWith("workspaces/") ||
        /workspaces\/runtime\/(apps\/runtime-server|packages\/runtime-(services|http))\//.test(
          file,
        )),
  );
  const scoped = (file: string) =>
    path.basename(file) === "account_session_runtime.rs";
  const sites = [
    ...matches(
      rules(false),
      eligible.filter((file) => !scoped(file)),
      cwd,
    ),
    ...matches(rules(true), eligible.filter(scoped), cwd),
  ].map((match) => ({
    file: match.file,
    line: match.range.start.line + 1,
    offset: match.range.byteOffset.start,
  }));
  return [
    ...new Map(
      sites.map((site) => [`${site.file}:${site.line}`, site]),
    ).values(),
  ];
}

export function senderWitnessErrors(files: string[], cwd = root): string[] {
  const errors: string[] = [];
  const names = new Map<string, Set<string>>();
  for (const site of discoverSenderSites(files, cwd)) {
    const source = readFileSync(path.join(cwd, site.file), "utf8").split("\n");
    const annotation = source[site.line - 2]
      ?.trim()
      .match(
        /^\/\/ internal-sender-witness: (\S+\.rs)#(internal_sender_witness_\w+)$/,
      );
    const location = `${site.file}:${site.line}`;
    if (!annotation) {
      errors.push(`${location}: missing internal-sender-witness annotation`);
      continue;
    }
    const [, witnessFile, witness] = annotation;
    if (!witnessFile || !witness) throw new Error("Invalid sender annotation");
    if (!names.has(witnessFile)) {
      try {
        const functions = matches(
          `id: witness-function\nlanguage: Rust\nrule:\n  kind: function_item\n  has:\n    field: name\n    regex: '^internal_sender_witness_'\n  follows:\n    kind: attribute_item\n    regex: '^#\\[(tokio::)?test(\\(|\\])'\n`,
          [witnessFile],
          cwd,
        );
        names.set(
          witnessFile,
          new Set(
            functions.flatMap((fn) => {
              const name = fn.text.match(
                /\bfn (internal_sender_witness_\w+)/,
              )?.[1];
              return name ? [name] : [];
            }),
          ),
        );
      } catch {
        names.set(witnessFile, new Set());
      }
    }
    if (!names.get(witnessFile)?.has(witness))
      errors.push(
        `${location}: witness does not exist: ${witnessFile}#${witness}`,
      );
  }
  return errors;
}
