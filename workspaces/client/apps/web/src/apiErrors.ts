import {
  API_SCHEMA_HASH,
  API_SCHEMA_HASH_HEADER,
  API_SCHEMA_MISMATCH_FIELD,
  API_SCHEMA_MISMATCH_HEADER,
} from "./generated/apiSchema.ts";
import { displayError } from "./errorPresentation.ts";
export class ApiError extends Error {
  public status: number;
  public readonly response: unknown;
  public readonly location: string[] | undefined;
  public readonly outcome?: string;
  public readonly catalogPending?: boolean;

  constructor(
    message: unknown,
    status: number,
    response: unknown = message,
    location?: string[],
  ) {
    super(displayError(message) || `Request failed (${status})`);
    this.status = status;
    this.response = response;
    this.location = location;

    if (isRecord(response)) {
      if (typeof response.outcome === "string") this.outcome = response.outcome;
      if (typeof response.catalogPending === "boolean")
        this.catalogPending = response.catalogPending;
    }
  }
}

export class ApiSchemaMismatchError extends ApiError {
  public readonly markedResponse: boolean;
  constructor(markedResponse = false) {
    super("Studio must be updated before using this tab.", 426);
    this.markedResponse = markedResponse;

    this.name = "ApiSchemaMismatchError";
  }
}

export class NetworkTimeoutError extends TypeError {
  constructor() {
    super("The server did not respond in time.");
    this.name = "NetworkTimeoutError";
  }
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

export function errorPayload(value: unknown, status: number): ApiError {
  const message = isRecord(value)
    ? value.error || value.message || `Request failed (${status})`
    : typeof value === "string"
      ? value
      : `Request failed (${status})`;
  const location =
    isRecord(value) && Array.isArray(value.location)
      ? value.location.filter(
          (part): part is string => typeof part === "string",
        )
      : undefined;
  return new ApiError(message, status, value, location);
}

export function errorText(error: unknown) {
  if (error instanceof ApiSchemaMismatchError) return "";
  if (
    error instanceof ApiError &&
    error.message === "Invalid request" &&
    error.location?.length
  )
    return `Invalid request (${error.location.join(".")}).`;
  return displayError(error);
}

export function isApiSchemaHashMismatch(serverHash: unknown) {
  return serverHash !== API_SCHEMA_HASH;
}

export function isApiSchemaMismatchHandshake(payload: unknown) {
  if (!payload || typeof payload !== "object") return true;
  const handshake = payload as Record<string, unknown>;
  return (
    isApiSchemaHashMismatch(handshake.hash) ||
    handshake[API_SCHEMA_MISMATCH_FIELD] === true
  );
}

export function isApiSchemaMismatchResponse(
  response: Response,
  request: Request,
) {
  const serverHash = response.headers.get(API_SCHEMA_HASH_HEADER);
  return (
    response.headers.get(API_SCHEMA_MISMATCH_HEADER) === "1" ||
    (!!serverHash && isApiSchemaHashMismatch(serverHash)) ||
    (response.ok &&
      new URL(request.url).pathname === "/api/sync/identity" &&
      !serverHash)
  );
}
