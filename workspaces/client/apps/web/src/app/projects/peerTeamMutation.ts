import { requestId as randomUUID } from "studio-claude-bridge/request-id.ts";
import type { Project } from "./ProjectOrganization.tsx";
import { makeAutoObservable, runInAction } from "mobx";
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
  constructor(complete: (clear: () => void, reload: boolean) => void) {
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
  async run(reload: boolean) {
    if (this.pending || (!reload && !this.request)) return;
    this.pending = true;
    if (!reload) this.error = "";
    try {
      if (!reload && !this.committed && this.request) {
        if (!this.backend) throw new Error("The team server is unavailable.");
        await this.backend.post("/api/peer-teams", this.request, {
          timeoutMs: 15000,
        });
        runInAction(() => {
          this.committed = true;
        });
      }
      runInAction(() => this.complete(this.clear, reload));
    } catch (error) {
      runInAction(() => {
        this.error = errorText(error);
        if (!reload)
          this.rejected =
            !this.committed &&
            error instanceof ApiError &&
            error.status >= 400 &&
            error.status < 500;
      });
    } finally {
      runInAction(() => {
        this.pending = false;
      });
    }
  }
  submit(body?: PeerTeamRequest, owner?: SidebarBackend) {
    if (!this.request && body && owner) {
      this.request = body;
      this.backend = owner;
    }
    return this.run(false);
  }
  reload() {
    return this.run(true);
  }
  reportError(error: string) {
    this.error = error;
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
