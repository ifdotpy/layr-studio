import { observer } from "mobx-react-lite";
import { resourceData } from "../../../store/runtime.ts";
import {
  useServerApi,
  useServerStore,
} from "../../servers/ServerServicesProvider/ServerServicesProvider.tsx";
import { SettingsRow } from "../../ui/SettingsRow/SettingsRow.tsx";
import { LayrStoreDirectory } from "../../projects/LayrStoreDirectory/LayrStoreDirectory.tsx";

export const LayrWorktreesSettings = observer(function LayrWorktreesSettings({
  active,
}: {
  active: boolean;
}) {
  const { post } = useServerApi();
  const store = useServerStore();
  const configuration = active
    ? resourceData(store.nativeLayrSettings().current)
    : undefined;
  if (!active) return null;
  return (
    <>
      <section className="settings-group" aria-label="Layr">
        <h2>Layr</h2>
        {configuration && (
          <>
            <SettingsRow label="Installed version">
              {configuration.version
                ? `${configuration.version.version} (${configuration.version.build})`
                : configuration.error
                  ? "Unavailable"
                  : "Not installed"}
            </SettingsRow>
            <LayrStoreDirectory
              label="Default store directory"
              path={configuration.storeRoot}
              source={configuration.configured ? "default" : "built-in"}
              overridden={configuration.configured}
              save={async (storeRoot) => {
                await post("/api/native-layr/settings", { storeRoot });
              }}
            />
            {configuration.readiness?.status === "notReady" && (
              <p role="status" className="layr-settings-detail">
                {configuration.readiness.reason}
              </p>
            )}
            {configuration.error && (
              <p role="alert" className="layr-settings-detail">
                {configuration.error}
              </p>
            )}
          </>
        )}
      </section>
    </>
  );
});
