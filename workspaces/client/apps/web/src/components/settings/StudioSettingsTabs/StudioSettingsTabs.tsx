import { studioSettingsTabs } from "../support/StudioSettingsTabs.tsx";
import type { StudioSettingsTab } from "../StudioSettingsLayout/StudioSettingsLayout.tsx";
import { Tabs } from "@mantine/core";

export default function StudioSettingsTabs({
  serversOnly = false,
  onSelect,
}: {
  serversOnly?: boolean;
  onSelect?: () => void;
}) {
  return (
    <Tabs.List aria-label="Studio settings">
      {studioSettingsTabs.map(([id, label]) => (
        <Tabs.Tab
          key={id}
          value={id}
          disabled={serversOnly && id !== "servers"}
          onClick={onSelect}
        >
          {label}
        </Tabs.Tab>
      ))}
    </Tabs.List>
  );
}

export function isStudioSettingsTab(
  value: unknown,
): value is StudioSettingsTab {
  return studioSettingsTabs.some(([id]) => id === value);
}
