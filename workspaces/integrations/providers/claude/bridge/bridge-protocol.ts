// Wire DTOs are generated from runtime-schema::provider_input::bridge. SDK
// types below are internal adapters for model catalogs, questions and opaque moves.
import type * as Wire from "./bridge-wire.generated.ts";
import type {
  BridgeMethods,
  BridgeNotifications,
  BridgeServerRequests,
} from "./bridge-wire.generated.ts";
export type {
  BridgeMethods,
  BridgeNotifications,
  BridgeServerRequests,
} from "./bridge-wire.generated.ts";
import type { Options, Query, ModelInfo } from "@anthropic-ai/claude-agent-sdk";
import type { AskUserQuestionInput } from "@anthropic-ai/claude-agent-sdk/sdk-tools";
import type { Session } from "./session-state.ts";
import type { MoveProof } from "./move.ts";
import type { ExternalServer, CatalogDependencies } from "./move-external.ts";

export type BridgeRpcId = Wire.RequestId;
export type BridgeEmpty = Wire.BridgeEmpty;
export type BridgeError = Wire.BridgeError;
export type BridgeSuccessResponse<R> = Omit<
  Wire.BridgeSuccessResponse,
  "result"
> & { result: R };
export type BridgeErrorResponse = Wire.BridgeErrorResponse;
export type BridgeResponse<R = unknown> =
  | BridgeSuccessResponse<R>
  | BridgeErrorResponse;
export type BridgeMethod = keyof BridgeMethods;
export type BridgeParams<M extends BridgeMethod> = BridgeMethods[M][0];
export type BridgeResult<M extends BridgeMethod> = BridgeMethods[M][1];
export type BridgeRequest<M extends BridgeMethod = BridgeMethod> = {
  [K in M]: {
    id?: BridgeRpcId | undefined;
    method: K;
    params: BridgeParams<K>;
  };
}[M];
export type BridgeMethodResult<M extends BridgeMethod = BridgeMethod> = {
  [K in M]: { method: K; result: BridgeResult<K> };
}[M];
export type BridgeInput = BridgeParams<"turn/start">["input"][number];
export type BridgeInputOrigin = NonNullable<BridgeInput["inputOrigin"]>;
export type BridgeThreadParams = Wire.ThreadParams;
export type BridgeSessionParams = Omit<BridgeParams<"thread/start">, "cwd"> & {
  cwd?: string | undefined;
};
export type BridgeTurnParams = BridgeParams<"turn/start">;
export type BridgeThreadResult = BridgeResult<"thread/resume">;
export type BridgeTurnResult = BridgeResult<"turn/start">;
export type BridgeTask = Wire.Task;
export type BridgeCommand = BridgeResult<"claude/commands">[number];
export type BridgeRateLimits = BridgeResult<"account/rateLimits/read">;
export type BridgeRateBucket = BridgeRateLimits["rateLimits"];
export type BridgeRateWindow = NonNullable<BridgeRateBucket["primary"]>;
// SDK model values are narrowed while adapting to Studio's model DTO.
export type BridgeModel = Wire.ProviderModel & {
  supportedReasoningEfforts: Array<
    NonNullable<Wire.ProviderModel["supportedReasoningEfforts"]>[number] & {
      reasoningEffort: NonNullable<ModelInfo["supportedEffortLevels"]>[number];
    }
  >;
  defaultReasoningEffort: Extract<
    NonNullable<ModelInfo["supportedEffortLevels"]>[number],
    "medium"
  > | null;
};
export type BridgeMoveProof = MoveProof & {
  compiledToolsJson?: string | undefined;
  externalCatalog?: ExternalServer[] | undefined;
  externalTransports?: CatalogDependencies["transports"] | undefined;
  options: {
    tools: string[];
    studioCatalogHash?: string | undefined;
    externalCatalog?: ExternalServer[] | undefined;
    disallowedTools: string[];
    settingSources: NonNullable<Options["settingSources"]>;
    strictMcpConfig: true;
    model: Session["model"];
    permissionMode: Options["permissionMode"];
    settings: Parameters<Query["applyFlagSettings"]>[0];
    effort: Options["effort"] | null;
    systemPrompt: {
      type: "preset";
      preset: "claude_code";
      append: string;
      snapshot: true;
      excludeDynamicSections: true;
    };
    extraArgs: NonNullable<Options["extraArgs"]>;
  };
  optionsHash: string;
  proofMethod:
    | "saved_snapshot_and_current_ordered_mcp_catalogs"
    | "saved_snapshot_and_identical_studio_options";
};
export type BridgeStoredMoveProof = Omit<BridgeMoveProof, "session">;
export type BridgeMoveSession = Omit<Session, "moveProof"> & {
  moveProof: BridgeStoredMoveProof;
  dynamicToolsJson: string;
};

export type BridgeInputReceiptIdentity =
  BridgeResult<"turn/interrupt">["cancelled"][number];
export type BridgeInterruptReceipt = BridgeResult<"turn/interrupt">;
export type BridgeQueuedInput =
  BridgeResult<"thread/queue/list">["items"][number];
export type BridgePlanStep =
  BridgeNotifications["turn/plan/updated"]["plan"][number];
export type BridgeRepairReport = BridgeResult<"claude/repairReport">;
export type BridgeState = BridgeResult<"claude/state">;
export type BridgeItemEvent = BridgeNotifications["item/started"];
export type BridgeHistoricalItemEvent = Wire.HistoricalItemEvent;
export type BridgeNotificationMethod = keyof BridgeNotifications;
export type BridgeNotificationParams<M extends BridgeNotificationMethod> =
  BridgeNotifications[M];
export type BridgeNotification<
  M extends BridgeNotificationMethod = BridgeNotificationMethod,
> = {
  [K in M]: { method: K; params: BridgeNotificationParams<K> };
}[M];
type NativeQuestion = AskUserQuestionInput["questions"][number];
export type BridgeUserQuestion = NativeQuestion & {
  id: string;
  isOther: true;
  isSecret: false;
};
export type BridgeServerMethod = keyof BridgeServerRequests;
export type BridgeServerParams<M extends BridgeServerMethod> =
  BridgeServerRequests[M][0];
export type BridgeServerResult<M extends BridgeServerMethod> =
  BridgeServerRequests[M][1];
export type BridgeServerRequest<
  M extends BridgeServerMethod = BridgeServerMethod,
> = {
  [K in M]: { id: BridgeRpcId; method: K; params: BridgeServerParams<K> };
}[M];
export type BridgeOutbound = Wire.BridgeOutbound;
