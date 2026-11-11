import { whileObserved } from "../../observed.ts";
import type { CommandRequest, createCommandClient } from "../../commands.ts";
import type { DeepReadonly } from "../../deepReadonly.ts";
import type { components } from "../../generated/api.ts";
import { reaction } from "mobx";
import type { PeerTeamActions } from "../../components/shell/messages/PeerTeamGroup/PeerTeamGroup.tsx";
import { flowValue } from "../../flowValue.ts";
import { requestId as randomUUID } from "studio-claude-bridge/request-id.ts";
import type { Project } from "./ProjectOrganization.tsx";
import { makeAutoObservable, flow } from "mobx";
import { ApiError, errorText } from "../../api.ts";
import type { paths } from "../../generated/api.ts";
import type { SidebarBackend } from "../../components/sidebar/support/services.tsx";
export type PeerTeamRequest =
  paths["/api/peer-teams"]["post"]["requestBody"]["content"]["application/json"];
/** One body and captured server survive uncertain replies. */
export class PeerTeamMutation {
  request: PeerTeamRequest | null = null;
  committed = false;
  backend: SidebarBackend | null = null;
  pending = false;
  error = "";
  rejected = false;
  readonly complete;
  constructor(
    complete: (clear: () => void, reload: boolean) => void = () => {},
  ) {
    this.complete = complete;
    makeAutoObservable(
      this,
      { backend: false, complete: false },
      { autoBind: true, deep: false },
    );
  }
  clear() {
    this.request = null;
    this.backend = null;
    this.committed = false;
    this.error = "";
    this.rejected = false;
  }
  run = flow(function* (
    this: PeerTeamMutation,
    reload: boolean,
    complete = this.complete,
  ) {
    if (this.pending || (!reload && !this.request)) return;
    this.pending = true;
    if (!reload) this.error = "";
    try {
      if (!reload && !this.committed && this.request) {
        if (!this.backend) throw new Error("The team server is unavailable.");
        yield* flowValue(
          this.backend.post("/api/peer-teams", this.request, {
            timeoutMs: 15000,
          }),
        );
        this.committed = true;
      }
      complete(this.clear, reload);
    } catch (error) {
      this.error = errorText(error);
      if (!reload)
        this.rejected =
          !this.committed &&
          error instanceof ApiError &&
          error.status >= 400 &&
          error.status < 500;
    } finally {
      this.pending = false;
    }
  });
  submit(
    body?: PeerTeamRequest,
    owner?: SidebarBackend,
    complete = this.complete,
  ) {
    if (!this.request && body && owner) {
      this.request = body;
      this.backend = owner;
    }
    return this.run(false, complete);
  }
  reload(complete = this.complete) {
    return this.run(true, complete);
  }
}
export class PeerTeamMoveOwner {
  readonly mutation: PeerTeamMutation;
  readonly backend;
  readonly ownerForProject;
  constructor(
    backend: SidebarBackend,
    notify?: (text: string) => void,
    ownerForProject?: (
      path: string,
      member: string,
      teamId: string | null,
    ) => SidebarBackend,
  ) {
    this.backend = backend;
    this.ownerForProject = ownerForProject;
    this.mutation = new PeerTeamMutation((clear, reload) => {
      clear();
      if (!reload) notify?.("Team membership updated.");
    });
    makeAutoObservable(
      this,
      {
        backend: false,
        ownerForProject: false,
        mutation: false,
      },
      { autoBind: true },
    );
  }
  blocked() {
    return !!this.mutation.request;
  }
  move(project: Project, member: string, teamId: string | null) {
    if (this.blocked() || typeof project.path !== "string") return;
    void this.mutation.submit(
      {
        action: "move",
        path: project.path,
        member,
        team_id: teamId,
        expected_revision: project.peerTeamsRevision || 0,
        request_id: randomUUID(),
      },
      this.ownerForProject?.(project.path, member, teamId) || this.backend,
    );
  }
}

export type PeerTeam = DeepReadonly<components["schemas"]["PeerTeamEntityDto"]>;

type RadioRequest = {
  body: PeerTeamRequest;
  acknowledged?: boolean;
  rejected?: boolean;
};

export function createPeerTeamRoom(
  team: PeerTeam,
  backend: SidebarBackend,
  actions: PeerTeamActions,
) {
  const { saved } = backend;
  const requestKey = `studio-radio-open:${actions.scope}:${team.id}`;
  class Room {
    request = saved<RadioRequest | null>(requestKey, null);
    pending = false;
    error = "";
    selectRoom = false;
    constructor() {
      makeAutoObservable(this, {}, { autoBind: true });
      // integration: SidebarListOwner must own this streamed-room selection reaction.
      whileObserved(this, "pending", () => {
        return reaction(
          () => this.selectRoom && actions.room(team),
          (id) => {
            if (id) {
              this.selectRoom = false;
              actions.openRoom(id);
            }
          },
          { fireImmediately: true },
        );
      });
    }
    openShared = flow(function* (
      this: Room,
      team: PeerTeam,
      backend: SidebarBackend,
      actions: PeerTeamActions,
    ) {
      const requestKey = `studio-radio-open:${actions.scope}:${team.id}`;
      if (this.pending) return;
      const roomId = actions.room(team);
      if (roomId && !this.request) {
        actions.openRoom(roomId);
        return;
      }
      if (typeof team.projectPath !== "string") {
        this.error = "This team has no project path for a shared chat.";
        return;
      }
      this.pending = true;
      this.error = "";
      let next: RadioRequest = this.request || {
        body: {
          action: "radio",
          radio_action: "open",
          path: team.projectPath,
          team_id: team.id,
          request_id: randomUUID(),
        },
      };
      this.remember(next, backend, requestKey);
      try {
        if (!next.acknowledged && !next.rejected) {
          yield* flowValue(
            backend.post("/api/peer-teams", next.body, { timeoutMs: 15000 }),
          );
          next = { ...next, acknowledged: true };
          this.remember(next, backend, requestKey);
        }
        if (next.acknowledged) this.selectRoom = true;
        this.remember(null, backend, requestKey);
      } catch (error) {
        if (
          !next.acknowledged &&
          error instanceof ApiError &&
          error.status >= 400 &&
          error.status < 500
        )
          this.remember({ ...next, rejected: true }, backend, requestKey);
        this.error = errorText(error);
      } finally {
        this.pending = false;
      }
    });
    remember(
      request: RadioRequest | null,
      backend: SidebarBackend,
      key: string,
    ) {
      this.request = request;
      backend.save(key, request);
    }
  }
  return new Room();
}

export function createPeerLeadMessage() {
  const draft = {
    attempt: null as { text: string; request: CommandRequest } | null,
    text: "",
    pending: false,
    error: "",
    *submit(
      lead: NonNullable<PeerTeam["peerLeads"]>[number],
      backend: SidebarBackend,
      make: ReturnType<typeof createCommandClient>["make"],
    ) {
      const text = this.text.trim();
      if (!text || this.pending) return;
      const attempt =
        this.attempt?.text === text
          ? this.attempt
          : { text, request: make("lead_message", lead.grant, { text }) };
      this.attempt = attempt;
      this.pending = true;
      this.error = "";
      try {
        yield* flowValue(
          backend.post("/api/commands", attempt.request, {
            timeoutMs: 15000,
          }),
        );
        this.attempt = null;
        this.text = "";
      } catch (error) {
        this.error = errorText(error);
      } finally {
        this.pending = false;
      }
    },
  };
  return makeAutoObservable(draft, {}, { autoBind: true });
}

export function createPeerTeamForm(
  project: Project,
  team: PeerTeam | undefined,
) {
  const id = team?.id || randomUUID(),
    revision = project.peerTeamsRevision || 0;
  const draft = {
    mutation: undefined as PeerTeamMutation | undefined,
    name: team?.name || "",
    members: [...(team?.members || [])],
    frozen: false,
    update(
      patch: Partial<{ name: string; members: string[]; frozen: boolean }>,
    ) {
      Object.assign(this, patch);
    },
    submit(
      project: Project,
      action: "save" | "delete",
      backend: SidebarBackend,
      close: () => void,
    ) {
      const mutation = (this.mutation ??= new PeerTeamMutation());
      if (typeof project.path !== "string") {
        mutation.error = "This team has no project path.";
        return;
      }
      this.frozen = true;
      return mutation.submit(
        {
          path: project.path,
          team_id: id,
          expected_revision: revision,
          request_id: randomUUID(),
          ...(action === "save"
            ? { action, name: this.name.trim(), members: [...this.members] }
            : { action }),
        },
        backend,
        close,
      );
    },
  };
  return makeAutoObservable(draft, {}, { autoBind: true });
}
