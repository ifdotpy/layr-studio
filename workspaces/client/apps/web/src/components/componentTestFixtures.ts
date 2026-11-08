import type { WorkspaceEntityDto } from "../generated/api.ts";
import type { Approval } from "../approvals.ts";
import type {
  Project,
  Room,
  Complaint,
  BackgroundTask,
  Monitor,
  Rule,
  PeerTeam,
  Event,
  Work,
} from "../types.ts";
import {
  BackgroundTaskIndicator,
  BackgroundTaskKind,
  BackgroundTaskKindLabel,
  BackgroundTaskLabel,
  TaskRecordKind,
} from "../generated/api";
import assert from "node:assert/strict";
import type { Agent, Message } from "../types";
import type { Attachment } from "./ComposerAttachments";
import type { components } from "../generated/api";

export function backgroundDisplay(
  overrides: Partial<components["schemas"]["BackgroundTaskDisplay"]> = {},
): components["schemas"]["BackgroundTaskDisplay"] {
  return {
    kind: BackgroundTaskKind.command,
    kindLabel: BackgroundTaskKindLabel.command,
    title: "Command",
    active: true,
    indicator: BackgroundTaskIndicator.working,
    label: BackgroundTaskLabel.running,
    inBackgroundList: true,
    actions: ["readOutput"],
    ...overrides,
  };
}

export function required<T>(value: T | undefined | null): T {
  assert.ok(value != null, "Expected fixture result to exist");
  return value;
}

type ComponentRuntime = WorkspaceEntityDto & {
  /** Projects of paired servers, published by this server and shown read-only. */
  remoteProjects?: Project[] | undefined;
  modelsByAccount?:
    | Record<string, components["schemas"]["ModelEntityDto"][]>
    | undefined;
  desktop?: components["schemas"]["DesktopResponse"] | undefined;
  sessionCostsByAgent?:
    | Record<string, components["schemas"]["SessionCostResponse"]>
    | undefined;
  accountCostsByAccount?:
    | Record<string, components["schemas"]["AccountCostResponse"]>
    | undefined;
  accountsSummary?: components["schemas"]["AccountsSummaryDto"] | undefined;
  rateLimitsByAccount?:
    | Record<string, components["schemas"]["RateLimitView"]>
    | undefined;
  rateLimits?: components["schemas"]["RateLimitView"] | null | undefined;
  retainedWorktrees?:
    | components["schemas"]["RetainedWorktreeEntityDto"][]
    | undefined;
  questionHistory?: Approval[] | undefined;
  plans?: components["schemas"]["PlanSummaryEntityDto"][] | undefined;
  annotations?: components["schemas"]["AnnotationEntityDto"][] | undefined;
  studioErrors: components["schemas"]["StudioError"][];
  agents: Agent[];
  rooms: Room[];
  tasks: BackgroundTask[];
  monitors: Monitor[];
  complaints: Complaint[];
  requests: Approval[];
  rules: Rule[];
  projects: Project[];
  /** Project entity sequence by native ID, owned by the entity projection; stale-delete guard. */
  projectSequences?: Record<string, number> | undefined;
  peerTeams: PeerTeam[];
  events: Event[];
  work: Work[];
};

// Legacy fixture records may omit ids and carry fields from an older runtime.
type RuntimeFixture = Omit<
  Partial<ComponentRuntime>,
  "tasks" | "monitors" | "requests" | "complaints"
> & {
  [K in "tasks" | "monitors" | "requests" | "complaints"]?: readonly (Partial<
    ComponentRuntime[K][number]
  > &
    Record<string, unknown>)[];
} & Record<string, unknown>;

export function componentSnapshot(
  threads: Agent[] = [],
  runtime: RuntimeFixture = {},
) {
  const metadata: { entityWorkspaceId?: string; entitiesComplete?: boolean } =
    {};
  return {
    ...metadata,
    token: "test-token",
    stateDir: "test-state",
    threads,
    chats: [] as import("../types.ts").Chat[],
    nodes: threads,
    edges: [] as import("../types.ts").Edge[],
    runtime: {
      studioErrors: [],
      agents: threads,
      rooms: [],
      monitors: [],
      complaints: [],
      requests: [],
      rules: [],
      projects: [],
      peerTeams: [],
      events: [],
      work: [],
      ...(runtime as Partial<ComponentRuntime>),
      tasks: (runtime.tasks ?? []).map((task) => ({
        ...task,
        display:
          task.display ??
          backgroundDisplay({
            kind:
              task.kind === TaskRecordKind.command ||
              (task.kind == null && !!task.command)
                ? "command"
                : "tool",
          }),
      })) as ComponentRuntime["tasks"],
    } as ComponentRuntime,
  };
}

export function componentMessage(overrides: Partial<Message> = {}): Message {
  return { id: "message", role: "user", text: "", ...overrides };
}

export function componentAttachment(
  overrides: Partial<Attachment> = {},
): Attachment {
  return {
    id: "asset",
    name: "Attachment",
    mime: "image/png",
    image: false,
    size: 1,
    ...overrides,
  };
}

/** State, memo, ref and effect slots retained by the component hook fixtures. */
export type ComponentHookCell = {
  value?: unknown;
  current?: unknown;
  deps?: unknown[] | undefined;
  cleanup?: (() => void) | undefined;
};

export function componentAgent(overrides: Partial<Agent>): Agent {
  return { id: "worker", ...overrides };
}

/** Historical component scenarios only; production consumes generated models. */
export type ComponentSnapshot = ReturnType<typeof componentSnapshot>;
