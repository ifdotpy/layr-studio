import { Switch } from "@mantine/core";
import { useEffect, useState } from "react";
import { errorText, type GetResult } from "../api";
import {
  useServerApi,
  useServerResources,
} from "../servers/ServerServicesContext";
import { SettingsRow } from "./ui/primitives";
import { LayrStoreDirectory } from "./LayrStoreDirectory";

type Configuration = GetResult<"/api/native-layr/settings">;

export function LayrWorktreesSettings({ active }: { active: boolean }) {
  const { post } = useServerApi();
  const resources = useServerResources();
  const [configuration, setConfiguration] = useState<Configuration | null>(
    null,
  );
  const [virtualLinesError, setVirtualLinesError] = useState("");
  useEffect(() => {
    if (!active) return;
    return resources.watchResourceChanges(
      { kind: "native-layr-settings" },
      (version) => {
        if (version?.nativeLayrSettings) {
          setConfiguration(version.nativeLayrSettings);
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
            <LayrStoreDirectory
              label="Default store directory"
              path={configuration.storeRoot}
              source={configuration.configured ? "default" : "built-in"}
              overridden={configuration.configured}
              save={async (storeRoot) => {
                await post("/api/native-layr/settings", { storeRoot });
              }}
            />
            {(configuration.virtualAgentLinesAvailable ||
              configuration.virtualAgentLines) && (
              <SettingsRow
                label="Virtual agent lines (experimental)"
                help="New worker agents get a virtual Layr line: Layr stores only the files that the agent changes, so a new line is ready at once. Leads and reviewers keep folder lines. A virtual line has no Git folder and no build caches."
              >
                <Switch
                  aria-label="Virtual agent lines"
                  checked={configuration.virtualAgentLines}
                  error={virtualLinesError || undefined}
                  onChange={(event) => {
                    setVirtualLinesError("");
                    post("/api/native-layr/settings", {
                      virtualAgentLines: event.currentTarget.checked,
                    }).catch((failure: unknown) =>
                      setVirtualLinesError(errorText(failure)),
                    );
                  }}
                />
              </SettingsRow>
            )}
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
}
