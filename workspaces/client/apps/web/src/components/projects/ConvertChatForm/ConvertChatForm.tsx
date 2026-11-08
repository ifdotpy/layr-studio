import type { Project } from "../../../app/projects/ProjectOrganization.tsx";
import { requestId as randomUUID } from "studio-claude-bridge/request-id.ts";
import { Button } from "@mantine/core";
import { useRef } from "react";
import { useProjectSave } from "../support/useProjectSave.ts";
import type { Agent } from "../../../types.ts";

export function ConvertChatForm({
  project,
  source,
  target,
  scope,
  saved,
}: {
  project: Project;
  source: Pick<Agent, "id" | "name">;
  target: Pick<Agent, "id" | "name">;
  scope: string;
  saved: () => Promise<void>;
}) {
  const requestId = useRef(randomUUID());
  const save = useProjectSave(
    "/api/peer-teams",
    saved,
    undefined,
    `${scope}:${source.id}:${target.id}`,
  );
  return (
    <form
      onSubmit={(event) => {
        event.preventDefault();
        if (typeof project.path !== "string") return;
        void save.submit({
          action: "convert",
          path: project.path,
          member: source.id,
          target: target.id,
          expected_revision: project.peerTeamsRevision || 0,
          request_id: requestId.current,
        });
      }}
    >
      <p>
        Make <strong>{source.name}</strong> a subagent of{" "}
        <strong>{target.name}</strong>?
      </p>
      <ul>
        <li>
          The chat keeps its history, thread, account, model, and directory.
        </li>
        <li>Its workers and task board move to {target.name}.</li>
        <li>
          It leaves its peer team. A team with fewer than two chats dissolves.
        </li>
        <li>
          Saved peer messages stay available to you. Former peers lose agent
          access.
        </li>
        <li>Its previous broadcast history stays with its original workers.</li>
        <li>The chat pauses. {target.name} becomes its lead.</li>
      </ul>
      <p>
        Both agent trees must be idle. Active commands, pending input, workspace
        operations, and account transfers block the move.
      </p>
      {save.error && <p role="alert">{save.error}</p>}
      <Button type="submit" loading={save.pending}>
        {save.retryLabel || "Make subagent"}
      </Button>
    </form>
  );
}
