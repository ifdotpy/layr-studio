import { useEffect, useState } from "react";
import { errorText, type GetResult } from "../api";
import {
  useServerApi,
  useServerResources,
} from "../servers/ServerServicesContext";
import { Button, Group } from "@mantine/core";
import DirectoryField from "./DirectoryField";
import { SettingsRow } from "./ui/primitives";

type Configuration = GetResult<"/api/native-layr/settings">;

export function LayrWorktreesSettings({ active }: { active: boolean }) {
  const { post } = useServerApi();
  const resources = useServerResources();
  const [storeRoot, setStoreRoot] = useState("");
  const [busy, setBusy] = useState(false);
  const [configuration, setConfiguration] = useState<Configuration | null>(
    null,
  );
  const [error, setError] = useState("");
  useEffect(() => {
    if (!active) return;
    return resources.watchResourceChanges(
      { kind: "native-layr-settings" },
      (version) => {
        if (version?.nativeLayrSettings) {
          setConfiguration(version.nativeLayrSettings);
          setStoreRoot(version.nativeLayrSettings.storeRoot ?? "");
        }
      },
    );
  }, [active, resources]);
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
            <DirectoryField
              id="layr-store-root"
              label="Default store directory"
              description="Leave the directory empty to use Studio's default store."
              value={storeRoot}
              onChange={setStoreRoot}
            />
            <Group justify="flex-end">
              <Button
                loading={busy}
                onClick={() => {
                  setBusy(true);
                  setError("");
                  void post("/api/native-layr/settings", {
                    storeRoot: storeRoot.trim(),
                  })
                    .then(
                      () => {},
                      (failure) => setError(errorText(failure)),
                    )
                    .finally(() => setBusy(false));
                }}
              >
                Save store directory
              </Button>
            </Group>
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
        {error && <p role="alert">{error}</p>}
      </section>
    </>
  );
}
