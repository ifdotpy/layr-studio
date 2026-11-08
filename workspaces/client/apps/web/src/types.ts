import { AgentStatus } from "./generated/api.ts";
import { nativeErrorView } from "./nativeErrors.ts";
import type { GetResult } from "./api.ts";
import type {
  AgentEntityDto,
  AgentSummaryEntityDto,
  ChatEntityDto,
  ComplaintEntityDto,
  EdgeEntityDto,
  EventEntityDto,
  MonitorEntityDto,
  PeerTeamEntityDto,
  ProjectEntityDto,
  RoomEntityDto,
  RuleEntityDto,
  TaskEntityDto,
  WorkEntityDto,
  components,
} from "./generated/api.ts";

export type {
  JsonValue,
  JsonInput,
  JsonInputOf,
  Json,
  KnownApiFields,
  TranscriptWireItem,
  TranscriptPageData,
} from "./apiTypes.ts";
import type { JsonInput, TranscriptWireItem } from "./apiTypes.ts";
import type { DeepReadonly } from "./deepReadonly.ts";
export type { DeepReadonly };
export type Agent = DeepReadonly<
  AgentEntityDto &
    Pick<AgentSummaryEntityDto, "projectPath"> &
    Partial<
      Pick<
        AgentSummaryEntityDto,
        "operationalStatus" | "markRead" | "markUnread"
      >
    >
>;
export type Chat = ChatEntityDto;
export type PeerTeam = PeerTeamEntityDto;
export type Room = RoomEntityDto;
export type Complaint = ComplaintEntityDto;
export type BackgroundTask = TaskEntityDto;
export type WorkspaceTask = GetResult<"/api/workspace/tasks">["tasks"][number];
export type Monitor = MonitorEntityDto;
export type Rule = RuleEntityDto;
export type Project = ProjectEntityDto;
export type Event = EventEntityDto;
export type Work = WorkEntityDto;
export type Edge = EdgeEntityDto;

export type FederationPeer = components["schemas"]["FederationPeer"];
export type FederationRoom = components["schemas"]["FederationRoom"];
export type FederationSnapshot = components["schemas"]["FederationSnapshot"];

type TranscriptAsset = NonNullable<TranscriptWireItem["assets"]>[number];
export type LocalMessageAttachment = {
  id: string;
  name: string;
  mime: string;
  image: boolean;
  size: number;
  preview?: string | undefined;
};
export type Message = Omit<
  TranscriptWireItem,
  | "assets"
  | "deliveryStatus"
  | "nativeError"
  | "pending"
  | "role"
  | "text"
  | "threadId"
> & {
  assets?:
    | readonly (TranscriptAsset | LocalMessageAttachment)[]
    | null
    | undefined;
  deliveryStatus?: import("./sync/send.ts").DeliveryState | undefined;
  nativeError?: DeepReadonly<JsonInput> | undefined;
  role: NonNullable<TranscriptWireItem["role"]>;
  text: NonNullable<TranscriptWireItem["text"]>;
  accountKey?: string | undefined;
  author?: string | undefined;
  effort?: string | undefined;
  excerpt?: string | undefined;
  localDelivery?: boolean | undefined;
  model?: string | undefined;
  title?: string | undefined;
  streamPending?: boolean | undefined;
  senderName?: string | undefined;
  sender?: string | undefined;
  seq?: number | undefined;
  created?: number | undefined;
  threadId?: string | undefined;
  timestamp?: number | string | undefined;
};
export const busy = new Set(["running", "starting", "approval"]);
// A stored turn failure is not the current health of the account connection.
export function agentErrorLabel(agent: Agent, error = agent.error): string {
  if (
    error === "Codex app-server is offline" &&
    typeof agent.status === "string" &&
    ["failed", "interrupted", "paused"].includes(agent.status) &&
    !agent.inFlight
  )
    return "The previous attempt stopped because Codex was offline.";
  return nativeErrorView(error).message;
}

export function agentStopReason(agent: Agent, error = agent.error): string {
  if (
    agent.status !== AgentStatus.paused ||
    agent.autoWake !== false ||
    typeof error !== "string"
  )
    return "";
  const reason = error;
  return reason === "Stopped by user" ||
    (reason.startsWith("Stopped by agent ") &&
      reason.slice("Stopped by agent ".length).trim())
    ? reason
    : "";
}
