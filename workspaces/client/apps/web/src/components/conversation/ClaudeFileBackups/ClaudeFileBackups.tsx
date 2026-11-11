import { Button } from "@mantine/core";
import { observer } from "mobx-react-lite";
import { ClaudeFileBackupsStore } from "./ClaudeFileBackupsStore.ts";
export { ClaudeFileBackupsStore } from "./ClaudeFileBackupsStore.ts";

export default observer(function ClaudeFileBackups({
  store,
  onReleased,
}: {
  store: ClaudeFileBackupsStore;
  onReleased: () => void;
}) {
  const { usage, busy, message } = store;
  return (
    <div>
      {usage && (
        <p>
          Backup usage: {usage.count} of {usage.maxCount} copies, {usage.bytes}{" "}
          of {usage.maxBytes} bytes.
        </p>
      )}
      <p>
        Release deletes the saved original files from completed restores in this
        chat. Unresolved restore backups stay saved.
      </p>
      <Button
        disabled={busy || !usage}
        onClick={() => void store.release(onReleased)}
      >
        Release completed backups
      </Button>
      {message && <p role="status">{message}</p>}
    </div>
  );
});
