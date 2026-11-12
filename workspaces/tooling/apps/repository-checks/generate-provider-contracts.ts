// Generate Codex wire types from the pinned vendor schema. Unsupported schema constructs fail
// here instead of silently becoming an untyped or invented protocol variant.
import {
  readFileSync,
  writeFileSync,
  mkdtempSync,
  rmSync,
  existsSync,
} from "node:fs";
import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import { tmpdir } from "node:os";
import path from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import { codexContractSource as source } from "./provider-contract-sources.ts";

type ObjectValue = Record<string, unknown>;
type Reader = (file: string) => string;
interface Rendered {
  path: string;
  text: string;
}
const root = fileURLToPath(new URL("../../../..", import.meta.url));
const rustPath =
  "workspaces/runtime/packages/runtime-schema/src/codex_generated.rs";
const bridge = "workspaces/providers/apps/claude-bridge/codex-generated";
const provenance = `Owner: codex-cli ${source.version}; app-server generate-json-schema --experimental.`;
const claudeBridge = "workspaces/providers/apps/claude-bridge";
const claudeSchema =
  "workspaces/runtime/packages/runtime-schema/src/schemas/claude-rate-limit.schema.json";
const claudeProvenance =
  "workspaces/runtime/packages/runtime-schema/src/schemas/claude-rate-limit.provenance.json";
const claudeInputs = [
  `${claudeBridge}/rate-limit-input.ts`,
  `${claudeBridge}/usage-policy.ts`,
  `${claudeBridge}/package.json`,
  "pnpm-lock.yaml",
  "workspaces/tooling/apps/repository-checks/generate-provider-contracts.ts",
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
function strings(value: unknown): string[] {
  if (
    !Array.isArray(value) ||
    !value.every((item): item is string => typeof item === "string")
  )
    throw new Error("Expected vendor string list");
  return value;
}
function parse(text: string): ObjectValue {
  return object(JSON.parse(text));
}
const pascal = (name: string) => name[0]?.toUpperCase() + name.slice(1);
const snake = (name: string) =>
  name.replace(/[A-Z]/g, (letter) => "_" + letter.toLowerCase());
function identifier(name: string): string {
  if (!/^[a-zA-Z_][a-zA-Z0-9_]*$/.test(name))
    throw new Error(`Unsupported vendor identifier ${name}`);
  return name;
}
function rustType(value: unknown): string {
  const schema = object(value);
  if (
    typeof schema.$ref === "string" &&
    schema.$ref.startsWith("#/definitions/")
  )
    return "Codex" + identifier(schema.$ref.slice("#/definitions/".length));
  if (Array.isArray(schema.type)) {
    const types = strings(schema.type);
    const nonNull = types.filter((type) => type !== "null");
    if (types.length !== 2 || nonNull.length !== 1)
      throw new Error("Unsupported vendor type union");
    return `Option<${rustType({ ...schema, type: nonNull[0] })}>`;
  }
  switch (schema.type) {
    case "string":
      return "String";
    case "integer":
      if (schema.format !== "int64")
        throw new Error("Unsupported vendor integer format");
      return "i64";
    case "boolean":
      return "bool";
    case "array":
      return `Vec<${rustType(schema.items)}>`;
    default:
      throw new Error(`Unsupported vendor field ${JSON.stringify(schema)}`);
  }
}
function tsType(value: unknown): string {
  const schema = object(value);
  if (
    typeof schema.$ref === "string" &&
    schema.$ref.startsWith("#/definitions/")
  )
    return identifier(schema.$ref.slice("#/definitions/".length));
  if (Array.isArray(schema.type))
    return strings(schema.type)
      .map((type) => (type === "null" ? "null" : tsType({ ...schema, type })))
      .join(" | ");
  switch (schema.type) {
    case "string":
      return "string";
    case "integer":
      return "number";
    case "boolean":
      return "boolean";
    case "array":
      return `Array<${tsType(schema.items)}>`;
    default:
      throw new Error(`Unsupported vendor field ${JSON.stringify(schema)}`);
  }
}
function fields(schema: ObjectValue, excluded?: string): string {
  const required =
    schema.required === undefined ? [] : strings(schema.required);
  return Object.entries(object(schema.properties))
    .filter(([name]) => name !== excluded)
    .map(([name, value]) => {
      const property = object(value);
      let type = rustType(property);
      const optional = !required.includes(name);
      const hasDefault = Object.hasOwn(property, "default");
      if (hasDefault && property.default !== 0)
        throw new Error(`Unsupported vendor default ${name}`);
      if (
        (hasDefault && !optional) ||
        (optional && !hasDefault && !type.startsWith("Option<"))
      )
        throw new Error(`Unsupported vendor presence codec: ${name}`);
      if (!optional && type.startsWith("Option<"))
        throw new Error(
          `Required nullable vendor field needs an explicit presence codec: ${name}`,
        );
      return `${hasDefault ? "    #[serde(default)]\n" : ""}    #[serde(rename = ${JSON.stringify(name)})]\n    pub ${identifier(snake(name))}: ${type},`;
    })
    .join("\n");
}
function enumStrings(schema: ObjectValue): string[] | null {
  if (schema.enum !== undefined) return strings(schema.enum);
  if (
    Array.isArray(schema.oneOf) &&
    schema.oneOf.every((entry) => object(entry).type === "string")
  )
    return schema.oneOf.flatMap((entry) => strings(object(entry).enum));
  return null;
}
function renderType(
  name: string,
  schema: ObjectValue,
): { rust: string; ts: string } {
  const values = enumStrings(schema);
  const derive =
    "#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]";
  const rustName = "Codex" + name;
  if (values)
    return {
      rust: `#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]\n#[schemars(rename = ${JSON.stringify(name)})]\npub enum ${rustName} {\n${values.map((value) => `    #[serde(rename = ${JSON.stringify(value)})]\n    ${identifier(pascal(value))},`).join("\n")}\n}\n`,
      ts: `export type ${name} = ${values.map((value) => JSON.stringify(value)).join(" | ")};\n`,
    };
  if (Array.isArray(schema.oneOf)) {
    const taggedDerive = schema.oneOf.every((value) =>
      Object.keys(object(object(value).properties)).every(
        (name) => name === "type",
      ),
    )
      ? derive.replace("Clone, Debug", "Clone, Copy, Debug")
      : derive;
    const variants = schema.oneOf.map((value) => {
      const variant = object(value);
      const properties = object(variant.properties);
      const tag = strings(object(properties.type).enum);
      if (tag.length !== 1 || !tag[0])
        throw new Error("Unsupported vendor discriminant");
      const payload = fields(variant, "type").replaceAll("    pub ", "    ");
      return `    #[serde(rename = ${JSON.stringify(tag[0])})]\n    ${identifier(pascal(tag[0]))}${payload ? ` {\n${payload.replaceAll("\n", "\n    ")}\n    }` : ""},`;
    });
    const ts = schema.oneOf
      .map((value) => {
        const variant = object(value);
        const required = strings(variant.required);
        return `{ ${Object.entries(object(variant.properties))
          .map(
            ([key, property]) =>
              `${key}${required.includes(key) ? "" : "?"}: ${
                key === "type"
                  ? strings(object(property).enum)
                      .map((value) => JSON.stringify(value))
                      .join(" | ")
                  : tsType(property)
              }`,
          )
          .join("; ")} }`;
      })
      .join(" | ");
    return {
      rust: `${taggedDerive}\n#[serde(tag = "type")]\n#[schemars(rename = ${JSON.stringify(name)})]\npub enum ${rustName} {\n${variants.join("\n")}\n}\n`,
      ts: `export type ${name} = ${ts};\n`,
    };
  }
  if (schema.type !== "object")
    throw new Error(`Unsupported vendor shape ${name}`);
  return {
    rust: `${derive}\n#[schemars(rename = ${JSON.stringify(name)})]\npub struct ${rustName} {\n${fields(schema)}\n}\n`,
    ts: "",
  };
}
export function renderCodexContracts(
  directory = root,
  reader: Reader = read,
  rustOnly = false,
): Rendered[] {
  const lifecycleText = reader(
    path.join(directory, source.directory, "lifecycle.schema.json"),
  );
  const usageText = reader(
    path.join(
      directory,
      source.directory,
      "ThreadTokenUsageUpdatedNotification.json",
    ),
  );
  const errorText = reader(
    path.join(directory, source.directory, "CodexErrorInfo.json"),
  );
  for (const text of [lifecycleText, usageText, errorText]) {
    const comment = parse(text).$comment;
    if (typeof comment !== "string" || !comment.startsWith(provenance))
      throw new Error(
        "Codex vendor schema provenance differs from provider-contract-sources.ts; refresh the pinned source",
      );
  }
  const digest = createHash("sha256")
    .update(JSON.stringify(source))
    .update(lifecycleText)
    .update(usageText)
    .update(errorText)
    .digest("hex");
  const header = `// Generated from Codex CLI ${source.version} selected JSON Schema; do not edit.\n// Source: provider-contract-sources.ts; SHA-256: ${digest}\n`;
  const lifecycle = object(parse(lifecycleText).definitions);
  const usage = parse(usageText);
  const threadStatus = object(lifecycle.ThreadStatus);
  if (!Array.isArray(threadStatus.oneOf))
    throw new Error("Missing vendor thread status variants");
  // Recovery consumes only the discriminator. Derive that projection from the
  // same owner; it neither invents active flags nor validates the full status.
  const activity = {
    oneOf: threadStatus.oneOf.map((value) => ({
      type: "object",
      properties: { type: object(object(value).properties).type },
      required: ["type"],
    })),
  };
  const shapes = {
    ...lifecycle,
    ThreadActivity: activity,
    ...object(usage.definitions),
    ThreadTokenUsageUpdatedNotification: usage,
  };
  const output: Rendered[] = [];
  let rust = header + "\n";
  for (const [name, value] of Object.entries(shapes)) {
    const rendered = renderType(identifier(name), object(value));
    rust += rendered.rust + "\n";
    if (!rustOnly && Object.hasOwn(lifecycle, name)) {
      const imports =
        name === "ThreadStatus"
          ? 'import type { ThreadActiveFlag } from "./ThreadActiveFlag.ts";\n\n'
          : "";
      const target = `${bridge}/${name}.ts`;
      output.push({
        path: target,
        text: execFileSync(
          path.join(directory, "node_modules/.bin/oxfmt"),
          [
            `--stdin-filepath=${target}`,
            `--config=${path.join(directory, ".oxfmtrc.json")}`,
          ],
          { input: header + imports + rendered.ts, encoding: "utf8" },
        ),
      });
    }
  }
  const errors = parse(errorText);
  if (!Array.isArray(errors.anyOf))
    throw new Error("Missing vendor error variants");
  const kinds: string[] = [];
  for (const value of errors.anyOf) {
    const variant = object(value);
    if (variant.enum !== undefined) kinds.push(...strings(variant.enum));
    else if (variant.properties !== undefined)
      kinds.push(...Object.keys(object(variant.properties)));
    else if (
      JSON.stringify(variant.type) !== JSON.stringify(["string", "object"])
    )
      throw new Error("Unsupported vendor error extension");
  }
  rust += `#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]\npub enum KnownCodexErrorKind {\n${kinds.map((kind) => `    #[serde(rename = ${JSON.stringify(kind)})]\n    ${identifier(pascal(kind))},`).join("\n")}\n}\n`;
  output.unshift({
    path: rustPath,
    text: execFileSync("rustfmt", ["--edition", "2024", "--emit", "stdout"], {
      input: rust,
      encoding: "utf8",
    }),
  });
  output.push(...renderFullCodex(directory, reader, rustOnly));
  return output;
}
export function checkCodexContracts(
  directory = root,
  reader: Reader = read,
  rustOnly = false,
): void {
  for (const entry of renderCodexContracts(directory, read, rustOnly)) {
    if (reader(path.join(directory, entry.path)) !== entry.text)
      throw new Error(
        `Stale Codex contract: ${entry.path}; run pnpm run contracts:vendor:generate`,
      );
  }
}

// Full vendor protocol. The snapshot is byte-for-byte CLI output; flatten only
// the vendor's v2 reference namespace, whose two shared aliases are identical.
export function codexDefinitions(text: string): ObjectValue {
  const bundle = object(
    parse(text.replaceAll("#/definitions/v2/", "#/definitions/")).definitions,
  );
  const v2 = object(bundle.v2);
  for (const [name, value] of Object.entries(v2)) {
    if (
      Object.hasOwn(bundle, name) &&
      JSON.stringify(schemaMeaning(bundle[name])) !==
        JSON.stringify(schemaMeaning(value))
    )
      throw new Error(`Conflicting vendor namespaces for ${name}`);
  }
  const definitions = { ...bundle, ...v2 };
  delete definitions.v2;
  return parse(
    JSON.stringify(definitions).replaceAll(
      "#/definitions/v2/",
      "#/definitions/",
    ),
  );
}
const fullRustPath =
  "workspaces/runtime/packages/codex-protocol/src/generated/mod.rs";
const fullTsPath = `${bridge}/protocol.ts`;
const protocolKinds = [
  "ClientRequest",
  "ClientNotification",
  "ServerNotification",
  "ServerRequest",
] as const;
function typeName(name: string): string {
  const result = name.replace(
    /[^a-zA-Z0-9]+(.)?/g,
    (_match, next: string | undefined) => next?.toUpperCase() ?? "",
  );
  return identifier(/^\d/.test(result) ? `Value${result}` : pascal(result));
}
function fieldName(name: string): string {
  const result = name
    .replace(/([a-z0-9])([A-Z])/g, "$1_$2")
    .replace(/[^a-zA-Z0-9_]/g, "_")
    .toLowerCase();
  return ["self", "super", "crate", "Self"].includes(result)
    ? `${result}_`
    : `r#${identifier(result)}`;
}
function refName(schema: ObjectValue): string | undefined {
  if (typeof schema.$ref === "string") {
    const prefix = "#/definitions/";
    if (!schema.$ref.startsWith(prefix))
      throw new Error(`Unsupported vendor reference ${schema.$ref}`);
    return identifier(schema.$ref.slice(prefix.length));
  }
  if (Array.isArray(schema.allOf) && schema.allOf.length === 1)
    return refName(object(schema.allOf[0]));
  if (Array.isArray(schema.anyOf)) {
    const nonNull = schema.anyOf.filter(
      (entry) => object(entry).type !== "null",
    );
    if (nonNull.length === 1) return refName(object(nonNull[0]));
  }
  return undefined;
}
function protocolMethods(
  definitions: ObjectValue,
  kind: (typeof protocolKinds)[number],
): Array<{ method: string; schema: ObjectValue }> {
  const variants = object(definitions[kind]).oneOf;
  if (!Array.isArray(variants)) throw new Error(`Missing ${kind} variants`);
  return variants.map((value) => {
    const schema = object(value);
    const method = strings(object(object(schema.properties).method).enum);
    if (method.length !== 1 || !method[0])
      throw new Error(`Unsupported ${kind} method`);
    return { method: method[0], schema };
  });
}
function responseType(
  definitions: ObjectValue,
  method: string,
  _params: unknown,
): string {
  const overrides: Readonly<Record<string, string>> = source.responseTypes;
  const name = overrides[method];
  if (!name || !Object.hasOwn(definitions, name))
    throw new Error(`No vendor response type for ${method}: ${name}`);
  return identifier(name);
}
function codexProtocolFixtures(
  definitions: ObjectValue,
): Record<string, unknown[]> {
  function sample(value: unknown): unknown {
    if (value === true) return null;
    const schema = object(value);
    if (typeof schema.$ref === "string")
      return sample(definitions[refName(schema) ?? ""]);
    if (Array.isArray(schema.enum)) return schema.enum[0];
    if (Array.isArray(schema.type))
      return sample({
        ...schema,
        type: strings(schema.type).includes("null")
          ? "null"
          : strings(schema.type)[0],
      });
    const union = schema.oneOf ?? schema.anyOf ?? schema.allOf;
    if (Array.isArray(union)) {
      const selected = object(
        union.find((entry) => object(entry).type === "null") ?? union[0],
      );
      return sample({
        ...selected,
        ...(schema.properties
          ? {
              properties: {
                ...object(schema.properties),
                ...object(selected.properties ?? {}),
              },
              required: [
                ...strings(schema.required ?? []),
                ...strings(selected.required ?? []),
              ],
            }
          : {}),
      });
    }
    switch (schema.type) {
      case "null":
        return null;
      case "string":
        return "fixture";
      case "boolean":
        return false;
      case "integer":
      case "number":
        return typeof schema.minimum === "number" ? schema.minimum : 0;
      case "array":
        return Array.from(
          { length: typeof schema.minItems === "number" ? schema.minItems : 0 },
          () => sample(schema.items),
        );
      case "object": {
        const properties =
          schema.properties === undefined ? {} : object(schema.properties);
        return Object.fromEntries(
          strings(schema.required ?? []).map((name) => [
            name,
            sample(properties[name]),
          ]),
        );
      }
      case undefined:
        return null;
      default:
        throw new Error(`Unsupported vendor fixture ${JSON.stringify(schema)}`);
    }
  }
  return Object.fromEntries(
    protocolKinds.map((kind) => [
      kind,
      protocolMethods(definitions, kind).map(({ schema }) => sample(schema)),
    ]),
  );
}

function renderFullCodex(
  directory: string,
  reader: Reader,
  rustOnly: boolean,
): Rendered[] {
  const text = reader(path.join(directory, source.schemaFile));
  if (sha256(text) !== source.schemaSha256)
    throw new Error(
      "Codex full schema hash differs from provider-contract-sources.ts; refresh the pinned source",
    );
  const definitions = codexDefinitions(text);
  return renderFullCodexDefinitions(definitions, rustOnly);
}
export function renderFullCodexDefinitions(
  definitions: ObjectValue,
  rustOnly = false,
): Rendered[] {
  const header = `// Generated from codex-cli ${source.version} full experimental JSON Schema; do not edit.\n// Owner: provider-contract-sources.ts; SHA-256: ${source.schemaSha256}\n`;
  const pending = new Map(Object.entries(definitions));
  const rendered = new Map<string, string>();
  const derive =
    "#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]";
  const walkers = new Map<string, string>();
  const openSchemas = new WeakSet<object>();
  const visitor = (name: string, body: string) =>
    `impl VisitUnknown for ${name} { fn visit_unknowns(&self, _visitor: &mut dyn FnMut(&'static str, &str)) { ${body.replaceAll(/\bvisitor\b/g, "_visitor")} } }\n`;
  const fieldNames = (schema: ObjectValue, excluded?: string) => [
    ...Object.keys(object(schema.properties ?? {}))
      .filter((field) => field !== excluded)
      .map(fieldName),
    ...(schema.additionalProperties !== undefined &&
    schema.additionalProperties !== false
      ? ["additional_properties"]
      : []),
  ];
  // Only edges in a recursive type require allocation. Vec/map already break
  // layout cycles, but including them here merely boxes the recursive edge.
  const edges = new Map<string, Set<string>>();
  function references(value: unknown, result = new Set<string>()): Set<string> {
    if (value && typeof value === "object") {
      if (!Array.isArray(value)) {
        const schema = object(value);
        if (typeof schema.$ref === "string") result.add(refName(schema) ?? "");
      }
      for (const nested of Object.values(value)) references(nested, result);
    }
    return result;
  }
  for (const [name, schema] of pending) edges.set(name, references(schema));
  function recursive(
    from: string,
    target: string,
    visited = new Set<string>(),
  ): boolean {
    if (target === from) return true;
    if (visited.has(target)) return false;
    visited.add(target);
    return [...(edges.get(target) ?? [])].some((next) =>
      recursive(from, next, visited),
    );
  }
  function enqueue(name: string, schema: unknown): string {
    const existing = pending.get(name);
    if (
      existing !== undefined &&
      JSON.stringify(existing) !== JSON.stringify(schema)
    )
      throw new Error(`Generated vendor type collision ${name}`);
    pending.set(name, schema);
    return name;
  }
  function rust(value: unknown, context: string, owner: string): string {
    if (value === true) return "studio_json::Value";
    if (value === false)
      throw new Error(`Unsupported impossible vendor field ${context}`);
    const schema = object(value);
    const ref = refName(schema);
    if (schema.$ref !== undefined) {
      if (!ref || !Object.hasOwn(definitions, ref))
        throw new Error(`Missing vendor type ${ref}`);
      return recursive(owner, ref) ? `Box<${ref}>` : ref;
    }
    if (Array.isArray(schema.allOf)) {
      if (schema.allOf.length !== 1)
        throw new Error(`Unsupported vendor intersection ${context}`);
      return rust(schema.allOf[0], context, owner);
    }
    if (Array.isArray(schema.type)) {
      const types = strings(schema.type);
      if (types.includes("null") && types.length === 2)
        return `Option<${rust({ ...schema, type: types.find((entry) => entry !== "null") }, context, owner)}>`;
      return rust(
        { anyOf: types.map((type) => ({ ...schema, type })) },
        context,
        owner,
      );
    }
    if (Array.isArray(schema.anyOf)) {
      const nonNull = schema.anyOf.filter(
        (entry) => object(entry).type !== "null",
      );
      if (nonNull.length === 1 && nonNull.length !== schema.anyOf.length)
        return `Option<${rust(nonNull[0], context, owner)}>`;
    }
    if (
      schema.enum !== undefined ||
      schema.oneOf !== undefined ||
      schema.anyOf !== undefined ||
      (schema.type === "object" &&
        (schema.properties !== undefined ||
          schema.additionalProperties === false))
    )
      return enqueue(context, schema);
    switch (schema.type) {
      case "string":
        return "String";
      case "boolean":
        return "bool";
      case "integer": {
        const formats: Readonly<Record<string, string>> = {
          int64: "i64",
          int32: "i32",
          uint64: "u64",
          uint32: "u32",
          uint16: "u16",
          uint: "u64",
        };
        if (typeof schema.format !== "string" || !formats[schema.format])
          throw new Error(`Unsupported vendor integer ${context}`);
        return formats[schema.format] ?? "";
      }
      case "number":
        return "f64";
      case "null":
        return "()";
      case "array":
        return `Vec<${rust(schema.items, `${context}Item`, owner)}>`;
      case "object":
        return `std::collections::BTreeMap<String, ${rust(schema.additionalProperties ?? true, `${context}Value`, owner)}>`;
      case undefined:
        if (
          Object.keys(schema).every((key) =>
            ["description", "default", "title", "$schema"].includes(key),
          )
        )
          return "studio_json::Value";
        throw new Error(
          `Unsupported vendor field ${context}: ${JSON.stringify(schema)}`,
        );
      default:
        throw new Error(
          `Unsupported vendor field ${context}: ${JSON.stringify(schema)}`,
        );
    }
  }
  function fieldsFull(
    schema: ObjectValue,
    context: string,
    excluded?: string,
  ): string {
    const required =
      schema.required === undefined ? [] : strings(schema.required);
    const properties =
      schema.properties === undefined ? {} : object(schema.properties);
    let result = Object.entries(properties)
      .filter(([name]) => name !== excluded)
      .map(([name, property]) => {
        const type = rust(property, `${context}${typeName(name)}`, context);
        const present = required.includes(name);
        return `    #[serde(rename = ${JSON.stringify(name)}${present ? ', deserialize_with = "required"' : ', default, skip_serializing_if = "Option::is_none", deserialize_with = "optional"'})]\n    pub ${fieldName(name)}: ${present ? type : `Option<${type}>`},`;
      })
      .join("\n");
    if (
      schema.additionalProperties !== undefined &&
      schema.additionalProperties !== false
    )
      result += `\n    #[serde(flatten)]\n    pub additional_properties: std::collections::BTreeMap<String, ${rust(schema.additionalProperties, `${context}AdditionalProperty`, context)}>,`;
    return result;
  }
  function named(name: string, value: unknown): string {
    const schema = object(value);
    const enumValues = schema.enum !== undefined ? strings(schema.enum) : null;
    if (enumValues) {
      const variants = enumValues.map(typeName);
      if (new Set(variants).size !== variants.length)
        throw new Error(`Vendor enum collision ${name}`);
      walkers.set(name, visitor(name, ""));
      return `${derive}\npub enum ${name} {\n${enumValues.map((wire, index) => `    #[serde(rename = ${JSON.stringify(wire)})]\n    ${variants[index]},`).join("\n")}\n}\n`;
    }
    const union = schema.oneOf ?? schema.anyOf;
    if (Array.isArray(union)) {
      const values = union.map((entry) => {
        const variant = object(entry);
        if (!schema.properties) return variant;
        return {
          ...variant,
          properties: {
            ...object(schema.properties),
            ...object(variant.properties ?? {}),
          },
          required: [
            ...strings(schema.required ?? []),
            ...strings(variant.required ?? []),
          ],
        };
      });
      const common = values[0]?.properties
        ? Object.keys(object(values[0].properties)).find((key) =>
            values.every((entry) => {
              if (
                !entry.properties ||
                !strings(entry.required ?? []).includes(key)
              )
                return false;
              const candidate = object(entry.properties)[key];
              if (
                !candidate ||
                typeof candidate !== "object" ||
                Array.isArray(candidate)
              )
                return false;
              const property = object(candidate);
              return (
                Array.isArray(property.enum) &&
                property.enum.length === 1 &&
                typeof property.enum[0] === "string"
              );
            }),
          )
        : undefined;
      if (common) {
        const seenVariants = new Set<string>();
        const arms: string[] = [];
        const tags: string[] = [];
        const variants = values.map((entry, index) => {
          const wire = strings(
            object(object(entry.properties)[common]).enum,
          )[0];
          if (!wire) throw new Error(`Missing vendor tag ${name}`);
          tags.push(wire);
          const fields = fieldsFull(
            entry,
            `${name}${typeName(wire)}`,
            common,
          ).replaceAll("    pub ", "    ");
          let variantName = typeName(wire);
          if (seenVariants.has(variantName))
            variantName += `Variant${index + 1}`;
          seenVariants.add(variantName);
          const names = fieldNames(entry, common);
          arms.push(
            `Self::${variantName}${names.length ? ` { ${names.join(", ")} }` : ""} => { ${names.map((field) => `VisitUnknown::visit_unknowns(${field}, visitor);`).join(" ")} }`,
          );
          return `    #[serde(rename = ${JSON.stringify(wire)})]\n    ${variantName}${fields ? ` {\n${fields}\n    }` : ""},`;
        });
        const open = !protocolKinds.includes(
          name as (typeof protocolKinds)[number],
        );
        if (open) openSchemas.add(schema);
        // 2026-11-11 release benchmark: Codex 5.0 -> 6.8 us/frame (+36%),
        // allocations 75.5 -> 51.4/frame. Untagged fallback buffers known fields.
        // Keep this simpler form; if profiling warrants it, generate raw-token
        // tag-first decoding rather than adding handwritten runtime visitors.
        if (open)
          arms.push(
            `Self::UnknownVendorVariant(value) => visitor(${JSON.stringify(name)}, value.variant())`,
          );
        walkers.set(name, visitor(name, `match self { ${arms.join(",\n")} }`));
        const unknown = open
          ? `\n#[derive(Clone, Debug, PartialEq, serde::Serialize)]\n#[serde(transparent)]\npub struct Unknown${name}(std::collections::BTreeMap<String, studio_json::Value>);\nimpl Unknown${name} { pub fn variant(&self) -> &str { self.0[${JSON.stringify(common)}].as_str().expect("validated unknown discriminator") } }\nimpl<'de> serde::Deserialize<'de> for Unknown${name} {\n    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {\n        let fields = <std::collections::BTreeMap<String, studio_json::Value> as serde::Deserialize>::deserialize(deserializer)?;\n        let variant = fields.get(${JSON.stringify(common)}).and_then(studio_json::Value::as_str).ok_or_else(|| serde::de::Error::custom("missing vendor discriminator"))?;\n        if matches!(variant, ${tags.map((tag) => JSON.stringify(tag)).join(" | ")}) { return Err(serde::de::Error::custom("malformed known vendor variant")); }\n        Ok(Self(fields))\n    }\n}\n`
          : "";
        return `${derive}\n#[serde(tag = ${JSON.stringify(common)})]\npub enum ${name} {\n${variants.join("\n")}\n${open ? `    #[serde(untagged)]\n    UnknownVendorVariant(Unknown${name}),` : ""}\n}\n${unknown}`;
      }
      walkers.set(
        name,
        visitor(
          name,
          `match self { ${values.map((_entry, index) => `Self::Variant${index + 1}(value) => VisitUnknown::visit_unknowns(value, visitor)`).join(", ")} }`,
        ),
      );
      return `${derive}\n#[serde(untagged)]\npub enum ${name} {\n${values.map((entry, index) => `    Variant${index + 1}(Box<${rust(entry, `${name}Variant${index + 1}`, name)}>),`).join("\n")}\n}\n`;
    }
    if (schema.type === "object") {
      const shape =
        schema.properties === undefined && schema.additionalProperties !== false
          ? {
              ...schema,
              additionalProperties: schema.additionalProperties ?? true,
            }
          : schema;
      walkers.set(
        name,
        visitor(
          name,
          fieldNames(shape)
            .map(
              (field) =>
                `VisitUnknown::visit_unknowns(&self.${field}, visitor);`,
            )
            .join(" "),
        ),
      );
      const preparationParams = [
        "ThreadStartParams",
        "ThreadResumeParams",
        "TurnStartParams",
        "ThreadForkParams",
      ].includes(name);
      const requiredFields = strings(shape.required ?? []);
      const constructor = preparationParams
        ? `    pub fn new(${requiredFields.map((field) => `${snake(field)}: ${rust(object(shape.properties)[field], `${name}${typeName(field)}`, name)}`).join(", ")}) -> Self { Self { ${Object.keys(
            object(shape.properties),
          )
            .map(
              (field) =>
                `${snake(field)}: ${requiredFields.includes(field) ? snake(field) : "None"}`,
            )
            .join(", ")} } }\n`
        : "";
      const fields = preparationParams
        ? `impl ${name} {\n${constructor}}\n`
        : "";
      return `${derive}\n${schema.additionalProperties === false ? "#[serde(deny_unknown_fields)]\n" : ""}pub struct ${name} {\n${fieldsFull(shape, name)}\n}\n${fields}`;
    }
    return `pub type ${name} = ${rust(schema, `${name}Value`, name)};\n`;
  }
  for (const [name, schema] of pending) {
    identifier(name);
    rendered.set(name, named(name, schema));
  }
  function containsUnknown(name: string, visited = new Set<string>()): boolean {
    if (visited.has(name)) return false;
    visited.add(name);
    const schema = pending.get(name);
    const inline = (value: unknown): boolean =>
      value !== null &&
      typeof value === "object" &&
      (openSchemas.has(value) || Object.values(value).some(inline));
    return (
      inline(schema) ||
      [...references(schema)].some((target) => containsUnknown(target, visited))
    );
  }
  let rustText =
    header +
    `\n// Generated vendor layouts preserve optional/nullable fields and large unions.\n#![allow(clippy::all, clippy::option_option, clippy::struct_excessive_bools)]\npub const CLI_VERSION: &str = ${JSON.stringify(source.version)};\npub const SCHEMA_SHA256: &str = ${JSON.stringify(source.schemaSha256)};\n\n// A required nullable field must still reject an absent member.\nfn required<'de, D: serde::Deserializer<'de>, T: serde::Deserialize<'de>>(d: D) -> Result<T, D::Error> { T::deserialize(d) }\nfn optional<'de, D: serde::Deserializer<'de>, T: serde::Deserialize<'de>>(d: D) -> Result<Option<T>, D::Error> { T::deserialize(d).map(Some) }\n\n` +
    [...rendered.values()].join("\n");
  rustText += `\npub trait VisitUnknown { fn visit_unknowns(&self, _visitor: &mut dyn FnMut(&'static str, &str)) {} }\nmacro_rules! leaves { ($($ty:ty),*) => { $(impl VisitUnknown for $ty {})* }; }\nleaves!(String, bool, i64, i32, u64, u32, u16, f64, (), studio_json::Value);\nimpl<T: VisitUnknown> VisitUnknown for Option<T> { fn visit_unknowns(&self, visitor: &mut dyn FnMut(&'static str, &str)) { if let Some(value) = self { value.visit_unknowns(visitor); } } }\nimpl<T: VisitUnknown> VisitUnknown for Vec<T> { fn visit_unknowns(&self, visitor: &mut dyn FnMut(&'static str, &str)) { for value in self { value.visit_unknowns(visitor); } } }\nimpl<T: VisitUnknown> VisitUnknown for Box<T> { fn visit_unknowns(&self, visitor: &mut dyn FnMut(&'static str, &str)) { (**self).visit_unknowns(visitor); } }\nimpl<T: VisitUnknown> VisitUnknown for std::collections::BTreeMap<String, T> { fn visit_unknowns(&self, visitor: &mut dyn FnMut(&'static str, &str)) { for value in self.values() { value.visit_unknowns(visitor); } } }\n${[...walkers].map(([name, code]) => (containsUnknown(name) ? code : `impl VisitUnknown for ${name} {}`)).join("\n")}\n`;
  let tsText = header + "\n";
  function ts(value: unknown): string {
    if (value === true) return "unknown";
    if (value === false) return "never";
    const schema = object(value);
    if (typeof schema.$ref === "string") return refName(schema) ?? "never";
    if (Array.isArray(schema.enum))
      return strings(schema.enum)
        .map((entry) => JSON.stringify(entry))
        .join(" | ");
    for (const key of ["oneOf", "anyOf", "allOf"])
      if (Array.isArray(schema[key])) {
        const union = schema[key]
          .map((entry) => `(${ts(entry)})`)
          .join(key === "allOf" ? " & " : " | ");
        if (!schema.properties) return union;
        const siblings = { ...schema };
        delete siblings.oneOf;
        delete siblings.anyOf;
        delete siblings.allOf;
        return `(${union}) & ${ts(siblings)}`;
      }
    if (Array.isArray(schema.type))
      return strings(schema.type)
        .map((type) => ts({ ...schema, type }))
        .join(" | ");
    switch (schema.type) {
      case "string":
        return "string";
      case "boolean":
        return "boolean";
      case "integer":
      case "number":
        return "number";
      case "null":
        return "null";
      case "array":
        return `Array<${ts(schema.items)}>`;
      case "object": {
        const required = strings(schema.required ?? []);
        const properties =
          schema.properties === undefined ? {} : object(schema.properties);
        const fields = Object.entries(properties)
          .map(
            ([name, property]) =>
              `${JSON.stringify(name)}${required.includes(name) ? "" : "?"}: ${ts(property)}`,
          )
          .join("; ");
        const additional = schema.additionalProperties;
        if (!fields)
          return `Record<string, ${additional === false ? "never" : ts(additional ?? true)}>`;
        return `{ ${fields} }${additional !== undefined && additional !== false ? ` & Record<string, ${ts(additional)}>` : ""}`;
      }
      case undefined:
        return "unknown";
      default:
        throw new Error(
          `Unsupported TypeScript vendor shape ${JSON.stringify(schema)}`,
        );
    }
  }
  for (const [name, schema] of Object.entries(definitions))
    tsText += `export type ${name} = ${ts(schema)};\n\n`;
  for (const kind of protocolKinds) {
    const methods = protocolMethods(definitions, kind);
    const name = `${kind}Method`;
    const variants = methods.map(({ method }) => typeName(method));
    if (new Set(variants).size !== methods.length)
      throw new Error(`Vendor method collision ${kind}`);
    rustText += `#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, serde::Serialize, serde::Deserialize)]\npub enum ${name} {\n${methods.map(({ method }, index) => `    #[serde(rename = ${JSON.stringify(method)})]\n    ${variants[index]},`).join("\n")}\n}\nimpl ${name} {\n    pub fn from_wire(value: &str) -> Option<Self> { match value {\n${methods.map(({ method }, index) => `        ${JSON.stringify(method)} => Some(Self::${variants[index]}),`).join("\n")}\n        _ => None,\n    }}\n    pub const fn as_str(self) -> &'static str { match self {\n${methods.map(({ method }, index) => `        Self::${variants[index]} => ${JSON.stringify(method)},`).join("\n")}\n    }}\n    pub const fn all() -> &'static [Self] { &[${variants.map((variant) => `Self::${variant}`).join(", ")}] }\n}\n`;
    tsText += `export type ${name} = ${methods.map(({ method }) => JSON.stringify(method)).join(" | ")};\n`;
    const request = kind === "ClientRequest" || kind === "ServerRequest";
    rustText += `impl ${kind} {\n    pub const fn method(&self) -> ${name} { match self {\n${methods.map(({ method, schema }) => `        Self::${typeName(method)}${Object.keys(object(schema.properties)).length > 1 ? " { .. }" : ""} => ${name}::${typeName(method)},`).join("\n")}\n    }}\n`;
    if (kind !== "ClientNotification") {
      rustText += `    pub fn from_params(${request ? "id: RequestId, " : ""}method: ${name}, params: Option<&str>${kind === "ServerNotification" ? ", emitted_at_ms: Option<i64>" : ""}) -> Result<Self, studio_json::Error> {\n        Ok(match method {\n${methods
        .map(({ method, schema }) => {
          const required = strings(schema.required ?? []).includes("params");
          const param = required
            ? `studio_json::from_str(params.ok_or_else(|| <studio_json::Error as serde::de::Error>::custom("missing vendor params"))?)?`
            : "params.map(studio_json::from_str).transpose()?";
          return `            ${name}::${typeName(method)} => Self::${typeName(method)} { ${request ? "id, " : ""}params: ${param}${kind === "ServerNotification" ? ", emitted_at_ms" : ""} },`;
        })
        .join("\n")}\n        })\n    }\n`;
    }
    if (request) {
      rustText += `    pub fn from_deserializer<'de, D: serde::Deserializer<'de>>(id: RequestId, method: ${name}, params: D) -> Result<Self, D::Error> { Ok(match method {\n${variants.map((variant) => `        ${name}::${variant} => Self::${variant} { id, params: serde::Deserialize::deserialize(params)? },`).join("\n")}\n    }) }\n`;
      rustText += `    pub const fn id(&self) -> &RequestId { match self {\n${variants.map((variant) => `        Self::${variant} { r#id, .. } => r#id,`).join("\n")}\n    }}\n    pub fn set_id(&mut self, new_id: RequestId) { match self {\n${variants.map((variant) => `        Self::${variant} { r#id, .. } => *r#id = new_id,`).join("\n")}\n    }}\n    pub fn from_method_params(id: RequestId, method: ${name}, params: &impl serde::Serialize) -> Result<Self, studio_json::Error> {\n        #[derive(serde::Serialize)]\n        struct Wire<'a, T> { id: RequestId, method: ${name}, params: &'a T }\n        studio_json::from_slice(&studio_json::to_vec(&Wire { id, method, params })?)\n    }\n`;
    }
    rustText += `}\n`;
    if (kind === "ClientRequest" || kind === "ServerRequest") {
      const results = methods.map(({ method, schema }) => ({
        method,
        result: responseType(
          definitions,
          method,
          object(schema.properties).params,
        ),
      }));
      rustText += `${derive}\n#[serde(tag = "method", content = "result")]\npub enum ${kind}Result {\n${results.map(({ method, result }) => `    #[serde(rename = ${JSON.stringify(method)})]\n    ${typeName(method)}(Box<${result}>),`).join("\n")}\n}\n`;
      rustText += visitor(
        `${kind}Result`,
        `match self { ${variants.map((variant) => `Self::${variant}(value) => VisitUnknown::visit_unknowns(value, visitor)`).join(", ")} }`,
      );
      rustText += `impl ${kind}Result {\n    pub const fn method(&self) -> ${name} { match self {\n${variants.map((variant) => `        Self::${variant}(_) => ${name}::${variant},`).join("\n")}\n    }}\n    pub fn decode(method: ${name}, result: &str) -> Result<Self, studio_json::Error> { match method {\n${variants.map((variant) => `        ${name}::${variant} => studio_json::from_str(result).map(|result| Self::${variant}(Box::new(result))),`).join("\n")}\n    }}\n    pub fn serialize_payload<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> { match self {\n${variants.map((variant) => `        Self::${variant}(result) => serde::Serialize::serialize(result, serializer),`).join("\n")}\n    }}\n}\n`;
      if (kind === "ServerRequest")
        rustText += `impl ${kind}Result { pub fn from_deserializer<'de, D: serde::Deserializer<'de>>(method: ${name}, result: D) -> Result<Self, D::Error> { match method {\n${variants.map((variant) => `        ${name}::${variant} => serde::Deserialize::deserialize(result).map(|result| Self::${variant}(Box::new(result))),`).join("\n")}\n    }} }\n`;
      tsText += `export type ${kind}Results = { ${results.map(({ method, result }) => `${JSON.stringify(method)}: ${result}`).join("; ")} };\n`;
    }
  }
  const allMethods = [
    ...new Set(
      protocolKinds.flatMap((kind) =>
        protocolMethods(definitions, kind).map(({ method }) => method),
      ),
    ),
  ];
  rustText += `\n/// Expand the vendor method inventory into a consumer's method definition.\n#[macro_export]\nmacro_rules! codex_methods {\n    ($consumer:ident) => { $consumer! {\n${allMethods.map((method) => `        ${method === "error" ? "ProtocolError" : method === "project/import" ? "CodexProjectImport" : typeName(method)} => ${JSON.stringify(method)},`).join("\n")}\n    }};\n}\n`;
  rustText += `\n#[cfg(test)]\nmod generated_fixtures {\n    use super::*;\n    #[derive(serde::Deserialize)]\n    #[serde(rename_all = "PascalCase")]\n    struct Samples {\n        client_request: Vec<ClientRequest>,\n        client_notification: Vec<ClientNotification>,\n        server_notification: Vec<ServerNotification>,\n        server_request: Vec<ServerRequest>,\n    }\n    #[test]\n    fn every_vendor_method_sample_decodes() {\n        let samples: Samples = studio_json::from_str(include_str!("fixtures.json")).unwrap();\n        assert_eq!(samples.client_request.len(), ClientRequestMethod::all().len());\n        assert_eq!(samples.client_notification.len(), ClientNotificationMethod::all().len());\n        assert_eq!(samples.server_notification.len(), ServerNotificationMethod::all().len());\n        assert_eq!(samples.server_request.len(), ServerRequestMethod::all().len());\n    }\n}\n`;
  const output: Rendered[] = [
    {
      path: "workspaces/runtime/packages/codex-protocol/src/generated/fixtures.json",
      text: JSON.stringify(codexProtocolFixtures(definitions), null, 2) + "\n",
    },
    {
      path: fullRustPath,
      text: execFileSync("rustfmt", ["--edition", "2024", "--emit", "stdout"], {
        input: rustText,
        encoding: "utf8",
        maxBuffer: 16 * 1024 * 1024,
      }),
    },
  ];
  if (!rustOnly)
    output.push({
      path: fullTsPath,
      text: execFileSync(
        path.join(root, "node_modules/.bin/oxfmt"),
        [
          `--stdin-filepath=${fullTsPath}`,
          `--config=${path.join(root, ".oxfmtrc.json")}`,
        ],
        { input: tsText, encoding: "utf8", maxBuffer: 16 * 1024 * 1024 },
      ),
    });
  return output;
}

/** Vendor-owned request/result associations absent from generate-json-schema. */
export function extractCodexResults(
  text: string,
  definitions: ObjectValue,
): Record<string, string> {
  const links = new Map<string, string>();
  for (const match of text.matchAll(
    /^    (\w+)(?: => "([^"]+)")? \{([\s\S]*?)\n    \}/gm,
  )) {
    const variant = match[1];
    const body = match[3];
    if (!variant || body === undefined)
      throw new Error("Invalid vendor request definition");
    const response = /\bresponse:\s*(?:v[12]::)?(\w+),/.exec(body)?.[1];
    if (response)
      links.set(
        match[2] ?? variant[0]?.toLowerCase() + variant.slice(1),
        response,
      );
  }
  return Object.fromEntries(
    ["ClientRequest", "ServerRequest"].flatMap((kind) => {
      if (kind !== "ClientRequest" && kind !== "ServerRequest")
        throw new Error("Invalid request kind");
      return protocolMethods(definitions, kind).map(({ method }) => {
        const response = links.get(method);
        if (!response || !Object.hasOwn(definitions, response))
          throw new Error(`Vendor request/result link missing for ${method}`);
        return [method, response];
      });
    }),
  );
}
function schemaMeaning(value: unknown, record = false): unknown {
  if (Array.isArray(value)) return value.map((entry) => schemaMeaning(entry));
  if (!value || typeof value !== "object") return value;
  const entries = Object.entries(value).sort(([left], [right]) =>
    left.localeCompare(right),
  );
  return Object.fromEntries(
    entries
      .filter(
        ([key]) =>
          record ||
          ![
            "$schema",
            "$comment",
            "description",
            "title",
            "examples",
            "deprecated",
          ].includes(key),
      )
      .map(([key, entry]) => [
        key,
        schemaMeaning(
          entry,
          !record &&
            ["properties", "definitions", "patternProperties"].includes(key),
        ),
      ]),
  );
}
export function codexDrift(
  pinned: ObjectValue,
  installed: ObjectValue,
  pinnedResults: Readonly<Record<string, string>>,
  installedResults: Readonly<Record<string, string>>,
): { lines: string[]; changed: boolean } {
  const lines: string[] = [];
  let changed = false;
  function compare(label: string, before: ObjectValue, after: ObjectValue) {
    const added = Object.keys(after)
      .filter((key) => !Object.hasOwn(before, key))
      .sort();
    const removed = Object.keys(before)
      .filter((key) => !Object.hasOwn(after, key))
      .sort();
    const modified = Object.keys(before)
      .filter(
        (key) =>
          Object.hasOwn(after, key) &&
          JSON.stringify(schemaMeaning(before[key])) !==
            JSON.stringify(schemaMeaning(after[key])),
      )
      .sort();
    for (const [kind, values] of [
      ["added", added],
      ["removed", removed],
      ["changed", modified],
    ] as const) {
      lines.push(
        `${label} ${kind}: ${values.length ? values.join(", ") : "none"}`,
      );
      changed ||= values.length > 0;
    }
  }
  for (const kind of protocolKinds)
    compare(
      kind,
      Object.fromEntries(
        protocolMethods(pinned, kind).map(({ method, schema }) => [
          method,
          schema,
        ]),
      ),
      Object.fromEntries(
        protocolMethods(installed, kind).map(({ method, schema }) => [
          method,
          schema,
        ]),
      ),
    );
  compare("types", pinned, installed);
  compare("request results", pinnedResults, installedResults);
  return { lines, changed };
}
export async function checkInstalledCodexDrift(
  directory = root,
  executable = process.env.CODEX_BIN || "codex",
  candidateVersion?: string,
  reviewedSha?: string,
): Promise<boolean> {
  const actual = execFileSync(executable, ["--version"], {
    encoding: "utf8",
  }).trim();
  const version = /^codex-cli (\d+\.\d+\.\d+(?:[-+][a-zA-Z0-9.-]+)?)$/.exec(
    actual,
  )?.[1];
  if (!version) throw new Error(`Invalid Codex CLI version: ${actual}`);
  if (candidateVersion && version !== candidateVersion)
    throw new Error(
      `Expected codex-cli ${candidateVersion}; received ${actual}`,
    );
  const temporary = mkdtempSync(path.join(tmpdir(), "studio-provider-drift-"));
  try {
    execFileSync(executable, [
      "app-server",
      "generate-json-schema",
      "--experimental",
      "--out",
      temporary,
    ]);
    const schema = read(
      path.join(temporary, "codex_app_server_protocol.schemas.json"),
    );
    const installed = codexDefinitions(schema);
    const responseUrl = source.responseSource.url.replace(
      `rust-v${source.version}/`,
      `rust-v${version}/`,
    );
    const response = await fetch(responseUrl);
    if (!response.ok)
      throw new Error(
        `Cannot inspect vendor request/result links: ${response.status} ${responseUrl}`,
      );
    const vendor = await response.text();
    if (
      version === source.version &&
      sha256(vendor) !== source.responseSource.sha256
    )
      throw new Error("Pinned vendor request/result source hash changed");
    const results = extractCodexResults(vendor, installed);
    const report = codexDrift(
      codexDefinitions(read(path.join(directory, source.schemaFile))),
      installed,
      source.responseTypes,
      results,
    );
    process.stdout.write(
      `Codex pinned ${source.version}; installed ${version}\n${report.lines.join("\n")}\n`,
    );
    if (report.changed)
      process.stdout.write(
        "Contract drift requires explicit dispositions and review; removals or changes are potentially breaking.\n",
      );
    if (candidateVersion) {
      const schemaSha = sha256(schema);
      process.stdout.write(
        `Candidate schema SHA-256: ${schemaSha}\nResponse provenance: ${responseUrl}\nResponse SHA-256: ${sha256(vendor)}\nReview, then rerun with --reviewed-sha ${schemaSha}\n`,
      );
      if (reviewedSha) {
        if (reviewedSha !== schemaSha)
          throw new Error("Reviewed SHA differs from candidate schema");
        const owner = path.join(
          directory,
          "workspaces/tooling/apps/repository-checks/provider-contract-sources.ts",
        );
        writeGenerated(
          owner,
          read(owner)
            .replace(JSON.stringify(source.version), JSON.stringify(version))
            .replace(source.schemaSha256, schemaSha)
            .replace(source.responseSource.url, responseUrl)
            .replace(source.responseSource.sha256, sha256(vendor))
            .replace(
              /responseTypes: \{[\s\S]*?\n  \},/,
              `responseTypes: ${JSON.stringify(results, null, 2)},`,
            ),
        );
      }
    }
    return !report.changed;
  } finally {
    rmSync(temporary, { recursive: true, force: true });
  }
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
            "Owner: generate-provider-contracts.ts. Declaration digest is checked by full generation; committed inputs and artifact are checked offline by Cargo.",
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
  const owner: typeof import("../../../providers/apps/claude-bridge/rate-limit-input.ts") =
    await import(
      pathToFileURL(
        path.join(
          directory,
          "workspaces/providers/apps/claude-bridge/rate-limit-input.ts",
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
export function refreshCodexSchemas(
  directory = root,
  executable = process.env.CODEX_BIN || "codex",
  check = false,
): void {
  const actual = execFileSync(executable, ["--version"], {
    encoding: "utf8",
  }).trim();
  if (actual !== `codex-cli ${source.version}`)
    throw new Error(`Expected codex-cli ${source.version}; received ${actual}`);
  const temporary = mkdtempSync(
    path.join(tmpdir(), "studio-vendor-contracts-"),
  );
  try {
    execFileSync(executable, [
      "app-server",
      "generate-json-schema",
      "--experimental",
      "--out",
      temporary,
    ]);
    const fullSchema = read(
      path.join(temporary, "codex_app_server_protocol.schemas.json"),
    );
    if (sha256(fullSchema) !== source.schemaSha256)
      throw new Error(
        "Installed Codex schema differs from pinned full schema; run just provider-drift and explicitly update provider-contract-sources.ts",
      );
    if (check) {
      if (read(path.join(directory, source.schemaFile)) !== fullSchema)
        throw new Error("Stale full Codex schema");
    } else writeGenerated(path.join(directory, source.schemaFile), fullSchema);
    const generated = (file: string) => parse(read(path.join(temporary, file)));
    const lifecycle = Object.fromEntries(
      Object.entries(source.lifecycle).map(([name, file]) => [
        name,
        object(generated(file).definitions)[name],
      ]),
    );
    const errors = generated(source.errors);
    const errorLeaf = object(object(errors.definitions).CodexErrorInfo);
    const outputs = [
      {
        path: "lifecycle.schema.json",
        value: {
          $comment: `${provenance} Selected definitions, unchanged.`,
          definitions: lifecycle,
        },
      },
      {
        path: "ThreadTokenUsageUpdatedNotification.json",
        value: { ...generated(source.usage), $comment: provenance },
      },
      {
        path: "CodexErrorInfo.json",
        value: {
          $schema: errors.$schema,
          $comment: `${provenance} definitions/CodexErrorInfo from ${source.errors}. Refresh with pnpm run contracts:vendor:refresh.`,
          title: "CodexErrorInfo",
          ...errorLeaf,
          definitions: {
            NonSteerableTurnKind: object(errors.definitions)
              .NonSteerableTurnKind,
          },
        },
      },
    ];
    for (const output of outputs) {
      const target = path.join(directory, source.directory, output.path);
      const text = execFileSync(
        path.join(directory, "node_modules/.bin/oxfmt"),
        [
          `--stdin-filepath=${target}`,
          `--config=${path.join(directory, ".oxfmtrc.json")}`,
        ],
        {
          input: JSON.stringify(output.value, null, 2) + "\n",
          encoding: "utf8",
        },
      );
      if (check) {
        if (read(target) !== text)
          throw new Error(`Stale vendor source ${output.path}`);
      } else writeGenerated(target, text);
    }
  } finally {
    rmSync(temporary, { recursive: true, force: true });
  }
}
if (
  process.argv[1] &&
  path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)
) {
  if (process.argv.includes("--drift")) {
    process.exitCode = (await checkInstalledCodexDrift()) ? 0 : 1;
  } else {
    if (process.argv.includes("--cargo-inputs")) {
      for (const file of [
        ...claudeInputs,
        claudeSchema,
        claudeProvenance,
        source.schemaFile,
        fullRustPath,
      ])
        process.stdout.write(
          `cargo:rerun-if-changed=${path.join(root, file)}\n`,
        );
    }
    if (process.argv.includes("--refresh"))
      refreshCodexSchemas(root, undefined, process.argv.includes("--check"));
    const rustOnly = process.argv.includes("--rust-only");
    if (process.argv.includes("--write"))
      for (const entry of renderCodexContracts(root, read, rustOnly))
        writeGenerated(path.join(root, entry.path), entry.text);
    else checkCodexContracts(root, read, rustOnly);
    if (!rustOnly) {
      if (process.argv.includes("--write")) {
        const entry = await renderClaudeRateContract(root);
        writeGenerated(path.join(root, entry.path), entry.text);
        const provenance = renderClaudeProvenance(root, entry);
        writeGenerated(path.join(root, provenance.path), provenance.text);
      } else await checkClaudeRateContract(root);
    }
    checkClaudeRateFingerprint(root);
  }
}
