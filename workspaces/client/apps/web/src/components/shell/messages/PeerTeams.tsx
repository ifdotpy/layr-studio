import { observer } from "mobx-react-lite";
import { randomUUID } from "../../../uuid";
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
  useEffect,
  useRef,
  useState,
  type FormEvent,
  type HTMLAttributes,
  type ReactNode,
} from "react";
import { ApiError, command, errorText } from "../../../api";
import type { CommandRequest } from "../../../commands";
import { MAX_DRAFT_TEXT_CODEPOINTS } from "../../../generated/sharedValues.ts";
import { useSidebarBackend, type SidebarBackend } from "../../sidebar/services";
import type { paths } from "../../../generated/api";
import type { PeerTeam as MutablePeerTeam, DeepReadonly } from "../../../types";
type PeerTeam = DeepReadonly<MutablePeerTeam>;
import type { SidebarChatView } from "../../sidebar/viewModel.ts";
import type { Project } from "../../ProjectOrganization";
import "./peer-teams.css";
import { ChatServerLine } from "../../../servers/ProjectChatRows";

type PeerTeamRequest =
  paths["/api/peer-teams"]["post"]["requestBody"]["content"]["application/json"];
type RadioRequest = {
  body: PeerTeamRequest;
  acknowledged?: boolean;
  rejected?: boolean;
};

/** Send one command of the peer lead message. A retry sends the same object. */
export type PeerLeadSend = (request: CommandRequest) => Promise<unknown>;

/** A lead of a paired project, shown in its team as a peer. Sends a message when `send` is set. */
export const PeerLeadRows = observer(function PeerLeadRows({
  leads,
  send,
}: {
  leads: PeerTeam["peerLeads"];
  send?: PeerLeadSend | undefined;
}) {
  return (leads ?? []).map((lead) => (
    <div
      key={lead.grant}
      className="peer-team-peer"
      data-peer-lead={lead.grant}
    >
      <span className="peer-team-peer-label">{lead.label}</span>
      <span className="peer-team-peer-mark">Peer</span>
      {send && (
        <PeerLeadMessage label={lead.label} grant={lead.grant} send={send} />
      )}
    </div>
  ));
});

function PeerLeadMessage({
  label,
  grant,
  send,
}: {
  label: string;
  grant: string;
  send: PeerLeadSend;
}) {
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
            request: command.make("lead_message", grant, { text: body }),
          };
    attempt.current = current;
    setPending(true);
    setError("");
    try {
      await send(current.request);
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
        aria-label={`Message ${label}`}
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
}

export const PeerTeamGroup = observer(function PeerTeamGroup({
  team,
  closed,
  toggle,
  edit,
  dissolve,
  children,
  drop,
  reorder,
  itemId,
  roomId,
  openRoom,
  refresh,
  scope,
  disabled = false,
  serverAlias,
  serverOffline = false,
}: {
  itemId?: string;
  roomId?: string | undefined;
  openRoom: (id: string) => void;
  refresh?: (() => Promise<void>) | undefined;
  scope: string;
  disabled?: boolean;
  serverAlias?: string | undefined;
  serverOffline?: boolean;
  team: PeerTeam;
  closed: boolean;
  toggle: () => void;
  edit: () => void;
  dissolve: () => void;
  children: ReactNode;
  reorder?: HTMLAttributes<HTMLElement>;
  drop?: HTMLAttributes<HTMLElement> & {
    "data-folder-drop"?: string | undefined;
  };
}) {
  const backend = useRef(useSidebarBackend()).current;
  const { post, saved, save } = backend;
  // The team's own server takes the command, so a remote team does not send to this one.
  const sendLead: PeerLeadSend = (request) =>
    post("/api/commands", request, { timeoutMs: 15000 });
  const requestKey = `studio-radio-open:${scope}:${team.id}`;
  const [request, setRequest] = useState<RadioRequest | null>(() =>
    saved<RadioRequest | null>(requestKey, null),
  );
  const [pending, setPending] = useState(false);
  const [error, setError] = useState("");
  const [selectRoom, setSelectRoom] = useState(false);
  const lock = useRef(false);
  useEffect(() => {
    if (selectRoom && roomId) {
      setSelectRoom(false);
      openRoom(roomId);
    }
  }, [selectRoom, roomId, openRoom]);
  const openShared = async () => {
    if (lock.current) return;
    if (roomId && !request) {
      openRoom(roomId);
      return;
    }
    if (typeof team.projectPath !== "string") {
      setError("This team has no project path for a shared chat.");
      return;
    }
    lock.current = true;
    setPending(true);
    setError("");
    let next: RadioRequest = request || {
      body: {
        action: "radio",
        radio_action: "open",
        path: team.projectPath,
        team_id: team.id,
        request_id: randomUUID(),
      },
    };
    const remember = () => {
      setRequest(next);
      save(requestKey, next);
    };
    remember();
    try {
      if (!next.acknowledged && !next.rejected) {
        await post("/api/peer-teams", next.body, { timeoutMs: 15000 });
        next = { ...next, acknowledged: true };
        remember();
      }
      await refresh?.();
      if (next.acknowledged) setSelectRoom(true);
      setRequest(null);
      save(requestKey, null);
    } catch (e) {
      if (
        !next.acknowledged &&
        e instanceof ApiError &&
        e.status >= 400 &&
        e.status < 500
      ) {
        next = { ...next, rejected: true };
        remember();
      }
      setError(errorText(e));
    } finally {
      lock.current = false;
      setPending(false);
    }
  };
  return (
    <section
      className="peer-team"
      data-peer-team={team.id}
      data-sidebar-item={itemId}
      {...drop}
    >
      <div className="project-tree-heading">
        <UnstyledButton
          {...reorder}
          aria-description="Drag to reorder. Alt + Up or Down also works."
          className="project-tree-toggle peer-team-toggle"
          aria-expanded={!closed}
          onClick={toggle}
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
            {((team.members ?? []).length === 2 || roomId) && (
              <Menu.Item
                disabled={pending || disabled}
                onClick={() => void openShared()}
              >
                Open shared chat
              </Menu.Item>
            )}
            <Menu.Item disabled={disabled} onClick={edit}>
              Edit team
            </Menu.Item>
            <Menu.Item disabled={disabled} onClick={dissolve}>
              Dissolve team
            </Menu.Item>
          </Menu.Dropdown>
        </Menu>
      </div>
      {request && (
        <div className="peer-team-form" role="alert">
          <p>
            {error || "Shared chat request needs confirmation."}{" "}
            {request.acknowledged || request.rejected
              ? "Refresh to continue."
              : "Retry the same request."}
          </p>
          <Button
            size="compact-xs"
            disabled={pending}
            onClick={() => void openShared()}
          >
            {request.acknowledged || request.rejected ? "Refresh" : "Retry"}
          </Button>
        </div>
      )}
      {children}
      <PeerLeadRows
        leads={team.peerLeads}
        send={disabled ? undefined : sendLead}
      />
    </section>
  );
});

export const PeerTeamForm = observer(function PeerTeamForm({
  project,
  team,
  teams,
  agents,
  action,
  refresh,
  close,
}: {
  project: Project;
  team?: PeerTeam | undefined;
  teams: readonly PeerTeam[];
  agents: readonly SidebarChatView[];
  action: "save" | "delete";
  refresh?: (() => Promise<void>) | undefined;
  close: () => void;
}) {
  const backend = useRef(useSidebarBackend()).current;
  const { post } = backend;
  const [name, setName] = useState(team?.name || "");
  const [members, setMembers] = useState([...(team?.members || [])]);
  const [error, setError] = useState("");
  const [pending, setPending] = useState(false);
  const [frozen, setFrozen] = useState(false);
  const [rejected, setRejected] = useState(false);
  const request = useRef<PeerTeamRequest | null>(null);
  const committed = useRef(false);
  const locked = useRef(false);
  const id = useRef(team?.id || randomUUID());
  const revision = useRef(project.peerTeamsRevision || 0);
  const available = agents.filter(
    (a) =>
      a.projectPath === project.path &&
      !teams.some((t) => t.id !== team?.id && (t.members ?? []).includes(a.id)),
  );
  const submit = async () => {
    if (locked.current) return;
    if (typeof project.path !== "string") {
      setError("This team has no project path.");
      return;
    }
    locked.current = true;
    setPending(true);
    setError("");
    setFrozen(true);
    request.current ||=
      action === "save"
        ? {
            action: "save",
            path: project.path,
            team_id: id.current,
            expected_revision: revision.current,
            request_id: randomUUID(),
            name: name.trim(),
            members,
          }
        : {
            action: "delete",
            path: project.path,
            team_id: id.current,
            expected_revision: revision.current,
            request_id: randomUUID(),
          };
    try {
      if (!committed.current) {
        const body = request.current;
        if (!body) return;
        await post("/api/peer-teams", body, { timeoutMs: 15000 });
        committed.current = true;
      }
      await refresh?.();
      close();
    } catch (e) {
      setError(errorText(e));
      if (
        !committed.current &&
        e instanceof ApiError &&
        e.status >= 400 &&
        e.status < 500
      )
        setRejected(true);
    } finally {
      locked.current = false;
      setPending(false);
    }
  };
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
          <TextInput
            label="Team name"
            value={name}
            maxLength={80}
            disabled={frozen}
            onChange={(e) => setName(e.currentTarget.value)}
            autoFocus
          />
          <p>Chosen chats can message each other.</p>
          <div className="peer-team-options">
            {available.map((a) => (
              <Checkbox
                key={a.id}
                label={a.agent.name ?? ""}
                checked={members.includes(a.id)}
                disabled={frozen}
                onChange={(e) =>
                  setMembers(
                    e.currentTarget.checked
                      ? [...members, a.id]
                      : members.filter((id) => id !== a.id),
                  )
                }
              />
            ))}
          </div>
          {!available.length && <p>No available chats.</p>}
        </>
      )}
      {error && (
        <p role="alert">
          {error}
          {committed.current
            ? " The change was saved. Retry to refresh the sidebar."
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
            onClick={async () => {
              if (locked.current) return;
              locked.current = true;
              setPending(true);
              try {
                await refresh?.();
                close();
              } catch (e) {
                setError(errorText(e));
              } finally {
                locked.current = false;
                setPending(false);
              }
            }}
          >
            Reload project
          </Button>
        ) : (
          <Button
            type="submit"
            size="compact-xs"
            loading={pending}
            disabled={action === "save" && (!name.trim() || members.length < 2)}
          >
            {frozen ? "Retry" : action === "delete" ? "Dissolve" : "Save team"}
          </Button>
        )}
        <Button
          size="compact-xs"
          variant="subtle"
          disabled={pending || (frozen && !rejected && !committed.current)}
          onClick={close}
        >
          Cancel
        </Button>
      </div>
    </form>
  );
});

// Keep the exact mutation after a lost response. A new drag must not replace it.
export function usePeerTeamMove(
  refresh?: () => Promise<void>,
  notify?: (text: string) => void,
  ownerForProject?: (
    path: string,
    member: string,
    teamId: string | null,
  ) => SidebarBackend,
) {
  const defaultBackend = useSidebarBackend();
  const requestBackend = useRef<SidebarBackend | null>(null);
  const request = useRef<PeerTeamRequest | null>(null);
  const committed = useRef(false);
  const running = useRef(false);
  const [pending, setPending] = useState(false);
  const [error, setError] = useState("");
  const [rejected, setRejected] = useState(false);
  const clear = () => {
    request.current = null;
    requestBackend.current = null;
    committed.current = false;
    setError("");
    setRejected(false);
  };
  const submit = async () => {
    if (running.current || !request.current) return;
    running.current = true;
    setPending(true);
    setError("");
    try {
      if (!committed.current) {
        const backend = requestBackend.current;
        if (!backend) throw new Error("The team server is unavailable.");
        await backend.post("/api/peer-teams", request.current, {
          timeoutMs: 15000,
        });
        committed.current = true;
      }
      await refresh?.();
      clear();
      notify?.("Team membership updated.");
    } catch (e) {
      setError(errorText(e));
      setRejected(
        !committed.current &&
          e instanceof ApiError &&
          e.status >= 400 &&
          e.status < 500,
      );
    } finally {
      running.current = false;
      setPending(false);
    }
  };
  const move = (project: Project, member: string, teamId: string | null) => {
    if (request.current) return;
    if (typeof project.path !== "string") return;
    requestBackend.current =
      ownerForProject?.(project.path, member, teamId) || defaultBackend;
    request.current = {
      action: "move",
      path: project.path,
      member,
      team_id: teamId,
      expected_revision: project.peerTeamsRevision || 0,
      request_id: randomUUID(),
    };
    void submit();
  };
  const reload = async () => {
    if (running.current) return;
    running.current = true;
    setPending(true);
    try {
      await refresh?.();
      clear();
    } catch (e) {
      setError(errorText(e));
    } finally {
      running.current = false;
      setPending(false);
    }
  };
  return {
    move,
    blocked: () => !!request.current,
    feedback:
      pending || error ? (
        <div className="peer-team-form">
          {pending ? (
            <p role="status">Updating team…</p>
          ) : (
            <>
              <p role="alert">
                {error}{" "}
                {committed.current
                  ? "The change was saved. Retry to refresh."
                  : rejected
                    ? "Reload the project before another move."
                    : "The result is unknown. Retry the same move."}
              </p>
              <Button
                size="compact-xs"
                onClick={() => void (rejected ? reload() : submit())}
              >
                {rejected ? "Reload project" : "Retry move"}
              </Button>
            </>
          )}
        </div>
      ) : null,
  };
}
