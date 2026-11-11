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

import { type NewChatSettings } from "../../../app/projects/newChatSettings.ts";

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
    const [chosenMode, setModeChoice] =
      useState<NewChatSettings["workspaceMode"]>();
    const {
      selected,
      system,
      nativeLayr,
      nativeAvailable,
      nativeReason,
      mode,
      worktreeAllowed,
      platformError,
      selection,
    } = owner.view(project, chosenMode);
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
            disabled={!selection}
            onClick={() =>
              selection && onChoose(selection.location, selection.settings)
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
