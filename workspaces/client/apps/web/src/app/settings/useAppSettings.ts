import { makeAutoObservable } from "mobx";
import type { ServerSettings } from "../../servers/ServerSettingsContext.tsx";
import { isStudioSettingsTab } from "../../components/settings/StudioSettingsTabs/StudioSettingsTabs.tsx";
import type { StudioSettingsTab } from "../../components/settings/StudioSettingsLayout/StudioSettingsLayout.tsx";
import type { AppScope } from "../contracts.ts";
import type { ServerCommand } from "../../servers/navigation.ts";

export class SettingsUi {
  chatOpen = false;
  accountModalOpen = false;
  externalAccount = false;
  settingsOpen = false;
  settingsTab: StudioSettingsTab = "accounts";
  command: Extract<
    ServerCommand,
    { action: "account-sign-in" | "account-action" }
  > | null = null;
  #screen: AppScope["screen"];
  #shell: ServerSettings;
  get source(): ServerSettings {
    return this.#screen.channel ? this : this.#shell;
  }
  constructor(screen: AppScope["screen"], source: ServerSettings | null) {
    this.#screen = screen;
    this.#shell = source || this;
    makeAutoObservable(this, {}, { autoBind: true });
  }
  get studioOpen() {
    return this.source.settingsOpen;
  }
  get tab() {
    return this.source.settingsTab;
  }
  setSettingsOpen(open: boolean) {
    this.settingsOpen = open;
  }
  setSettingsTab(tab: StudioSettingsTab) {
    this.settingsTab = tab;
  }
  setChatOpen(open: boolean) {
    this.chatOpen = open;
  }
  setCommand(command: SettingsUi["command"]) {
    this.command = command;
  }
  setStudioOpen(open: boolean) {
    this.source.setSettingsOpen(open);
  }
  openSettings(next: StudioSettingsTab = "accounts") {
    this.source.setSettingsTab(next);
    if (
      this.#screen.channel &&
      !this.externalAccount &&
      (next === "accounts" || next === "servers")
    ) {
      this.setStudioOpen(false);
      this.#screen.channel.publish({
        kind: "studio-server-open-settings",
        tab: next,
      });
    } else this.setStudioOpen(true);
  }
  activateTab(next: string | null) {
    if (isStudioSettingsTab(next)) {
      if (this.studioOpen) this.openSettings(next);
      else this.source.setSettingsTab(next);
    }
  }
  reportAccountDialog(opened: boolean) {
    if (this.accountModalOpen === opened) return;
    this.accountModalOpen = opened;
    this.#screen.channel?.publish({
      kind: "studio-server-account-dialog",
      opened,
    });
    if (!opened && this.externalAccount) {
      this.externalAccount = false;
      this.setStudioOpen(false);
    }
  }
  closeChatSettings() {
    if (!this.accountModalOpen) this.chatOpen = false;
  }
  run(next: ServerCommand) {
    if (next.action === "settings") this.openSettings(next.tab);
    else if (
      next.action === "account-sign-in" ||
      next.action === "account-action"
    ) {
      this.externalAccount = true;
      this.openSettings("accounts");
      this.command = next;
    } else return false;
    return true;
  }
}
