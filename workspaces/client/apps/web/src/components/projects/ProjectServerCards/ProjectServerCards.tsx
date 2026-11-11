import { Button, Text, SegmentedControl } from "@mantine/core";
import { useState } from "react";
import { observer } from "mobx-react-lite";
import type { ProjectServerSelection } from "../../../app/projects/CreationOwner.ts";
import {
  projectLocationOn,
  type LocationProjectView as LocationProject,
  type ProjectServerChoice,
  type ProjectLocation,
} from "../../../servers/projectLocations.ts";

import NewChatModelSettings from "../../settings/NewChatModelSettings/NewChatModelSettings.tsx";

import {
  projectAllowsWorktree,
  type NewChatSettings,
} from "../../../app/projects/newChatSettings.ts";

type Props = {
  project: LocationProject;
  servers: ProjectServerChoice[];
  owner: ProjectServerSelection;
  onChoose: (location: ProjectLocation, settings: NewChatSettings) => void;
  onAdd: (server: string) => void;
  onCancel: () => void;
};

export default observer(function ProjectServerCards(props: Props) {
  return <ProjectServerSelectionView {...props} key={props.owner.key} />;
});

const ProjectServerSelectionView = observer(
  function ProjectServerSelectionView({
    project,
    servers,
    owner,
    onChoose,
    onAdd,
    onCancel,
  }: Props) {
    const { selected, read } = owner;
    const server = servers.find((row) => row.id === selected);
    const location = server && projectLocationOn(project, server);
    const knownSystem =
      !!server?.system &&
      server.nativeLayr !== undefined &&
      server.defaultWorkspaceMode !== undefined;
    const fetched =
      server && !server.disabled && !knownSystem ? read.value : undefined;
    const system = server?.system ?? fetched?.system;
    const nativeLayr = server?.nativeLayr ?? fetched?.nativeLayr;
    const nativeAvailable =
      !!nativeLayr && nativeLayr.status !== "notInstalled";
    const nativeReason =
      server?.nativeLayrMessage ?? fetched?.nativeLayrMessage;
    const reportedMode =
      server?.defaultWorkspaceMode ?? fetched?.defaultWorkspaceMode;
    const supportedDefaultMode =
      reportedMode === "layr-native" || reportedMode === "worktree"
        ? reportedMode
        : reportedMode
          ? "worktree"
          : undefined;
    const [chosenMode, setModeChoice] =
      useState<NewChatSettings["workspaceMode"]>();
    const mode =
      supportedDefaultMode &&
      (chosenMode && (chosenMode !== "layr-native" || nativeAvailable)
        ? chosenMode
        : supportedDefaultMode);
    const worktreeAllowed = projectAllowsWorktree(project);
    const models = owner.model?.selection;
    const platformError = knownSystem || server?.disabled ? "" : read.error;
    return (
      <div
        className="project-server-choice"
        onKeyDown={(event) => {
          if (event.key === "Escape") {
            event.preventDefault();
            event.stopPropagation();
            onCancel();
          }
        }}
      >
        <div
          className="project-server-cards"
          role="group"
          aria-label="Chat server"
        >
          {servers.map((server) => {
            const folder = projectLocationOn(project, server);
            return (
              <div key={server.id} className="project-server-card">
                <button
                  type="button"
                  aria-pressed={selected === server.id}
                  disabled={server.disabled || !folder}
                  onClick={() => owner.select(server.id)}
                >
                  <strong>{server.label}</strong>
                  <Text component="span" size="xs" c="dimmed">
                    {server.disabled ? "Offline" : "Active"}
                  </Text>
                  {folder && (
                    <span className="project-server-path">{folder.path}</span>
                  )}
                </button>
                {!folder && (
                  <Button
                    variant="subtle"
                    size="xs"
                    disabled={!!server.disabled}
                    onClick={() => onAdd(server.id)}
                  >
                    + Add folder on this server
                  </Button>
                )}
              </div>
            );
          })}
        </div>
        {system && (
          <div className="new-chat-run-mode">
            <Text component="label" id="new-chat-run-mode-label" size="sm">
              Workspace
            </Text>
            <SegmentedControl
              fullWidth
              aria-labelledby="new-chat-run-mode-label"
              {...(mode ? { value: mode } : {})}
              onChange={(value) =>
                setModeChoice(
                  value === "layr-native" ? "layr-native" : "worktree",
                )
              }
              data={[
                {
                  value: "layr-native",
                  label: (
                    <span
                      className={
                        nativeLayr?.status !== "ready" && nativeReason
                          ? "native-layr-error"
                          : undefined
                      }
                      title={nativeReason}
                    >
                      Layr
                    </span>
                  ),
                  disabled: !nativeAvailable,
                },
                { value: "worktree", label: "WT", disabled: !worktreeAllowed },
              ]}
            />
            {!worktreeAllowed && (
              <Text size="xs" c="dimmed" role="status">
                This project allows only Layr workspaces.
              </Text>
            )}
          </div>
        )}
        {platformError && (
          <Text role="alert" c="red">
            {platformError}
          </Text>
        )}
        {owner.model && <NewChatModelSettings model={owner.model} />}
        <div className="project-server-actions">
          <Button onClick={onCancel}>Cancel</Button>
          <Button
            variant="filled"
            disabled={
              !location ||
              !models ||
              !system ||
              (mode === "layr-native" && !nativeAvailable) ||
              (mode === "worktree" && !worktreeAllowed) ||
              !supportedDefaultMode ||
              !!platformError ||
              !!servers.find((row) => row.id === selected)?.disabled
            }
            onClick={() =>
              location &&
              mode &&
              models &&
              onChoose(
                { ...location, serverId: selected },
                {
                  ...models,
                  workspaceMode: mode,
                },
              )
            }
          >
            Start chat
          </Button>
        </div>
      </div>
    );
  },
);

export { ProjectServerRead } from "./ProjectServerRead.ts";
