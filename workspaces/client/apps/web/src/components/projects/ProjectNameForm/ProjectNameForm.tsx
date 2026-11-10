import {
  type Project,
  type ProjectFolder,
  folderLabel,
} from "../../../app/projects/ProjectOrganization.tsx";
import { requestId as randomUUID } from "studio-claude-bridge/request-id.ts";
import { observer } from "mobx-react-lite";
import { Button, TextInput } from "@mantine/core";
import { useId, useRef, useState } from "react";
import { useProjectSave } from "../support/useProjectSave.ts";
import { SettingsRow } from "../../ui/SettingsRow/SettingsRow.tsx";

export const ProjectNameForm = observer(function ProjectNameForm({
  project,
  folder,
  parentId,
  saved,
  displayPath,
}: {
  project: Project;
  folder?: ProjectFolder | "new" | undefined;
  parentId?: string | undefined;
  saved: () => Promise<void>;
  displayPath?: string | undefined;
}) {
  const [name, setName] = useState(
    folder === "new" ? "" : folder?.name || project.name || "",
  );
  const save = useProjectSave("/api/projects", saved);
  const id = useRef(folder === "new" ? randomUUID() : folder?.id);
  const revision = useRef(project.organizationRevision || 0);
  const nameId = useId();
  return (
    <form
      onSubmit={(event) => {
        event.preventDefault();
        if (typeof project.path !== "string") return;
        void save.submit({
          action:
            folder === "new"
              ? "add_folder"
              : folder
                ? "rename_folder"
                : "rename",
          path: project.path,
          name: name.trim(),
          folder_id: id.current,
          parent_id: parentId || null,
          expected_revision: revision.current,
        });
      }}
    >
      <p className="project-directory">{displayPath || project.path}</p>
      {parentId && <p>In {folderLabel(project.folders || [], parentId)}</p>}
      <SettingsRow
        label={
          <label htmlFor={nameId}>
            {folder ? "Folder name" : "Project name"}
          </label>
        }
      >
        <TextInput
          id={nameId}
          value={name}
          autoFocus
          maxLength={255}
          required
          disabled={save.pending || save.frozen}
          onChange={(event) => {
            setName(event.currentTarget.value);
          }}
        />
      </SettingsRow>
      {save.error && <p role="alert">{save.error}</p>}
      <Button
        type="submit"
        mt="md"
        loading={save.pending}
        disabled={!name.trim()}
      >
        {save.retryLabel || (folder === "new" ? "Create folder" : "Save name")}
      </Button>
    </form>
  );
});
