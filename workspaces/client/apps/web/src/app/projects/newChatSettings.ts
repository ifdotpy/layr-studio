import {
  CHAT_WORKSPACE_MODES,
  PROJECT_LAYR_NATIVE_POLICY,
} from "../../generated/workspaceModes.ts";
import {
  get,
  ApiError,
  READ_DEADLINE_MS,
  type GetResult,
  type GetOptions,
  type PostBody,
} from "../../servers/net/api.ts";
import {
  API_SCHEMA_HASH,
  API_SCHEMA_HASH_HEADER,
} from "../../generated/apiSchema.ts";
import { serverCredentialAdapter } from "../../servers/net/transport.ts";
import type { StudioServer } from "../../servers/registry.ts";
import {
  ACCOUNT_KEY_MIN_CHARS,
  ACCOUNT_KEY_MAX_CHARS,
} from "../../generated/sharedValues.ts";

export type NewChatWorkspaceMode = Exclude<
  (typeof CHAT_WORKSPACE_MODES)[number],
  "layr"
>;

export type NewChatSettings = Pick<
  PostBody<"/api/conversation">,
  | "model"
  | "effort"
  | "fast_mode"
  | "daybreak_enabled"
  | "worker_defaults"
  | "review_defaults"
> &
  Pick<
    PostBody<"/api/leads">,
    | "layrStoreRoot"
    | "worker_excluded_account_keys"
    | "reviewer_excluded_account_keys"
    | "account_key"
  > & {
    workspaceMode: NewChatWorkspaceMode;
  };

export type NewChatSettingsDraft = Omit<NewChatSettings, "workspaceMode">;

export function settingsForNewChatRequest(
  settings: NewChatSettingsDraft,
  accountKey: string,
  model: string | undefined,
): NewChatSettingsDraft {
  const { account_key: _accountKey, ...requestSettings } = settings;
  return {
    ...requestSettings,
    account_key: accountKey,
    model,
  };
}

export function settingsForNewChatAccount(
  settings: NewChatSettingsDraft,
  accountKey: string,
): NewChatSettingsDraft {
  return {
    ...settings,
    account_key: accountKey,
    model: undefined,
    effort: null,
    fast_mode: false,
    daybreak_enabled: false,
    worker_defaults: settings.worker_defaults,
    review_defaults: settings.review_defaults,
  };
}

/** A project whose policy is native Layr offers no WT workspace. */
export function projectAllowsWorktree(
  project: { workspacePolicy?: string | null | undefined } | undefined,
): boolean {
  return project?.workspacePolicy !== PROJECT_LAYR_NATIVE_POLICY;
}

type ChatReadOptions = Pick<GetOptions<"/api/ui-summary">, "signal">;
export async function readNewChatResource(
  server: StudioServer | undefined,
  path: "/api/ui-summary",
  options: ChatReadOptions = {},
): Promise<GetResult<"/api/ui-summary">> {
  if (!server || server.id === "local") {
    return get("/api/ui-summary", { signal: options.signal });
  }
  const url = new URL(path, server.origin);
  const response = await serverCredentialAdapter().fetch(
    server,
    new Request(url, {
      headers: { [API_SCHEMA_HASH_HEADER]: API_SCHEMA_HASH },
      signal: options.signal
        ? AbortSignal.any([
            options.signal,
            AbortSignal.timeout(READ_DEADLINE_MS),
          ])
        : AbortSignal.timeout(READ_DEADLINE_MS),
      redirect: "error",
    }),
  );
  const value = await response.json();
  if (response.headers.get(API_SCHEMA_HASH_HEADER) !== API_SCHEMA_HASH)
    throw new Error("The selected server requires a Studio update.");
  if (!response.ok) throw new ApiError(value, response.status);
  return value as GetResult<"/api/ui-summary">;
}

const record = (value: unknown): value is Record<string, unknown> =>
  !!value && typeof value === "object" && !Array.isArray(value);
const nullableName = (value: unknown) =>
  value === null || (typeof value === "string" && !!value.trim());
export function isNewChatSettings(value: unknown): value is NewChatSettings {
  if (!record(value)) return false;
  const allowed = [
    "workspaceMode",
    "account_key",
    "worker_excluded_account_keys",
    "reviewer_excluded_account_keys",
    "model",
    "effort",
    "fast_mode",
    "daybreak_enabled",
    "worker_defaults",
    "review_defaults",
  ];
  if (Object.keys(value).some((key) => !allowed.includes(key))) return false;
  if (
    value.workspaceMode === "layr" ||
    typeof value.workspaceMode !== "string" ||
    !(CHAT_WORKSPACE_MODES as readonly string[]).includes(value.workspaceMode)
  )
    return false;
  if (
    value.account_key !== undefined &&
    (typeof value.account_key !== "string" || !value.account_key.trim())
  )
    return false;
  if (
    (
      [
        "worker_excluded_account_keys",
        "reviewer_excluded_account_keys",
      ] as const
    ).some((name) => {
      const keys = value[name];
      return (
        keys !== undefined &&
        (!Array.isArray(keys) ||
          keys.some(
            (key) =>
              typeof key !== "string" ||
              !key.trim() ||
              [...key].length < ACCOUNT_KEY_MIN_CHARS ||
              [...key].length > ACCOUNT_KEY_MAX_CHARS,
          ))
      );
    })
  )
    return false;
  if (
    ["model", "effort"].some(
      (key) => value[key] !== undefined && !nullableName(value[key]),
    )
  )
    return false;
  if (
    ["fast_mode", "daybreak_enabled"].some(
      (key) => value[key] !== undefined && typeof value[key] !== "boolean",
    )
  )
    return false;
  const worker = value.worker_defaults;
  if (
    worker !== undefined &&
    (!record(worker) ||
      Object.keys(worker).some(
        (key) =>
          !["model", "effort", "fast_mode", "daybreak_enabled"].includes(key),
      ) ||
      !nullableName(worker.model) ||
      !nullableName(worker.effort) ||
      typeof worker.fast_mode !== "boolean" ||
      (worker.daybreak_enabled !== undefined &&
        typeof worker.daybreak_enabled !== "boolean"))
  )
    return false;
  const review = value.review_defaults;
  return (
    review === undefined ||
    (record(review) &&
      Object.keys(review).every((key) =>
        ["model", "effort", "fast_mode"].includes(key),
      ) &&
      (review.fast_mode === undefined ||
        typeof review.fast_mode === "boolean") &&
      nullableName(review.model) &&
      nullableName(review.effort))
  );
}
