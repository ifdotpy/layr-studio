import { Button, Modal } from "@mantine/core";
import { observer } from "mobx-react-lite";
import { useServerAreas } from "../../../app/ServerAreasProvider.tsx";

export const StartupStatus = observer(function StartupStatus() {
  const { startup } = useServerAreas();
  return (
    <main
      className="startup"
      aria-label="Studio startup"
      data-connecting={!startup.error || undefined}
    >
      <p
        role={startup.error ? "alert" : "status"}
        className={startup.error ? "studio-recovery-banner" : undefined}
      >
        {startup.error || "Connecting to LAYR Studio…"}
      </p>
      {startup.error && <a href="/">Reload Studio</a>}
    </main>
  );
});
const SchemaUpdateContent = observer(function SchemaUpdateContent() {
  const { startup } = useServerAreas();
  return (
    <>
      <p>
        {startup.schemaMismatchNeedsRebuild
          ? "The installed renderer build does not match the server. Rebuild Studio before continuing."
          : "Studio has been updated. Update this tab to continue syncing and sending."}
      </p>
      {!startup.schemaMismatchNeedsRebuild && (
        <>
          <Button onClick={startup.retryUpdate}>Update</Button>
          {startup.schemaUpdateError && (
            <p role="alert">{startup.schemaUpdateError}</p>
          )}
        </>
      )}
    </>
  );
});
export const SchemaUpdateDialog = observer(function SchemaUpdateDialog() {
  const { startup } = useServerAreas();
  return (
    <Modal
      opened={startup.isSchemaGateVisible}
      onClose={() => {}}
      title="Studio update required"
      closeOnEscape={false}
      closeOnClickOutside={false}
      withCloseButton={false}
      role="alertdialog"
      aria-label="Studio update required"
    >
      <SchemaUpdateContent />
    </Modal>
  );
});
export const ServerUpdateStatus = observer(function ServerUpdateStatus() {
  const { scope } = useServerAreas();
  return (
    <main className="startup" aria-label="Server update required">
      <h2>{scope.services.owner.alias || "Server"}: update required</h2>
      <SchemaUpdateContent />
    </main>
  );
});
