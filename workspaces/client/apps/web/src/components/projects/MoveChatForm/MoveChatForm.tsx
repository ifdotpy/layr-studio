import {
  type Project,
  folderLabel,
} from "../../../app/projects/ProjectOrganization.tsx";
import { Button, NativeSelect } from "@mantine/core";
import { useId, useRef, useState } from "react";
import { useProjectSave } from "../support/useProjectSave.ts";
import type { Agent } from "../../../types.ts";
import { SettingsRow } from "../../ui/SettingsRow/SettingsRow.tsx";

export function MoveChatForm({
  project,
  agent,
  saved,
}: {
  project: Project;
  agent: Pick<Agent, "id" | "projectFolder" | "projectFolderRevision">;
  saved: () => Promise<void>;
}) {
  const [folder, setFolder] = useState(agent.projectFolder || "");
  const folderId = useId();
  const save = useProjectSave("/api/organization", saved, (result) => {
    if ((result.projectFolder || "") !== folder)
      throw new Error("Restart Studio to use project folders.");
  });
  const expected = useRef({
    folder: agent.projectFolder || null,
    revision: agent.projectFolderRevision || 0,
  });
  return (
    <form
      onSubmit={(event) => {
        event.preventDefault();
        if (typeof project.path !== "string") return;
        void save.submit({
          id: agent.id,
          project_path: project.path,
          project_folder: folder || null,
          expected_folder: expected.current.folder,
          expected_revision: expected.current.revision,
        });
      }}
    >
      <SettingsRow label={<label htmlFor={folderId}>Move to folder</label>}>
        <NativeSelect
          id={folderId}
          value={folder}
          disabled={save.pending || save.frozen}
          onChange={(event) => setFolder(event.currentTarget.value)}
          data={[
            { value: "", label: project.name || project.path || "Project" },
            ...(project.folders || [])
              .map((item) => ({
                value: item.id,
                label: folderLabel(project.folders || [], item.id),
              }))
              .sort((a, b) => a.label.localeCompare(b.label)),
          ]}
        />
      </SettingsRow>
      {save.error && <p role="alert">{save.error}</p>}
      <Button type="submit" mt="md" loading={save.pending}>
        {save.retryLabel || "Move chat"}
      </Button>
    </form>
  );
}
