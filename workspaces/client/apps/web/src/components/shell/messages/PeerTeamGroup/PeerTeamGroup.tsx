import { whileObserved } from "../../../../observed.ts";
import type { ShellFlow } from "../../support/flow.ts";
import { observer } from "mobx-react-lite";
import { requestId as randomUUID } from "studio-claude-bridge/request-id.ts";
import {
  ActionIcon,
  Button,
  Checkbox,
  Menu,
  TextInput,
  UnstyledButton,
} from "@mantine/core";
import { ChevronDown, MoreHorizontal } from "lucide-react";
import {
  useRef,
  useState,
  type FormEvent,
  type HTMLAttributes,
  type ReactNode,
} from "react";
import { ApiError, command, errorText } from "../../../../servers/net/api.ts";
import type { CommandRequest } from "../../../../commands.ts";
import { MAX_DRAFT_TEXT_CODEPOINTS } from "../../../../generated/sharedValues.ts";
import { useSidebarBackend } from "../../../sidebar/support/services.tsx";
import {
  PeerTeamMutation,
  type PeerTeamRequest,
} from "../../../../app/projects/peerTeamMutation.ts";
import type { DeepReadonly } from "../../../../deepReadonly.ts";
import type { components } from "../../../../generated/api.ts";
export type PeerTeam = DeepReadonly<components["schemas"]["PeerTeamEntityDto"]>;
import { createArea } from "../../../../app/area.ts";
import { makeAutoObservable, flowResult, reaction } from "mobx";
import type { SidebarChatView } from "../../../sidebar/support/viewModel.ts";
import type { Project } from "../../../../app/projects/ProjectOrganization.tsx";

import { ChatServerLine } from "../../../sidebar/chat-row/ChatServerLine/ChatServerLine.tsx";

export type PeerTeamActions = {
  readonly scope: string;
  openRoom(id: string): void;
  closeForm(): void;
  edit(team: PeerTeam, action: "save" | "delete"): void;
  toggle(team: PeerTeam): void;
  room(team: PeerTeam): string | undefined;
  disabled(team: PeerTeam): boolean;
  candidates(project: Project, team?: PeerTeam): readonly SidebarChatView[];
};
export const [PeerTeamActionsProvider, usePeerTeamActions] =
  createArea<PeerTeamActions>("PeerTeamActions");

type RadioRequest = {
  body: PeerTeamRequest;
  acknowledged?: boolean;
  rejected?: boolean;
};

type PeerLead = NonNullable<PeerTeam["peerLeads"]>[number];
export const PeerLeadRows = observer(function PeerLeadRows({
  team,
}: {
  team: PeerTeam;
}) {
  return (team.peerLeads ?? []).map((lead) => (
    <PeerLeadRow key={lead.grant} team={team} lead={lead} />
  ));
});
const PeerLeadRow = observer(function PeerLeadRow({
  team,
  lead,
}: {
  team: PeerTeam;
  lead: PeerLead;
}) {
  const actions = usePeerTeamActions();
  return (
    <div className="peer-team-peer" data-peer-lead={lead.grant}>
      <span className="peer-team-peer-label">{lead.label}</span>
      <span className="peer-team-peer-mark">Peer</span>
      {!actions.disabled(team) && <PeerLeadMessage lead={lead} />}
    </div>
  );
});

const PeerLeadMessage = observer(function PeerLeadMessage({
  lead,
}: {
  lead: PeerLead;
}) {
  const { post } = useSidebarBackend();

  const [text, setText] = useState("");
  const [pending, setPending] = useState(false);
  const [error, setError] = useState("");
  // The same text keeps its command object (id and number), so a retry after a lost response
  // is the same command. Changed text makes a new command.
  const attempt = useRef<{ text: string; request: CommandRequest } | null>(
    null,
  );
  const submit = async (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    const body = text.trim();
    if (!body || pending) return;
    const current =
      attempt.current?.text === body
        ? attempt.current
        : {
            text: body,
            request: command.make("lead_message", lead.grant, { text: body }),
          };
    attempt.current = current;
    setPending(true);
    setError("");
    try {
      await post("/api/commands", current.request, { timeoutMs: 15000 });
      attempt.current = null;
      setText("");
    } catch (e) {
      setError(errorText(e));
    } finally {
      setPending(false);
    }
  };
  return (
    <form className="peer-team-message" onSubmit={(e) => void submit(e)}>
      <input
        aria-label={`Message ${lead.label}`}
        value={text}
        maxLength={MAX_DRAFT_TEXT_CODEPOINTS}
        disabled={pending}
        onChange={(e) => setText(e.currentTarget.value)}
      />
      <button type="submit" disabled={pending || !text.trim()}>
        Send
      </button>
      {error && <p role="alert">{error}</p>}
    </form>
  );
});

type GroupProps = {
  team: PeerTeam;
  closed: boolean;
  children: ReactNode;
  itemId?: string;
  serverAlias?: string | undefined;
  serverOffline?: boolean;
  reorder?: HTMLAttributes<HTMLElement>;
  drop?: HTMLAttributes<HTMLElement> & {
    "data-folder-drop"?: string | undefined;
  };
};
export const PeerTeamGroup = observer(function PeerTeamGroup({
  team,
  children,
  itemId,
  drop,
  ...layout
}: GroupProps) {
  return (
    <section
      className="peer-team"
      data-peer-team={team.id}
      data-sidebar-item={itemId}
      {...drop}
    >
      <PeerTeamHeading team={team} {...layout} />
      {children}
      <PeerLeadRows team={team} />
    </section>
  );
});

function createPeerTeamRoom(
  team: PeerTeam,
  backend: ReturnType<typeof useSidebarBackend>,
  actions: PeerTeamActions,
) {
  const { post, saved, save } = backend;
  const requestKey = `studio-radio-open:${actions.scope}:${team.id}`;
  class Room {
    request = saved<RadioRequest | null>(requestKey, null);
    pending = false;
    error = "";
    selectRoom = false;
    constructor() {
      makeAutoObservable(this, {}, { autoBind: true });
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
    openShared() {
      return flowResult(this.open());
    }
    *open(): ShellFlow {
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
      this.remember(next);
      try {
        if (!next.acknowledged && !next.rejected) {
          yield post("/api/peer-teams", next.body, { timeoutMs: 15000 });
          next = { ...next, acknowledged: true };
          this.remember(next);
        }
        if (next.acknowledged) this.selectRoom = true;
        this.remember(null);
      } catch (error) {
        if (
          !next.acknowledged &&
          error instanceof ApiError &&
          error.status >= 400 &&
          error.status < 500
        )
          this.remember({ ...next, rejected: true });
        this.error = errorText(error);
      } finally {
        this.pending = false;
      }
    }
    remember(request: RadioRequest | null) {
      this.request = request;
      save(requestKey, request);
    }
  }
  return new Room();
}

function usePeerTeamRoom(team: PeerTeam) {
  const actions = usePeerTeamActions(),
    backend = useSidebarBackend();
  const [room, setRoom] = useState<ReturnType<typeof createPeerTeamRoom>>();
  return {
    request: room
      ? room.request
      : backend.saved<RadioRequest | null>(
          `studio-radio-open:${actions.scope}:${team.id}`,
          null,
        ),
    pending: room?.pending || false,
    error: room?.error || "",
    openShared() {
      const model = room || createPeerTeamRoom(team, backend, actions);
      setRoom(model);
      return model.openShared();
    },
  };
}

const PeerTeamName = observer(function PeerTeamName({
  team,
}: {
  team: PeerTeam;
}) {
  return <>{team.name}</>;
});
const PeerTeamHeading = observer(function PeerTeamHeading({
  team,
  closed,
  reorder,
  serverAlias,
  serverOffline = false,
}: Omit<GroupProps, "children" | "drop" | "itemId">) {
  const actions = usePeerTeamActions();
  const disabled = actions.disabled(team);
  const room = usePeerTeamRoom(team);
  return (
    <>
      <div className="project-tree-heading">
        <UnstyledButton
          {...reorder}
          aria-description="Drag to reorder. Alt + Up or Down also works."
          className="project-tree-toggle peer-team-toggle"
          aria-expanded={!closed}
          onClick={() => actions.toggle(team)}
        >
          <ChevronDown size={14} />
          <span>
            <PeerTeamName team={team} />
            {serverAlias && (
              <ChatServerLine alias={serverAlias} offline={serverOffline} />
            )}
          </span>
        </UnstyledButton>
        <Menu withinPortal position="bottom-end">
          <Menu.Target>
            <ActionIcon
              className="project-tree-action"
              aria-label={`Options for team ${team.name}`}
            >
              <MoreHorizontal size={15} />
            </ActionIcon>
          </Menu.Target>
          <Menu.Dropdown className="fleet-menu">
            <PeerTeamRoomAction team={team} room={room} />
            <Menu.Item
              disabled={disabled}
              onClick={() => actions.edit(team, "save")}
            >
              Edit team
            </Menu.Item>
            <Menu.Item
              disabled={disabled}
              onClick={() => actions.edit(team, "delete")}
            >
              Dissolve team
            </Menu.Item>
          </Menu.Dropdown>
        </Menu>
      </div>
      {room.request && (
        <div className="peer-team-form" role="alert">
          <p>
            {room.error || "Shared chat request needs confirmation."}{" "}
            {room.request.acknowledged || room.request.rejected
              ? "Continue with the current project."
              : "Retry the same request."}
          </p>
          <Button
            size="compact-xs"
            disabled={room.pending}
            onClick={() => void room.openShared()}
          >
            {room.request.acknowledged || room.request.rejected
              ? "Continue"
              : "Retry"}
          </Button>
        </div>
      )}
    </>
  );
});
const PeerTeamRoomAction = observer(function PeerTeamRoomAction({
  team,
  room,
}: {
  team: PeerTeam;
  room: ReturnType<typeof usePeerTeamRoom>;
}) {
  const actions = usePeerTeamActions();
  return (
    ((team.members ?? []).length === 2 || actions.room(team)) && (
      <Menu.Item
        disabled={room.pending || actions.disabled(team)}
        onClick={() => void room.openShared()}
      >
        Open shared chat
      </Menu.Item>
    )
  );
});

export const PeerTeamForm = observer(function PeerTeamForm({
  project,
  team,
  action,
}: {
  project: Project;
  team?: PeerTeam | undefined;
  action: "save" | "delete";
}) {
  const form = usePeerTeamForm(project, team, action);
  const { draft, error, pending, rejected, committed, submit, reload, close } =
    form;
  return (
    <form
      className="peer-team-form"
      aria-label={
        action === "delete" ? "Dissolve team" : team ? "Edit team" : "New team"
      }
      onSubmit={(e) => {
        e.preventDefault();
        void submit();
      }}
    >
      <strong>
        {action === "delete"
          ? `Dissolve ${team?.name}?`
          : team
            ? "Edit team"
            : "New team"}
      </strong>
      {action === "delete" ? (
        <p>The chats stay in this project.</p>
      ) : (
        <>
          <PeerTeamNameInput draft={draft} />
          <p>Chosen chats can message each other.</p>
          <PeerTeamMemberList project={project} team={team} draft={draft} />
        </>
      )}
      {error && (
        <p role="alert">
          {error}
          {committed
            ? " The change was saved. Retry to finish."
            : rejected
              ? " Reload the project before you edit again."
              : " The result is unknown. Retry the same request."}
        </p>
      )}
      <div className="peer-team-form-actions">
        {rejected ? (
          <Button
            size="compact-xs"
            disabled={pending}
            onClick={() => void reload()}
          >
            Reload project
          </Button>
        ) : (
          <Button
            type="submit"
            size="compact-xs"
            loading={pending}
            disabled={
              action === "save" &&
              (!draft.name.trim() || draft.members.length < 2)
            }
          >
            {draft.frozen
              ? "Retry"
              : action === "delete"
                ? "Dissolve"
                : "Save team"}
          </Button>
        )}
        <Button
          size="compact-xs"
          variant="subtle"
          disabled={pending || (draft.frozen && !rejected && !committed)}
          onClick={close}
        >
          Cancel
        </Button>
      </div>
    </form>
  );
});

function usePeerTeamForm(
  project: Project,
  team: PeerTeam | undefined,
  action: "save" | "delete",
) {
  const backend = useRef(useSidebarBackend()).current;
  const { closeForm: close } = usePeerTeamActions();
  const [draft, setDraft] = useState(() => ({
    name: team?.name || "",
    members: [...(team?.members || [])],
    frozen: false,
  }));
  const [mutation, setMutation] = useState<PeerTeamMutation>();
  const update = (patch: Partial<typeof draft>) =>
    setDraft((previous) => ({ ...previous, ...patch }));
  const id = useRef(team?.id || randomUUID());
  const revision = useRef(project.peerTeamsRevision || 0);
  const submit = () => {
    if (typeof project.path !== "string") {
      const model = mutation || new PeerTeamMutation(() => close());
      model.reportError("This team has no project path.");
      setMutation(model);
      return;
    }
    const model = mutation || new PeerTeamMutation(() => close());
    setMutation(model);
    update({ frozen: true });
    void model.submit(
      {
        path: project.path,
        team_id: id.current,
        expected_revision: revision.current,
        request_id: randomUUID(),
        ...(action === "save"
          ? { action, name: draft.name.trim(), members: [...draft.members] }
          : { action }),
      },
      backend,
    );
  };
  return {
    draft: { ...draft, update },
    submit,
    close,
    error: mutation?.error || "",
    pending: mutation?.pending || false,
    rejected: mutation?.rejected || false,
    committed: mutation?.committed || false,
    reload: () => mutation?.reload(),
  };
}
type TeamDraft = ReturnType<typeof usePeerTeamForm>["draft"];
const PeerTeamNameInput = observer(function PeerTeamNameInput({
  draft,
}: {
  draft: TeamDraft;
}) {
  return (
    <TextInput
      label="Team name"
      value={draft.name}
      maxLength={80}
      disabled={draft.frozen}
      onChange={(e) => draft.update({ name: e.currentTarget.value })}
      autoFocus
    />
  );
});
const PeerTeamMemberList = observer(function PeerTeamMemberList({
  project,
  team,
  draft,
}: {
  project: Project;
  team: PeerTeam | undefined;
  draft: TeamDraft;
}) {
  const actions = usePeerTeamActions();
  const candidates = actions.candidates(project, team);
  return (
    <>
      <div className="peer-team-options">
        {candidates.map((row) => (
          <PeerTeamCandidate key={row.id} row={row} draft={draft} />
        ))}
      </div>
      {!candidates.length && <p>No available chats.</p>}
    </>
  );
});
const PeerTeamCandidate = observer(function PeerTeamCandidate({
  row,
  draft,
}: {
  row: SidebarChatView;
  draft: TeamDraft;
}) {
  return (
    <Checkbox
      label={row.agent.name ?? ""}
      checked={draft.members.includes(row.id)}
      disabled={draft.frozen}
      onChange={(e) =>
        draft.update({
          members: e.currentTarget.checked
            ? [...draft.members, row.id]
            : draft.members.filter((id) => id !== row.id),
        })
      }
    />
  );
});

export const PeerTeamMoveFeedback = observer(function PeerTeamMoveFeedback({
  owner,
}: {
  owner: import("../../../../app/projects/peerTeamMutation.ts").PeerTeamMoveOwner;
}) {
  return owner.mutation.pending || owner.mutation.error ? (
    <div className="peer-team-form">
      {owner.mutation.pending ? (
        <p role="status">Updating team…</p>
      ) : (
        <>
          <p role="alert">
            {owner.mutation.error}{" "}
            {owner.mutation.committed
              ? "The change was saved. Retry to finish."
              : owner.mutation.rejected
                ? "Reload the project before another move."
                : "The result is unknown. Retry the same move."}
          </p>
          <Button
            size="compact-xs"
            onClick={() =>
              void (owner.mutation.rejected
                ? owner.mutation.reload()
                : owner.mutation.submit())
            }
          >
            {owner.mutation.rejected ? "Reload project" : "Retry move"}
          </Button>
        </>
      )}
    </div>
  ) : null;
});
