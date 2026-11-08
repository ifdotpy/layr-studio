import {
  ApiError,
  NetworkTimeoutError,
  errorText,
  post,
  type PostBody,
  type PostOptions,
} from "../api.ts";
import type { components } from "../generated/api.ts";
import {
  isServerCommand,
  type ServerCommand,
  type ServerNavigation,
} from "./navigation.ts";

type ErrorResponse = components["schemas"]["ErrorResponse"];
type SidebarError = {
  message: string;
  status?: number;
  // The window sends the same request again after a timeout.
  timedOut?: boolean;
} & Pick<
  ErrorResponse,
  | "location"
  | "code"
  | "metadata"
  | "outcome"
  | "catalogPending"
  | "protocolVersion"
  | "supportedVersions"
> & { status?: number };
const sidebarErrorFields = [
  "location",
  "code",
  "metadata",
  "outcome",
  "catalogPending",
  "protocolVersion",
  "supportedVersions",
] as const satisfies readonly (keyof ErrorResponse)[];

function isRecord(value: unknown): value is Record<string, unknown> {
  return !!value && typeof value === "object" && !Array.isArray(value);
}

function copySidebarErrorFields(value: unknown): Partial<SidebarError> {
  if (!isRecord(value)) return {};
  return Object.fromEntries(
    sidebarErrorFields
      .filter((field) => field in value)
      .map((field) => [field, value[field]]),
  ) as Partial<SidebarError>;
}

/** Serialize only the documented API diagnostics that the sidebar can display. */
export function serializeSidebarError(failure: unknown): SidebarError {
  const error: SidebarError = { message: errorText(failure) };
  if (failure instanceof NetworkTimeoutError) error.timedOut = true;
  if (!(failure instanceof ApiError)) return error;
  error.status = failure.status;
  Object.assign(error, copySidebarErrorFields(failure.response));
  if (failure.location && !("location" in error))
    error.location = failure.location;
  return error;
}

const sidebarPaths = [
  "/api/projects",
  "/api/organization",
  "/api/peer-teams",
  "/api/rename",
] as const;
export type SidebarPostPath = (typeof sidebarPaths)[number];
type PostRequest = {
  [Path in SidebarPostPath]: {
    action: "post";
    path: Path;
    body: PostBody<Path>;
    options?: Pick<PostOptions, "timeoutMs" | "requestId">;
  };
}[SidebarPostPath];
export type SidebarRpcRequest =
  | PostRequest
  | { action: "refresh" }
  | {
      action: "command";
      command: Extract<
        ServerCommand,
        { action: "prepare-chat" | "mark-unread" }
      >;
    };
/** Keep token, mutation identity and sync coverage inside the owning frame. */
export async function executeSidebarRequest(
  value: unknown,
  refresh: () => Promise<ServerNavigation>,
  run: (command: ServerCommand) => void | Promise<void>,
  sessionToken?: string,
): Promise<unknown> {
  if (!value || typeof value !== "object")
    throw new Error("Invalid sidebar request.");
  const request = value as SidebarRpcRequest;
  if (request.action === "refresh") return refresh();
  if (request.action === "command") {
    if (
      !isServerCommand(request.command) ||
      !["prepare-chat", "mark-unread"].includes(request.command.action)
    )
      throw new Error("Invalid sidebar command.");
    return run(request.command);
  }
  if (
    request.action !== "post" ||
    !sidebarPaths.includes(request.path!) ||
    !request.body ||
    typeof request.body !== "object" ||
    Array.isArray(request.body)
  )
    throw new Error("Invalid sidebar request.");
  const options = request.options;
  if (
    options &&
    (Object.keys(options).some(
      (key) => !["timeoutMs", "requestId"].includes(key),
    ) ||
      (options.timeoutMs !== undefined &&
        (!Number.isFinite(options.timeoutMs) ||
          options.timeoutMs <= 0 ||
          options.timeoutMs > 30000)) ||
      (options.requestId !== undefined &&
        typeof options.requestId !== "string"))
  )
    throw new Error("Invalid sidebar request options.");
  if (!sessionToken)
    throw new Error("The sidebar server session is not ready.");
  if (request.path === "/api/organization") {
    const source = await post(request.path, request.body, {
      ...options,
      sessionToken,
    });
    const receipt: Pick<
      typeof source,
      "projectFolder" | "projectFolderRevision"
    > = {};
    if ("projectFolder" in source) receipt.projectFolder = source.projectFolder;
    if ("projectFolderRevision" in source)
      receipt.projectFolderRevision = source.projectFolderRevision;
    return receipt;
  }
  return post(request.path, request.body, { ...options, sessionToken });
}
