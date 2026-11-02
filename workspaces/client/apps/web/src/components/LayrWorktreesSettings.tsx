import { useEffect, useState } from "react";
import { errorText, type GetResult } from "../api";
import { useServerApi } from "../servers/ServerServicesContext";
import { SettingsRow } from "./ui/primitives";

type Configuration = GetResult<"/api/native-layr/settings">;

export function LayrWorktreesSettings({ active }: { active: boolean }) {
  const { get } = useServerApi();
  const [configuration, setConfiguration] = useState<Configuration | null>(
    null,
  );
  const [error, setError] = useState("");
  useEffect(() => {
    if (!active) return;
    let cancelled = false;
    void get("/api/native-layr/settings").then(
      (value) => {
        if (!cancelled) setConfiguration(value);
      },
      (failure) => {
        if (!cancelled) setError(errorText(failure));
      },
    );
    return () => {
      cancelled = true;
    };
  }, [active]);
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
            <SettingsRow label="Store subvolume">
              {configuration.storeRoot ?? configuration.error}
            </SettingsRow>
            {configuration.error && configuration.storeRoot && (
              <p role="alert">{configuration.error}</p>
            )}
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
