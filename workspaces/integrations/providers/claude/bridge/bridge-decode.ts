import { z, type core } from "zod";
import { bridgeWireDefinitions } from "./bridge-wire-schema.generated.ts";
import type {
  BridgeRequest,
  BridgeResponse,
  BridgeServerMethod,
  BridgeServerResult,
  BridgeMethod,
  BridgeResult,
  BridgeRpcId,
  BridgeOutbound,
} from "./bridge-protocol.ts";

const definitions: Record<string, core.JSONSchema.JSONSchema> =
  bridgeWireDefinitions;
const id = z.union([
  z.string(),
  z.number().int().nonnegative().max(Number.MAX_SAFE_INTEGER),
]);
export class InvalidBridgeRequest extends Error {
  readonly id: BridgeRpcId | undefined;
  readonly code: number;
  constructor(id: BridgeRpcId | undefined, method?: string) {
    super(
      method === undefined
        ? "Invalid Claude bridge request parameters"
        : `Claude does not support ${method}`,
    );
    this.id = id;
    this.code = method === undefined ? -32602 : -32000;
  }
}

const requestEnvelope = z
  .object({
    id: id.optional(),
    method: z.string(),
    params: z.record(z.string(), z.unknown()),
  })
  .passthrough();
const successEnvelope = z.object({ id, result: z.unknown() }).passthrough();
const errorEnvelope = z
  .object({
    id,
    error: z
      .object({
        code: z.number().int(),
        message: z.string(),
        data: z.unknown().optional(),
      })
      .passthrough(),
  })
  .passthrough();

function methodSchemas(owner: string, index: number) {
  const methods = definitions[owner]?.properties;
  if (!methods) throw new Error(`Missing generated bridge contract ${owner}`);
  return new Map(
    Object.entries(methods).map(([name, schema]) => {
      if (
        typeof schema === "boolean" ||
        schema.prefixItems?.[index] === undefined
      )
        throw new Error(`Missing generated bridge tuple ${owner}.${name}`);
      const item = schema.prefixItems[index];
      return [
        name,
        z.fromJSONSchema(
          typeof item === "boolean" ? item : { ...item, $defs: definitions },
        ),
      ];
    }),
  );
}
const params = methodSchemas("BridgeMethods", 0);
const results = methodSchemas("BridgeMethods", 1);
const serverResults = methodSchemas("BridgeServerRequests", 1);

// The checked assertion connects the generated declaration to its generated
// schema. The two are rendered together from the same Rust DTO owner.
export function decodeBridgeInput(
  line: string,
): BridgeRequest | BridgeResponse {
  const value: unknown = JSON.parse(line);
  if (value === null || typeof value !== "object" || Array.isArray(value))
    throw new Error("Invalid Claude bridge frame");
  if ("method" in value) {
    if ("result" in value || "error" in value)
      throw new Error("Ambiguous Claude bridge frame");
    try {
      const envelope = requestEnvelope.parse(value);
      const validator = params.get(envelope.method);
      if (!validator)
        throw new InvalidBridgeRequest(envelope.id, envelope.method);
      return {
        ...envelope,
        params: validator.parse(envelope.params),
      } as BridgeRequest;
    } catch (error) {
      if (error instanceof InvalidBridgeRequest) throw error;
      const requestId = "id" in value ? id.safeParse(value.id) : undefined;
      throw new InvalidBridgeRequest(
        requestId?.success ? requestId.data : undefined,
      );
    }
  }
  if ("result" in value === "error" in value)
    throw new Error("Invalid Claude bridge response");
  return "error" in value
    ? errorEnvelope.parse(value)
    : successEnvelope.parse(value);
}

export function decodeBridgeServerResult<M extends BridgeServerMethod>(
  method: M,
  value: unknown,
): BridgeServerResult<M> {
  const validator = serverResults.get(method);
  if (!validator) throw new Error("Unknown Claude bridge callback method");
  return validator.parse(value) as BridgeServerResult<M>;
}

export function decodeBridgeResult<M extends BridgeMethod>(
  method: M,
  value: unknown,
): BridgeResult<M> {
  const validator = results.get(method);
  if (!validator) throw new Error("Unknown Claude bridge result method");
  return validator.parse(value) as BridgeResult<M>;
}

function schemaFor(name: keyof typeof bridgeWireDefinitions) {
  return z.fromJSONSchema({ $ref: "#/$defs/" + name, $defs: definitions });
}
const outbound = schemaFor("BridgeOutbound");
const providerOptions = schemaFor("BridgeProviderOptions");
export function decodeBridgeOutbound(value: unknown): BridgeOutbound {
  return outbound.parse(value) as BridgeOutbound;
}
export function decodeBridgeProviderOptions(
  text: string,
): import("./bridge-wire.generated.ts").BridgeProviderOptions {
  const value: unknown = JSON.parse(text);
  return providerOptions.parse(
    value,
  ) as import("./bridge-wire.generated.ts").BridgeProviderOptions;
}

const dynamicTools = z.array(schemaFor("ToolDefinition"));
export function decodeBridgeDynamicTools(
  value: unknown,
): import("./runtime-contract.generated.ts").ToolDefinition[] {
  dynamicTools.parse(value);
  // Preserve the proven payload's exact property order for move hashes.
  return value as import("./runtime-contract.generated.ts").ToolDefinition[];
}
