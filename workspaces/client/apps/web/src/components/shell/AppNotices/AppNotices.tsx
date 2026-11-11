import { Button } from "@mantine/core";
import { observer } from "mobx-react-lite";
import { useServerAreas } from "../../../app/ServerAreasProvider.tsx";

export const AppNotices = observer(function AppNotices() {
  const { startup, delivery, native, selected, navigation, creation } =
    useServerAreas();
  const agent = navigation.selectedAgent;
  const pending = agent && native.pending(agent.id);
  const error = !navigation.selectedId && startup.error;
  return (
    <>
      {agent && pending && (
        <div
          role="status"
          className="native-action-pending"
          style={{ padding: "8px 16px" }}
        >
          <span>The {pending.action} request has no confirmed reply. </span>
          <Button
            size="compact-sm"
            loading={native.active}
            onClick={() => selected.nativeAction(agent, pending.action)}
          >
            Check action request
          </Button>
        </div>
      )}
      {!creation.isCreating &&
        creation.pendingCreationRequests.map((request) => (
          <div className="sync-status" key={request.id}>
            <p>
              Studio lost confirmation for this chat request in{" "}
              {request.cwd || "default project"}
              {request.project_folder ? ` (${request.project_folder})` : ""}.
              Retrying uses the saved request ID to check whether the chat was
              created. Dismissing stops checking; a chat this request created
              still appears in the chat list.
            </p>
            <Button onClick={() => void creation.retry(request)}>
              Retry chat request
            </Button>
            <Button onClick={() => creation.dismiss(request)}>Dismiss</Button>
          </div>
        ))}
      {error && !delivery.ownsOfflineError(error) && (
        <div id="error" role="alert">
          {error}
        </div>
      )}
      <div className="sync-notices">
        {delivery.draftSyncError && (
          <p className="sync-status" role="status" data-draft-sync-status>
            {delivery.draftSyncError}
          </p>
        )}
        {delivery.outboxSyncError && (
          <p className="sync-status" role="alert">
            {delivery.outboxSyncError}
          </p>
        )}
      </div>
    </>
  );
});
