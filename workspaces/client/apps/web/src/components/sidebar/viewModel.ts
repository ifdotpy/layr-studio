import { computed, makeObservable } from "mobx";
import type { components } from "../../generated/api.ts";
import type { DeepReadonly } from "../../deepReadonly.ts";
import type { StateCollections } from "../../generated/observableResources.ts";
import { COMPACT_RECENT_WINDOW_SECONDS } from "../../generated/sharedValues.ts";
import { formatOperationalStatus } from "../../status/operationalStatus.ts";

export type SidebarAgent = DeepReadonly<
  components["schemas"]["AgentSummaryEntityDto"]
>;
type Team = DeepReadonly<components["schemas"]["PeerTeamEntityDto"]>;
export interface SidebarIdentity {
  chatKey(id: string): string;
  projectKey(agent: SidebarAgent): string;
  folderKey(path: string, id: string): string;
  teamKey(id: string): string;
}
export interface SidebarRowContext extends SidebarIdentity {
  readonly collections: StateCollections;
}
export const localSidebarIdentity: SidebarIdentity = {
  chatKey: (id) => id,
  projectKey: (agent) => agent.projectPath || "",
  folderKey: (_path, id) => id,
  teamKey: (id) => id,
};

/** A presentation facade retains the generated facts; it never copies them. */
export class SidebarChatView {
  readonly agent: SidebarAgent;
  private readonly context: SidebarRowContext;
  constructor(agent: SidebarAgent, context: SidebarRowContext) {
    this.agent = agent;
    this.context = context;
    makeObservable(this, {
      id: computed,
      projectPath: computed,
      projectFolder: computed,
      team: computed,
      teamId: computed,
      groupKey: computed,
      activityAt: computed,
      indicator: computed,
      canArchive: computed,
      canChangeProject: computed,
      waitingForLead: computed,
      copyProgress: computed,
      searchText: computed,
    });
  }
  get id() {
    return this.context.chatKey(this.agent.id);
  }
  get projectPath() {
    return this.context.projectKey(this.agent);
  }
  get projectFolder() {
    return this.agent.projectFolder
      ? this.context.folderKey(
          this.agent.projectPath || "",
          this.agent.projectFolder,
        )
      : null;
  }
  get team(): Team | null {
    for (const team of this.context.collections.peerTeams.values())
      if (team.members?.includes(this.agent.id)) return team;
    return null;
  }
  get teamId() {
    return this.team ? this.context.teamKey(this.team.id) : null;
  }
  get groupKey() {
    if (!this.teamId && !this.agent.pinned)
      return JSON.stringify(["items", this.projectPath, this.projectFolder]);
    return JSON.stringify([
      "chats",
      this.projectPath,
      !!this.agent.pinned,
      !!this.agent.archived,
      ...(this.teamId ? ["team", this.teamId] : []),
      ...(this.projectFolder ? [this.projectFolder] : []),
    ]);
  }
  get activityAt() {
    return this.agent.updated === undefined
      ? this.agent.created || 0
      : this.agent.updated || 0;
  }
  get indicator() {
    return formatOperationalStatus(this.agent.operationalStatus);
  }
  get canArchive() {
    return !this.agent.inFlight;
  }
  get canChangeProject() {
    return !this.agent.threadId && !this.agent.inFlight;
  }
  get waitingForLead() {
    return this.agent.stall != null;
  }
  get copyProgress() {
    const wait = this.agent.startWait;
    return wait?.reason === "workspace_copy" &&
      wait.epoch === this.agent.epoch &&
      (this.agent.status === "queued" || this.agent.status === "starting")
      ? wait.progress
      : null;
  }
  compactVisible(nowSeconds = Date.now() / 1000) {
    return (
      !!(this.team?.projectPath === this.agent.projectPath && this.team) ||
      !!this.agent.pinned ||
      this.agent.inFlight ||
      this.agent.markRead?.read === false ||
      this.agent.status === "running" ||
      this.agent.status === "starting" ||
      this.agent.status === "approval" ||
      this.indicator.kind === "unread" ||
      nowSeconds - this.activityAt < COMPACT_RECENT_WINDOW_SECONDS
    );
  }
  get searchText() {
    const a = this.agent;
    const project =
      (a.projectId && this.context.collections.projects.get(a.projectId)) ||
      [...this.context.collections.projects.values()].find(
        (p) => p.path === a.projectPath,
      );
    const labels: string[] = [],
      seen = new Set<string>();
    let folder = a.projectFolder;
    while (folder && !seen.has(folder)) {
      seen.add(folder);
      const value = project?.folders?.find((f) => f.id === folder);
      if (!value) break;
      labels.unshift(value.name);
      folder = value.parentId;
    }
    return `${this.team?.projectPath === a.projectPath ? this.team?.name || "" : ""} ${a.name || ""} ${a.projectPath || ""} ${project?.name || a.project || ""} ${labels.join(" / ")} ${this.context.collections.agentTail.get(a.id)?.tail || ""}`.toLowerCase();
  }
}
export const createSidebarRow = (
  agent: SidebarAgent,
  context: SidebarRowContext,
) => new SidebarChatView(agent, context);

export interface SidebarInputs {
  readonly query: string;
  readonly archived: boolean;
  readonly opened: string | null;
  readonly rank: (group: string, id: string) => number;
}
/** Callers retain transient controls and effective order in their existing owners.
 * A React caller recreates this facade when those plain inputs change. */
export function createSidebarViewModel(
  getCollections: () => StateCollections,
  getInputs: () => SidebarInputs,
  identity: SidebarIdentity = localSidebarIdentity,
) {
  const rows = computed(() => {
    const collections = getCollections();
    return [...collections.agents.values()]
      .filter(
        (a) =>
          a.source === "managed" &&
          a.isLead &&
          !a.sharedRoomId &&
          !a.movedTo &&
          !a.moveImportPending,
      )
      .map((agent) => createSidebarRow(agent, { collections, ...identity }));
  });
  const ordered = computed(() => {
    const { rank } = getInputs();
    return [...rows.get()].sort(
      (a, b) =>
        Number(!!b.agent.pinned) - Number(!!a.agent.pinned) ||
        rank(a.groupKey, a.id) - rank(b.groupKey, b.id) ||
        b.activityAt - a.activityAt,
    );
  });
  const search = computed(() => {
    const needle = getInputs().query.toLowerCase();
    return needle
      ? ordered.get().filter((row) => row.searchText.includes(needle))
      : [];
  });
  const visible = computed(() => {
    const input = getInputs();
    const matching = input.query ? new Set(search.get()) : null;
    return ordered
      .get()
      .filter(
        (row) =>
          !!row.agent.archived === input.archived &&
          (!matching ||
            matching.has(row) ||
            (row.id === input.opened && row.teamId !== null)),
      );
  });
  const groups = computed(() => {
    const result = new Map<string, SidebarChatView[]>();
    for (const row of visible.get()) {
      const group = result.get(row.projectPath);
      if (group) group.push(row);
      else result.set(row.projectPath, [row]);
    }
    return result;
  });
  return {
    get rows() {
      return rows.get();
    },
    get orderedRows() {
      return ordered.get();
    },
    get visibleRows() {
      return visible.get();
    },
    get groups() {
      return groups.get();
    },
    get searchResults() {
      return search.get();
    },
  };
}
