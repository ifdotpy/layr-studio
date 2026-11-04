import { useEffect, useState } from "react";
import { errorText, type GetResult } from "../api";
import {
  useServerApi,
  useServerResources,
} from "../servers/ServerServicesContext";
import { Button } from "@mantine/core";
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
            <SettingsRow
              label={
                <label htmlFor="layr-store-root">Default store directory</label>
              }
            >
              <input
                id="layr-store-root"
                value={storeRoot}
                onChange={(event) => setStoreRoot(event.currentTarget.value)}
              />
            </SettingsRow>
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
            <p>Leave the directory empty to use Studio's default store.</p>
            {configuration.readiness?.status === "notReady" && (
              <p role="status">{configuration.readiness.reason}</p>
            )}
            {configuration.error && <p role="alert">{configuration.error}</p>}
          </>
        )}
        {error && <p role="alert">{error}</p>}
      </section>
      <section className="settings-group" aria-label="Worktrees">
        <h2>Worktrees</h2>
      </section>
    </>
  );
}
