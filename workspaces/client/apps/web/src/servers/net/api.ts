export { READ_DEADLINE_MS, untilAnswered } from "../../api.ts";
export {
  isApiSchemaHashMismatch,
  isApiSchemaMismatchHandshake,
  isApiSchemaMismatchResponse,
} from "../../apiErrors.ts";
import { createApiStorage } from "../../apiStorage.ts";
import { READ_DEADLINE_MS } from "../../api.ts";
import { defaultNetStatus } from "./status.ts";
import {
  ProcessActionStatus,
  MonitorInputKind,
  MonitorInputStatus,
} from "../../generated/api.ts";
import { createCommandClient } from "../../commands.ts";
import { createPendingCommands } from "../../pendingCommands.ts";
import { onEntitySequenceAdvance } from "../../sync/entitySequence.ts";
import { hasNativeClient } from "../../native/client.ts";

import { makeAutoObservable } from "mobx";

import {
  beginServerMutation,
  pendingMonitorInput as pendingInput,
  finishMonitorInput,
} from "../mutationIdentity.ts";

import {
  serverLocalStorage as defaultStorage,
  serverSessionStorage as sessionStorage,
} from "../storage.ts";

import { apiOrigin, serverFetch } from "./transport.ts";

import createClient from "openapi-fetch";

import type { FetchResponse } from "openapi-fetch";

import type {
  ApiPathsFor as PathsFor,
  ApiOperation as Operation,
  GetResult,
  PostResult,
  PostBody,
  PostInput,
} from "../../apiTypes.ts";
export type {
  ApiGetPath,
  ApiPostPath,
  GetResult,
  PostResult,
  PostBody,
  PostInput,
} from "../../apiTypes.ts";

import type { components, paths } from "../../generated/api.ts";

import {
  API_SCHEMA_HASH,
  API_SCHEMA_HASH_HEADER,
  API_SCHEMA_MISMATCH_HEADER,
} from "../../generated/apiSchema.ts";

import type {
  ApiGetOptions,
  ApiPathsWithRequiredQuery,
  ApiQueryFor,
} from "../../apiContracts.ts";

import { onResume } from "../../sync/resume.ts";

import {
  ApiSchemaMismatchError,
  NetworkTimeoutError,
  isApiSchemaMismatchResponse,
  errorPayload,
} from "../../apiErrors.ts";
export {
  ApiError,
  ApiSchemaMismatchError,
  NetworkTimeoutError,
  errorText,
} from "../../apiErrors.ts";

type QueryOf<Op> = ApiQueryFor<Op>;

type QueryOfPath<Path extends PathsFor<"get">> = Path extends keyof paths
  ? ApiQueryFor<NonNullable<paths[Path]["get"]>>
  : never;

type InternalGetOptions<Path extends PathsFor<"get">> = ApiOptions & {
  query?: QueryOf<Operation<Path, "get">> | undefined;
  etag?: string | undefined;
  readMetadata?: ApiReadMetadata | undefined;
};

type PathsWithRequiredQuery = ApiPathsWithRequiredQuery<paths>;

type DownloadInit<Path extends PathsFor<"get">> = {
  parseAs: "blob";
  params?: { query?: ApiQueryFor<NonNullable<paths[Path]["get"]>> };
  headers?: Record<string, string>;
  [key: string]: unknown;
};

type JsonGetInit<Path extends PathsFor<"get">> = {
  parseAs: "json";
  params?: { query?: ApiQueryFor<NonNullable<paths[Path]["get"]>> };
  headers?: Record<string, string>;
  signal?: AbortSignal;
  cache?: RequestCache;
  [key: string]: unknown;
};

type JsonPostInit<Path extends PathsFor<"post">> = {
  parseAs: "json";
  body: PostBody<Path>;
  headers?: Record<string, string>;
  signal?: AbortSignal;
  [key: string]: unknown;
};

type GetFetchResponse<Path extends PathsFor<"get">> = FetchResponse<
  NonNullable<paths[Path]["get"]>,
  JsonGetInit<Path>,
  "application/json"
>;

type PostFetchResponse<Path extends PathsFor<"post">> = FetchResponse<
  NonNullable<paths[Path]["post"]>,
  JsonPostInit<Path>,
  "application/json"
>;

export type ApiReadMetadata = {
  etag?: string | undefined;
  notModified?: boolean;
};

export type ApiOptions = {
  timeoutMs?: number | undefined;
  workspaceId?: string | undefined;
  sessionToken?: string | undefined;
  signal?: AbortSignal | undefined;
  cache?: RequestCache | undefined;
};

export type GetOptions<Path extends PathsFor<"get">> = ApiGetOptions<
  Operation<Path, "get">,
  ApiOptions,
  ApiReadMetadata
>;

export type PostOptions = ApiOptions & { requestId?: string };

/** A read waits this long for a slow server. Only a lost connection should end it. */

const deadlines = new Map<
  AbortController,
  { at: number; expire: () => void }
>();

if (typeof window !== "undefined")
  onResume(() => {
    // iOS can suspend JavaScript timers while the request remains unresolved.
    const now = Date.now();
    for (const deadline of deadlines.values())
      if (now >= deadline.at) deadline.expire();
  });

const SCHEMA_UPDATE_ATTEMPT_KEY = "studio-api-schema-update-attempted";

const SERVICE_WORKER_UPDATE_TIMEOUT_MS = 20_000;

async function withUpdateTimeout<T>(work: Promise<T>) {
  return new Promise<T>((resolve, reject) => {
    const timeout = setTimeout(
      () => reject(new Error("Studio update timed out.")),
      SERVICE_WORKER_UPDATE_TIMEOUT_MS,
    );
    work.then(
      (value) => {
        clearTimeout(timeout);
        resolve(value);
      },
      (error: unknown) => {
        clearTimeout(timeout);
        reject(error);
      },
    );
  });
}

function requestController(options: ApiOptions, timeoutMs: number | undefined) {
  const controller =
    timeoutMs || options.signal ? new AbortController() : undefined;
  const cancel = () => controller?.abort(options.signal?.reason);
  if (options.signal?.aborted) cancel();
  else options.signal?.addEventListener("abort", cancel, { once: true });
  let timedOut = false;
  const expire = () => {
    if (!controller?.signal.aborted) {
      timedOut = true;
      controller?.abort();
    }
  };
  const timer = timeoutMs ? setTimeout(expire, timeoutMs) : undefined;
  if (controller && timeoutMs)
    deadlines.set(controller, { at: Date.now() + timeoutMs, expire });
  return {
    signal: controller?.signal,
    timedOut: () => timedOut,
    finish: () => {
      clearTimeout(timer);
      options.signal?.removeEventListener("abort", cancel);
      if (controller) deadlines.delete(controller);
    },
  };
}

// Only reads and operations with a durable request identity use this deadline.
type SyncGetOptions<Path extends PathsFor<"get">> = Omit<
  GetOptions<Path>,
  "timeoutMs"
>;

export type QueueView = GetResult<"/api/queue">;

export type QueueItemDto = QueueView["items"][number];

/** Each instance owns one server's session, schema gate, workspace and persistence. */
export type ApiScopeOptions = {
  origin: string;
  fetch: (request: Request) => Promise<Response>;
  localStorage: Storage;
  sessionStorage: Storage;
  beginMutation: typeof beginServerMutation;
  requestStatus?: { begin(): (error?: unknown) => void } | undefined;
};
export type ProjectionSessionEvent =
  | { kind: "token"; token: string }
  | { kind: "rejected" }
  | { kind: "workspace"; workspaceId: string };
export function createApiScope(options: ApiScopeOptions) {
  const { localStorage: storage, sessionStorage, requestStatus } = options;
  const apiOrigin = () => options.origin;
  const serverFetch = options.fetch;
  const beginServerMutation = options.beginMutation;
  const posting = makeAutoObservable(
    {
      count: 0,
      begin() {
        this.count++;
      },
      end() {
        this.count--;
      },
    },
    {},
    { autoBind: true },
  );

  let token = "";

  let sessionGeneration = 0;

  let manualTokenGeneration = 0;

  const pendingSessions = new Set<number>();

  let successfulSession: { generation: number; token: string } | undefined;

  let confirmedSession: { generation: number; token: string } | undefined;

  let workspace = "";
  const sessionObservers = new Set<
    (event: ProjectionSessionEvent) => void | Promise<void>
  >();
  function onProjectionSession(
    accept: (event: ProjectionSessionEvent) => void | Promise<void>,
  ) {
    sessionObservers.add(accept);
    return () => {
      sessionObservers.delete(accept);
    };
  }
  function projectionSession(event: ProjectionSessionEvent) {
    return Promise.all(
      [...sessionObservers].map((accept) => accept(event)),
    ).then(() => {});
  }
  function rejectSession() {
    token = "";
    manualTokenGeneration = ++sessionGeneration;
    successfulSession = undefined;
    confirmedSession = undefined;
    activeStreamSession = undefined;
    return projectionSession({ kind: "rejected" });
  }
  let projectionReady = () => true;
  const mutationReady = () => !schemaMismatch && projectionReady();
  function assertMutationReady() {
    if (!mutationReady())
      throw new Error("Studio is syncing. Wait before changing server state.");
  }
  function setMutationReadiness(ready: () => boolean) {
    projectionReady = ready;
    return () => {
      if (projectionReady === ready) projectionReady = () => false;
    };
  }

  let latestStreamSession: components["schemas"]["SyncSessionEvent"];
  let activeStreamSession:
    | components["schemas"]["SyncSessionEvent"]
    | undefined;
  let resolveStreamSession!: () => void;
  const streamSession = new Promise<void>((resolve) => {
    resolveStreamSession = resolve;
  });

  function receiveStreamSession(
    event: components["schemas"]["SyncSessionEvent"],
  ) {
    if (!/^[a-f0-9]{32}$/.test(event.workspaceId))
      throw new Error("Invalid workspace identity");
    void projectionSession({
      kind: "workspace",
      workspaceId: event.workspaceId,
    });
    workspace = event.workspaceId;
    if (event.token !== undefined) setToken(event.token);
    latestStreamSession = event;
    activeStreamSession = event;
    resolveStreamSession();
  }

  function getStreamSession() {
    return activeStreamSession;
  }

  function clearStreamSession(sessionId: string) {
    if (activeStreamSession?.sessionId === sessionId)
      activeStreamSession = undefined;
  }

  function waitForStreamSession() {
    return streamSession.then(() => latestStreamSession);
  }

  const schemaMismatchListeners = new Set<() => void>();

  const matchingSchemaListeners = new Set<() => void>();

  let schemaMismatch = false;

  let mismatchedServerSchema: string | undefined;

  let matchingSchemaResponseGeneration = 0;

  function isApiSchemaMismatch() {
    return schemaMismatch;
  }

  function matchingApiSchemaResponseGeneration() {
    return matchingSchemaResponseGeneration;
  }

  function onMatchingApiSchemaResponse(listener: () => void) {
    matchingSchemaListeners.add(listener);
    if (matchingSchemaResponseGeneration > 0 && !schemaMismatch) listener();
    return () => matchingSchemaListeners.delete(listener);
  }

  function onApiSchemaMismatch(listener: () => void) {
    schemaMismatchListeners.add(listener);
    if (schemaMismatch) listener();
    return () => {
      schemaMismatchListeners.delete(listener);
    };
  }

  function markApiSchemaMismatch(serverHash?: unknown) {
    if (schemaMismatch) return;
    if (typeof serverHash === "string" && /^[a-f0-9]{64}$/.test(serverHash))
      mismatchedServerSchema = serverHash;
    schemaMismatch = true;
    for (const listener of schemaMismatchListeners) listener();
  }

  function schemaUpdateFailedAfterReload() {
    try {
      const attempt = sessionStorage.getItem(SCHEMA_UPDATE_ATTEMPT_KEY);
      return attempt === "1" || !!attempt?.startsWith("auto:");
    } catch {
      return false;
    }
  }

  function clearSchemaUpdateAttemptAfterMatch() {
    try {
      sessionStorage.removeItem(SCHEMA_UPDATE_ATTEMPT_KEY);
    } catch {
      // Storage may be disabled; the mismatch gate still works in memory.
    }
  }

  async function updateRendererAndReload(
    attempt = "1",
    canReload = () => true,
  ) {
    try {
      if (
        (window as Window & { codexDesktop?: unknown }).codexDesktop ||
        hasNativeClient() ||
        !navigator.serviceWorker
      ) {
        if (!canReload()) return false;
        sessionStorage.setItem(SCHEMA_UPDATE_ATTEMPT_KEY, attempt);
        window.location.reload();
        return true;
      }
      const registration = await navigator.serviceWorker.getRegistration("/");
      if (!registration)
        throw new Error(
          "Studio's service worker is not registered. Reload Studio and try again.",
        );
      await withUpdateTimeout(registration.update());
      const worker = registration.waiting ?? registration.installing;
      if (worker) {
        await new Promise<void>((resolve, reject) => {
          const timeout = setTimeout(
            () => finish(new Error("Studio update timed out.")),
            SERVICE_WORKER_UPDATE_TIMEOUT_MS,
          );
          const finish = (error?: Error) => {
            clearTimeout(timeout);
            worker.removeEventListener("statechange", changed);
            if (error) reject(error);
            else resolve();
          };
          let requestedActivation = false;
          const changed = () => {
            if (worker.state === "installed" && !requestedActivation) {
              requestedActivation = true;
              worker.postMessage({ type: "STUDIO_SKIP_WAITING" });
            }
            if (worker.state === "activated") finish();
            else if (worker.state === "redundant")
              finish(new Error("Studio update could not be installed."));
          };
          worker.addEventListener("statechange", changed);
          changed();
        });
      }
      if (!canReload()) return false;
      sessionStorage.setItem(SCHEMA_UPDATE_ATTEMPT_KEY, attempt);
      const url = new URL(window.location.href);
      url.searchParams.set("studio-update", String(Date.now()));
      window.location.assign(url.href);
      return true;
    } catch (error) {
      try {
        sessionStorage.removeItem(SCHEMA_UPDATE_ATTEMPT_KEY);
      } catch {
        // Storage may be unavailable; a failed update remains retryable in memory.
      }
      throw error;
    }
  }

  async function automaticallyUpdateRenderer(
    canReload: () => boolean,
    signal: AbortSignal,
  ) {
    if (!schemaMismatch || !mismatchedServerSchema || !canReload())
      return false;
    // Read the renderer origin, which can differ from the selected API server.
    // This file is generated with the assets and never enters the offline cache.
    const response = await fetch(
      new URL("./studio-renderer.json", window.location.href),
      {
        cache: "no-store",
        redirect: "error",
        signal: AbortSignal.any([signal, AbortSignal.timeout(5000)]),
      },
    );
    if (!response.ok) return false;
    const manifest: unknown = await response.json();
    if (!manifest || typeof manifest !== "object") return false;
    const { version, build, apiSchema } = manifest as Record<string, unknown>;
    if (
      version !== 1 ||
      typeof build !== "string" ||
      !/^[a-f0-9]{16}$/.test(build) ||
      apiSchema !== mismatchedServerSchema ||
      apiSchema === API_SCHEMA_HASH ||
      build ===
        document
          .querySelector('meta[name="studio-build"]')
          ?.getAttribute("content")
    )
      return false;
    const attempt = `auto:${apiSchema}:${build}`;
    // One attempt per target build. A broken deployment must not reload forever.
    if (sessionStorage.getItem(SCHEMA_UPDATE_ATTEMPT_KEY) === attempt)
      return false;
    return updateRendererAndReload(
      attempt,
      () => !signal.aborted && canReload(),
    );
  }

  const fetchApiRequest = async (request: Request) => {
    if (schemaMismatch) throw new ApiSchemaMismatchError();
    const headers = new Headers(request.headers);
    headers.set(API_SCHEMA_HASH_HEADER, API_SCHEMA_HASH);
    const response = await serverFetch(new Request(request, { headers }));
    if (
      (response.status === 401 || response.status === 403) &&
      ["/api/session", "/api/sync/identity", "/api/sync/stream"].includes(
        new URL(request.url).pathname,
      )
    )
      await rejectSession();
    const serverHash = response.headers.get(API_SCHEMA_HASH_HEADER);
    if (isApiSchemaMismatchResponse(response, request)) {
      markApiSchemaMismatch(serverHash);
      await response.body?.cancel().catch(() => {});
      // Do not let a response from another contract reach a projection or
      // caller. The update gate is still raised before rejection.
      throw new ApiSchemaMismatchError(true);
    } else if (serverHash === API_SCHEMA_HASH) {
      matchingSchemaResponseGeneration++;
      clearSchemaUpdateAttemptAfterMatch();
      for (const listener of matchingSchemaListeners) {
        try {
          listener();
        } catch {
          // Schema confirmation observers must not break a successful request.
        }
      }
    }
    return response;
  };

  const client = createClient<paths, "application/json">({
    baseUrl: apiOrigin(),
    fetch: fetchApiRequest,
  });

  function serverAccess(
    method: "GET",
    body?: undefined,
    timeoutMs?: number,
    sessionToken?: string,
  ): Promise<import("../accessContract.ts").ServerAccessState>;
  function serverAccess(
    method: "POST",
    body: import("../accessContract.ts").ServerAccessRequest,
    timeoutMs?: number,
    sessionToken?: string,
  ): Promise<PostResult<"/api/multi-server">>;
  async function serverAccess(
    ...args:
      | [
          method: "GET",
          body?: undefined,
          timeoutMs?: number | undefined,
          sessionToken?: string | undefined,
        ]
      | [
          method: "POST",
          body: import("../accessContract.ts").ServerAccessRequest,
          timeoutMs?: number | undefined,
          sessionToken?: string | undefined,
        ]
  ): Promise<
    | import("../accessContract.ts").ServerAccessState
    | PostResult<"/api/multi-server">
  > {
    const [method, body, timeoutMs, sessionToken] = args;
    if (method === "POST") assertMutationReady();
    const controller = requestController({}, timeoutMs ?? 15000);
    const finishRequest = requestStatus?.begin();
    let failure: unknown;
    try {
      const signal = controller.signal ? { signal: controller.signal } : {};
      const result =
        method === "GET"
          ? await client.GET("/api/multi-server", signal)
          : await client.POST("/api/multi-server", {
              body,
              ...signal,
              headers: {
                "X-Canvas-Token": sessionToken ?? token,
                "X-Canvas-Workspace": workspace,
              },
            });
      if (!result.response.ok)
        throw errorPayload(result.error, result.response.status);
      if (!result.data) throw new Error("The server access response is empty.");
      return result.data;
    } catch (error) {
      failure = controller.timedOut() ? new NetworkTimeoutError() : error;
      throw failure;
    } finally {
      controller.finish();
      finishRequest?.(failure);
    }
  }

  // The openapi-fetch generic accepts a correlated path/init pair. TypeScript
  // cannot preserve that correlation inside the generic transport functions;
  // these adapters keep the generated path, params, body, and response types.
  const requestGet = client.GET as <Path extends PathsFor<"get">>(
    path: Path,
    init: JsonGetInit<Path>,
  ) => Promise<GetFetchResponse<Path>>;

  const requestPost = client.POST as <Path extends PathsFor<"post">>(
    path: Path,
    init: JsonPostInit<Path>,
  ) => Promise<PostFetchResponse<Path>>;

  const requestDownload = client.GET as <Path extends PathsFor<"get">>(
    path: Path,
    init: DownloadInit<Path>,
  ) => Promise<
    FetchResponse<
      NonNullable<paths[Path]["get"]>,
      DownloadInit<Path>,
      "application/json"
    >
  >;

  function setWorkspace(value: string) {
    workspace = value;
  }

  function setToken(value: string) {
    if (token && token !== value) void rejectSession();
    manualTokenGeneration = ++sessionGeneration;
    successfulSession = undefined;
    confirmedSession = undefined;
    token = value;
    void projectionSession({ kind: "token", token: value });
  }

  async function performGet<Path extends PathsFor<"get">>(
    path: Path,
    options?: InternalGetOptions<Path>,
  ): Promise<GetResult<Path> | undefined> {
    const requestOptions = options ?? {};
    const timeoutMs = requestOptions.timeoutMs ?? READ_DEADLINE_MS;
    const controller = requestController(requestOptions, timeoutMs);
    const finishRequest = requestStatus?.begin();
    let failure: unknown;
    try {
      const fetchOptions = {
        parseAs: "json" as const,
        ...(requestOptions.query === undefined
          ? {}
          : { params: { query: requestOptions.query } }),
        ...(controller.signal ? { signal: controller.signal } : {}),
        ...(requestOptions.cache ? { cache: requestOptions.cache } : {}),
        ...(requestOptions.etag
          ? { headers: { "If-None-Match": requestOptions.etag } }
          : {}),
      };
      // openapi-fetch's path generic loses path/query correlation in this
      // generic implementation. Public overloads validate that correlation;
      // this assertion only maps `query` into the library's `params.query`.
      const result = await requestGet(path, fetchOptions as JsonGetInit<Path>);
      const response = result.response;
      if (response.status === 304 && requestOptions.readMetadata) {
        requestOptions.readMetadata.etag =
          response.headers.get("ETag") || requestOptions.etag;
        requestOptions.readMetadata.notModified = true;
        return undefined;
      }
      if (requestOptions.readMetadata)
        requestOptions.readMetadata.etag =
          response.headers.get("ETag") || undefined;
      if (!response.ok) throw errorPayload(result.error, response.status);
      if (!("data" in result) || result.data == null)
        throw new Error("Successful response did not contain a body.");
      return result.data as GetResult<Path>;
    } catch (error) {
      failure = controller.timedOut() ? new NetworkTimeoutError() : error;
      throw failure;
    } finally {
      controller.finish();
      finishRequest?.(failure);
    }
  }

  function get<Path extends PathsFor<"get">>(
    path: Path,
    options: GetOptions<Path> & { readMetadata: ApiReadMetadata },
  ): Promise<GetResult<Path> | undefined>;

  function get<Path extends PathsWithRequiredQuery>(
    path: Path,
    options: GetOptions<Path> & { readMetadata?: undefined },
  ): Promise<GetResult<Path>>;

  function get<Path extends Exclude<PathsFor<"get">, PathsWithRequiredQuery>>(
    path: Path,
    options?: GetOptions<Path> & { readMetadata?: undefined },
  ): Promise<GetResult<Path>>;

  function get<Path extends PathsFor<"get">>(
    path: Path,
    options?: InternalGetOptions<Path>,
  ): Promise<GetResult<Path> | undefined> {
    return performGet(path, options);
  }

  async function post<Path extends PathsFor<"post">>(
    path: Path,
    body: PostInput<Path>,
    options: PostOptions = {},
  ): Promise<PostResult<Path>> {
    if (schemaMismatch) throw new ApiSchemaMismatchError();
    if (!path.startsWith("/api/sync/")) assertMutationReady();
    const targetWorkspaceId = options.workspaceId ?? workspace;
    const sessionToken = options.sessionToken ?? token;
    const mutation = await beginServerMutation(path, body, options.requestId);
    const timeoutMs = options.timeoutMs ?? (mutation ? 15000 : undefined);
    const controller = requestController(options, timeoutMs);
    let responseRejected = false;
    posting.begin();
    const finishRequest = requestStatus?.begin();
    let failure: unknown;
    try {
      const fetchOptions = {
        parseAs: "json" as const,
        body:
          path === "/api/native-command" && mutation
            ? ({
                ...(body as Omit<
                  PostBody<"/api/native-command">,
                  "request_id"
                >),
                request_id: mutation.requestId,
              } satisfies PostBody<"/api/native-command">)
            : body,
        ...(controller.signal ? { signal: controller.signal } : {}),
        headers: {
          "Content-Type": "application/json",
          ...(mutation ? { "X-Studio-Request-Id": mutation.requestId } : {}),
          "X-Canvas-Token": sessionToken,
          ...(targetWorkspaceId
            ? { "X-Canvas-Workspace": targetWorkspaceId }
            : {}),
        },
      };
      const result = await requestPost(
        path,
        fetchOptions as JsonPostInit<Path>,
      );
      if (!result.response.ok) {
        responseRejected = true;
        await mutation?.reject(result.response.status, result.error);
      }
      if (
        !result.response.ok &&
        result.response.headers.get(API_SCHEMA_MISMATCH_HEADER) === "1"
      )
        throw new ApiSchemaMismatchError(true);
      if (!result.response.ok)
        throw errorPayload(result.error, result.response.status);
      if (!("data" in result) || result.data == null)
        throw new Error("Successful response did not contain a body.");
      const data = result.data as PostResult<Path>;
      await mutation?.finish();
      return data;
    } catch (error) {
      try {
        if (!responseRejected) await mutation?.unknown();
        failure = controller.timedOut() ? new NetworkTimeoutError() : error;
      } catch (journalError) {
        failure = journalError;
      }
      throw failure;
    } finally {
      posting.end();
      controller.finish();
      finishRequest?.(failure);
    }
  }

  function apiDownload<Path extends PathsWithRequiredQuery>(
    path: Path,
    query: QueryOfPath<Path>,
  ): Promise<{ blob: Blob; name: string; truncated: boolean }>;

  function apiDownload<
    Path extends Exclude<PathsFor<"get">, PathsWithRequiredQuery>,
  >(
    path: Path,
    query?: QueryOfPath<Path>,
  ): Promise<{
    blob: Blob;
    name: string;
    truncated: boolean;
  }>;

  async function apiDownload<Path extends PathsFor<"get">>(
    path: Path,
    query?: QueryOfPath<Path>,
  ): Promise<{
    blob: Blob;
    name: string;
    truncated: boolean;
  }> {
    const fetchOptions = {
      ...(query === undefined ? {} : { params: { query } }),
      parseAs: "blob" as const,
      headers: {
        "X-Canvas-Token": token,
        ...(workspace ? { "X-Canvas-Workspace": workspace } : {}),
      },
    };
    const finishRequest = requestStatus?.begin();
    let failure: unknown;
    try {
      const result = await requestDownload(
        path,
        fetchOptions as DownloadInit<Path>,
      );
      const response = result.response;
      if (!response.ok) throw errorPayload(result.error, response.status);
      const disposition = response.headers.get("Content-Disposition") || "";
      const name =
        disposition.match(/filename="([^"]+)"/)?.[1] || "download.log";
      if (!("data" in result) || result.data === undefined)
        throw new Error("Download response did not contain a file.");
      return {
        blob: result.data,
        name,
        truncated: response.headers.get("X-Log-Truncated") === "true",
      };
    } catch (error) {
      failure = error;
      throw failure;
    } finally {
      finishRequest?.(failure);
    }
  }

  function syncGet<Path extends PathsFor<"get">>(
    path: Path,
    options: SyncGetOptions<Path> & { readMetadata: ApiReadMetadata },
  ): Promise<GetResult<Path> | undefined>;

  function syncGet<Path extends PathsWithRequiredQuery>(
    path: Path,
    options: SyncGetOptions<Path> & { readMetadata?: undefined },
  ): Promise<GetResult<Path>>;

  function syncGet<
    Path extends Exclude<PathsFor<"get">, PathsWithRequiredQuery>,
  >(
    path: Path,
    options?: SyncGetOptions<Path> & { readMetadata?: undefined },
  ): Promise<GetResult<Path>>;

  function syncGet<Path extends PathsFor<"get">>(
    path: Path,
    options?: SyncGetOptions<Path>,
  ): Promise<GetResult<Path> | undefined> {
    if (schemaMismatch) return Promise.reject(new ApiSchemaMismatchError());
    return performGet(path, { ...options, timeoutMs: READ_DEADLINE_MS });
  }

  const syncPost = <Path extends PathsFor<"post">>(
    path: Path,
    body: PostInput<Path>,
    options?: Omit<PostOptions, "timeoutMs">,
  ) => post(path, body, { ...options, timeoutMs: 15000 });

  // Every write action goes through the one command path (`POST /api/commands`).
  const pending = createPendingCommands();
  const command = createCommandClient(
    storage,
    (body, options) => syncPost("/api/commands", body, options),
    pending,
    assertMutationReady,
  );

  async function refreshSession(): Promise<GetResult<"/api/session">> {
    const generation = ++sessionGeneration;
    pendingSessions.add(generation);
    let session: GetResult<"/api/session">;
    try {
      session = await get("/api/session");
      if (
        generation > manualTokenGeneration &&
        (!successfulSession || successfulSession.generation < generation)
      )
        successfulSession = { generation, token: session.token };
    } finally {
      pendingSessions.delete(generation);
      // A failed newer read releases the last successful credential. Pending
      // newer reads and explicit setToken owners keep their precedence.
      const latest = successfulSession;
      if (
        latest &&
        ![...pendingSessions].some((pending) => pending > latest.generation)
      ) {
        token = latest.token;
        confirmedSession = latest;
      }
    }
    // A delayed response cannot replace a newer confirmed server credential.
    await projectionSession({ kind: "token", token });
    if (confirmedSession && confirmedSession.generation > generation)
      return { ...session, token: confirmedSession.token };
    return session;
  }

  const { saved, save, stableRequestId, clearStableRequestId } =
    createApiStorage(storage);

  function pendingMonitorInput(id: string) {
    return pendingInput(beginServerMutation, id);
  }

  async function checkMonitorInputReceipt(
    id: string,
    owner: { agent: string; accountKey: string; epoch: number },
  ): Promise<"none" | "unknown" | "acknowledged" | "rejected"> {
    const pending = await pendingMonitorInput(id);
    if (!pending) return "none";
    const receipt = await get("/api/processes/action", {
      query: { requestId: pending.requestId, ...owner },
    });
    const result = receipt.result;
    const response = result?.response;
    if (
      !receipt.found ||
      result?.requestId !== pending.requestId ||
      result.status !== ProcessActionStatus.completed ||
      !response ||
      !("kind" in response) ||
      response.kind !== MonitorInputKind.monitor_input ||
      response.monitorId !== id ||
      (response.status !== MonitorInputStatus.acknowledged &&
        response.status !== MonitorInputStatus.rejected)
    )
      return "unknown";
    return (await finishMonitorInput(
      beginServerMutation,
      id,
      pending.requestId,
    ))
      ? response.status
      : "unknown";
  }
  return {
    onProjectionSession,
    rejectSession,
    mutationReady,
    setMutationReadiness,
    pendingMonitorInput,
    checkMonitorInputReceipt,
    isApiSchemaMismatch,
    matchingApiSchemaResponseGeneration,
    onMatchingApiSchemaResponse,
    onApiSchemaMismatch,
    markApiSchemaMismatch,
    schemaUpdateFailedAfterReload,
    clearSchemaUpdateAttemptAfterMatch,
    updateRendererAndReload,
    automaticallyUpdateRenderer,
    fetchApiRequest,
    client,
    serverAccess,
    receiveStreamSession,
    getStreamSession,
    clearStreamSession,
    waitForStreamSession,
    setWorkspace,
    getWorkspaceId: () => workspace,
    setToken,
    getToken: () => token,
    get,
    post,
    apiDownload,
    syncGet,
    syncPost,
    command,
    pending,
    posting,
    refreshSession,
    saved,
    save,
    stableRequestId,
    clearStableRequestId,
  };
}

const defaultApi = createApiScope({
  origin: apiOrigin(),
  fetch: serverFetch,
  localStorage: defaultStorage,
  sessionStorage,
  beginMutation: beginServerMutation,
  requestStatus: defaultNetStatus,
});
export const {
  onProjectionSession,
  rejectSession,
  mutationReady,
  setMutationReadiness,
  posting,
  pendingMonitorInput,
  checkMonitorInputReceipt,
  isApiSchemaMismatch,
  matchingApiSchemaResponseGeneration,
  onMatchingApiSchemaResponse,
  onApiSchemaMismatch,
  markApiSchemaMismatch,
  schemaUpdateFailedAfterReload,
  clearSchemaUpdateAttemptAfterMatch,
  updateRendererAndReload,
  automaticallyUpdateRenderer,
  fetchApiRequest,
  client,
  serverAccess,
  receiveStreamSession,
  getStreamSession,
  clearStreamSession,
  waitForStreamSession,
  setWorkspace,
  getWorkspaceId,
  setToken,
  getToken,
  get,
  post,
  apiDownload,
  syncGet,
  syncPost,
  command,
  pending,
  refreshSession,
  saved,
  save,
  stableRequestId,
  clearStableRequestId,
} = defaultApi;
onEntitySequenceAdvance((sequence, reset) => {
  if (reset) pending.resetClientSeq();
  pending.observeClientSeq(sequence);
});
export type ServerApi = ReturnType<typeof createApiScope>;
