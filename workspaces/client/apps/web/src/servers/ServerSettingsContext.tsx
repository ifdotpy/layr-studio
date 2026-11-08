import { createContext, useContext, type ReactNode } from "react";
import type { StudioServer } from "./registry.ts";
import type { StudioSettingsTab } from "../components/StudioSettingsTabs.tsx";
export interface ServerSettings {
  readonly server?: StudioServer;
  readonly settingsOpen: boolean;
  readonly settingsTab: StudioSettingsTab;
  setSettingsOpen(opened: boolean): void;
  setSettingsTab(tab: StudioSettingsTab): void;
}
export const ServerSettingsContext = createContext<ServerSettings | null>(null);
/** A render slot stays in React, separate from the settings state owner. */
export const ServerManagementPanelContext = createContext<ReactNode>(null);
export const useServerSettings = () => useContext(ServerSettingsContext);
export const useServerManagementPanel = () =>
  useContext(ServerManagementPanelContext);
