import { WorkspaceResourceState as ResourceState } from "../WorkspaceResourceState/WorkspaceResourceState.tsx";
import {
  date,
  type PanelContext,
} from "../WorkspaceSection/WorkspaceSection.tsx";
import { Button, Modal, TextInput } from "@mantine/core";
import { action } from "mobx";
import { observer } from "mobx-react-lite";
import {
  section,
  WorkspaceSection as Empty,
} from "../WorkspaceSection/WorkspaceSection.tsx";

export const WorkspaceCheckpoints = section(
  observer(function WorkspaceCheckpoints(c: PanelContext<never>) {
    const state = c.model.checkpointState;
    const ui = c.panel;
    const { label, preview, busy } = ui;

    const canRewind = c.model.canRewind;
    return (
      <>
        <ResourceState state={state} />
        <p className="workspace-muted">
          Save the project files and chat history. Preview changes before you
          restore a checkpoint.
        </p>
        <form
          className="workspace-toolbar"
          onSubmit={(e) => {
            e.preventDefault();
            void ui.saveCheckpoint(c);
          }}
        >
          <TextInput
            className="workspace-grow"
            aria-label="Checkpoint name"
            placeholder="Checkpoint name (optional)"
            value={label}
            onChange={action((e) => (ui.label = e.target.value))}
          />
          <Button
            variant="filled"
            type="submit"
            loading={busy}
            disabled={!!c.selected?.inFlight}
          >
            Save checkpoint
          </Button>
        </form>
        {(state.data?.checkpoints || []).map((checkpoint) => (
          <div className="workspace-row" key={checkpoint.id}>
            <div className="workspace-row-head">
              <strong>{checkpoint.label}</strong>
              {canRewind && (
                <Button
                  size="compact-xs"
                  variant="light"
                  disabled={busy}
                  onClick={() => void ui.previewCheckpoint(c, checkpoint)}
                >
                  Preview restore
                </Button>
              )}
            </div>
            <small>
              {date(checkpoint.created)} · {checkpoint.commit?.slice(0, 12)}
            </small>
          </div>
        ))}
        {!canRewind && (
          <p className="workspace-muted">
            Restore is not available for this chat. It needs an isolated project
            folder.
          </p>
        )}
        {!state.data?.checkpoints?.length && state.data !== null && (
          <Empty>No checkpoints yet.</Empty>
        )}
        <Modal
          opened={!!preview}
          onClose={action(() => {
            if (!ui.busy) ui.preview = null;
          })}
          title={`Restore: ${preview?.label || "checkpoint"}`}
          size="xl"
        >
          {preview && (
            <>
              <p>Restores files and chat state. The agent stays stopped.</p>
              <pre className="workspace-code workspace-diff-preview">
                {preview.diff || "No file changes."}
              </pre>
              {!preview.canRestore && (
                <p className="workspace-muted">
                  {preview.restoreUnavailableReason ||
                    "A restore requires an idle agent with an isolated worktree."}
                </p>
              )}
              <Button
                variant="filled"
                color="orange"
                disabled={!preview.canRestore}
                loading={busy}
                onClick={() => void ui.restoreCheckpoint(c)}
              >
                Restore this checkpoint
              </Button>
            </>
          )}
        </Modal>
      </>
    );
  }),
);
