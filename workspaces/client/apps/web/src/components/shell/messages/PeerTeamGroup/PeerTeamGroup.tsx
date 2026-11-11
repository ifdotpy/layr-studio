import { command } from "../../../../servers/net/api.ts";
import { textField } from "../../../../stores/fields.ts";
import { Owned } from "../../../ui/Owned/Owned.tsx";
import {
  createPeerLeadMessage,
  createPeerTeamForm,
  createPeerTeamRoom,
} from "../../../../app/projects/peerTeamMutation.ts";

import { observer } from "mobx-react-lite";

import {
  ActionIcon,
  Button,
  Checkbox,
  Menu,
  TextInput,
  UnstyledButton,
} from "@mantine/core";
import { ChevronDown, MoreHorizontal } from "lucide-react";
import type { HTMLAttributes, ReactNode } from "react";

import { MAX_DRAFT_TEXT_CODEPOINTS } from "../../../../generated/sharedValues.ts";
import { useSidebarBackend } from "../../../sidebar/support/services.tsx";

import type { DeepReadonly } from "../../../../deepReadonly.ts";
import type { components } from "../../../../generated/api.ts";
export type PeerTeam = DeepReadonly<components["schemas"]["PeerTeamEntityDto"]>;
import { createArea } from "../../../../app/area.ts";

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
  const backend = useSidebarBackend();
  return (
    <Owned
      key={JSON.stringify([backend.ownerId, lead.grant])}
      create={createPeerLeadMessage}
    >
      {(state) => (
        <form
          className="peer-team-message"
          onSubmit={(event) => {
            event.preventDefault();
            void state.submit(lead, backend, command.make);
          }}
        >
          <input
            aria-label={`Message ${lead.label}`}
            {...textField(state, "text")}
            maxLength={MAX_DRAFT_TEXT_CODEPOINTS}
            disabled={state.pending}
          />
          <button type="submit" disabled={state.pending || !state.text.trim()}>
            Send
          </button>
          {state.error && <p role="alert">{state.error}</p>}
        </form>
      )}
    </Owned>
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

const PeerTeamHeading = observer(function PeerTeamHeading({
  team,
  closed,
  reorder,
  serverAlias,
  serverOffline = false,
}: Omit<GroupProps, "children" | "drop" | "itemId">) {
  const actions = usePeerTeamActions();
  const disabled = actions.disabled(team);
  const backend = useSidebarBackend();
  return (
    <Owned
      key={JSON.stringify([backend.ownerId, actions.scope, team.id])}
      create={() => createPeerTeamRoom(team, backend, actions)}
    >
      {(room) => (
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
                {team.name}
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
                onClick={() => void room.openShared(team, backend, actions)}
              >
                {room.request.acknowledged || room.request.rejected
                  ? "Continue"
                  : "Retry"}
              </Button>
            </div>
          )}
        </>
      )}
    </Owned>
  );
});
const PeerTeamRoomAction = observer(function PeerTeamRoomAction({
  team,
  room,
}: {
  team: PeerTeam;
  room: ReturnType<typeof createPeerTeamRoom>;
}) {
  const actions = usePeerTeamActions();
  const backend = useSidebarBackend();
  return (
    ((team.members ?? []).length === 2 || actions.room(team)) && (
      <Menu.Item
        disabled={room.pending || actions.disabled(team)}
        onClick={() => void room.openShared(team, backend, actions)}
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
  const backend = useSidebarBackend();
  const { closeForm } = usePeerTeamActions();
  return (
    <Owned
      key={JSON.stringify([backend.ownerId, project.id, team?.id, action])}
      create={() => createPeerTeamForm(project, team)}
    >
      {(form) => {
        const error = form.mutation?.error,
          pending = !!form.mutation?.pending,
          rejected = !!form.mutation?.rejected,
          committed = !!form.mutation?.committed;
        return (
          <form
            className="peer-team-form"
            aria-label={
              action === "delete"
                ? "Dissolve team"
                : team
                  ? "Edit team"
                  : "New team"
            }
            onSubmit={(e) => {
              e.preventDefault();
              void form.submit(project, action, backend, closeForm);
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
                <TextInput
                  label="Team name"
                  {...textField(form, "name")}
                  maxLength={80}
                  disabled={form.frozen}
                  autoFocus
                />
                <p>Chosen chats can message each other.</p>
                <PeerTeamMemberList
                  project={project}
                  team={team}
                  draft={form}
                />
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
                  onClick={() => void form.mutation?.reload(closeForm)}
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
                    (!form.name.trim() || form.members.length < 2)
                  }
                >
                  {form.frozen
                    ? "Retry"
                    : action === "delete"
                      ? "Dissolve"
                      : "Save team"}
                </Button>
              )}
              <Button
                size="compact-xs"
                variant="subtle"
                disabled={pending || (form.frozen && !rejected && !committed)}
                onClick={closeForm}
              >
                Cancel
              </Button>
            </div>
          </form>
        );
      }}
    </Owned>
  );
});
type TeamDraft = ReturnType<typeof createPeerTeamForm>;
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
          <Checkbox
            key={row.id}
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
        ))}
      </div>
      {!candidates.length && <p>No available chats.</p>}
    </>
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
