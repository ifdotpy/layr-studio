import { WorkspaceResourceState as ResourceState } from "../WorkspaceResourceState/WorkspaceResourceState.tsx";
import { type PanelContext } from "../WorkspaceSection/WorkspaceSection.tsx";
import { Button, Modal, TextInput } from "@mantine/core";
import { Plus } from "lucide-react";
import { action } from "mobx";
import { observer } from "mobx-react-lite";
import { ModelField } from "../../settings/models/ModelField/ModelField.tsx";
import { WorkerLaunchDialog } from "../WorkerLaunchDialog/WorkerLaunchDialog.tsx";
import { WorkspaceFields } from "../WorkspaceFields/WorkspaceFields.tsx";
import {
  profileDetailsFields,
  profileFields,
  type Profile,
} from "../../../app/shell/WorkspaceStore.ts";
import {
  section,
  WorkspaceSection as Empty,
} from "../WorkspaceSection/WorkspaceSection.tsx";

export const WorkspaceProfiles = section(
  observer(function WorkspaceProfiles(c: PanelContext<Profile>) {
    const state = c.model.profileState;
    const ui = c.panel;
    const form = c.model.profileDraft;
    const draft = form.value;
    const setDraft = form.update;
    const { busy, remove, launch } = ui;

    const { lead, catalog, profileModel } = c.model;
    const { profileModels, draftModel, validModel } = profileModel;
    return (
      <>
        <ResourceState state={state} />
        <div className="workspace-toolbar">
          <span className="workspace-muted">
            Apply profiles when you create workers.
          </span>
          <Button
            size="xs"
            variant="filled"
            leftSection={<Plus size={14} />}
            onClick={c.model.newProfile}
          >
            New profile
          </Button>
        </div>
        {(state.data?.profiles || []).map((profile) => (
          <div className="workspace-row" key={profile.id}>
            <div className="workspace-row-head">
              <strong>{profile.name}</strong>
              <div className="workspace-actions">
                <Button
                  size="compact-xs"
                  variant="light"
                  disabled={!lead || lead.autoWake === false}
                  onClick={() => c.model.beginProfileLaunch(ui, profile)}
                >
                  Start subagent
                </Button>
                <Button
                  size="compact-xs"
                  variant="subtle"
                  onClick={() => setDraft({ ...profile })}
                >
                  Edit
                </Button>
                <Button
                  size="compact-xs"
                  color="red"
                  variant="subtle"
                  onClick={action(() => (ui.remove = profile))}
                >
                  Delete
                </Button>
              </div>
            </div>
            <small>
              {profile.role} · {profile.model} · {profile.effort}
            </small>
            <p className="workspace-clamp">{profile.instructions}</p>
          </div>
        ))}
        {!state.data?.profiles?.length && state.data !== null && (
          <Empty>No saved worker profiles.</Empty>
        )}
        <Modal
          opened={!!draft}
          onClose={() => {
            if (!busy) setDraft(null);
          }}
          title={
            draft?.id && !draft.isNew
              ? "Edit subagent profile"
              : "New subagent profile"
          }
          size="lg"
        >
          {draft && (
            <form
              className="workspace-form"
              onSubmit={(e) => {
                e.preventDefault();
                if (validModel) void ui.saveProfile(c, draftModel);
              }}
            >
              <WorkspaceFields
                draft={draft}
                change={form.change}
                fields={profileFields}
              />
              {catalog.models.length ? (
                <ModelField
                  retainCurrent={!draft.isNew}
                  presentation="select"
                  label="Model"
                  value={draftModel}
                  options={profileModels}
                  onChange={(model) => setDraft({ ...draft, model })}
                />
              ) : (
                <TextInput
                  label="Model"
                  required
                  value={draftModel}
                  onChange={(e) =>
                    setDraft({ ...draft, model: e.target.value })
                  }
                />
              )}
              <WorkspaceFields
                draft={draft}
                change={form.change}
                fields={profileDetailsFields}
              />
              <Button
                variant="filled"
                type="submit"
                loading={busy}
                disabled={!validModel}
              >
                Save profile
              </Button>
            </form>
          )}
        </Modal>
        <Modal
          opened={!!remove}
          onClose={action(() => {
            if (!ui.busy) ui.remove = null;
          })}
          title="Delete profile"
        >
          <p>
            Delete “{remove?.name}”? Existing agents keep their current
            instructions.
          </p>
          <Button
            variant="filled"
            color="red"
            loading={busy}
            onClick={() => void ui.deleteProfile(c)}
          >
            Delete profile
          </Button>
        </Modal>
        {launch && lead && (
          <WorkerLaunchDialog
            key={launch.id}
            launch={launch}
            lead={lead}
            busy={busy}
            onClose={action(() => (ui.launch = null))}
            onStart={(body) => ui.launchProfile(c, body)}
          />
        )}
      </>
    );
  }),
);
