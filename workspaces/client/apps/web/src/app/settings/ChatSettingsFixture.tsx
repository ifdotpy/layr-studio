import type { ComponentSnapshot as Snapshot } from "../../components/conversation/ComposerAttachments/componentTestFixtures.ts";
import { settingsAreasFixture } from "./SettingsAreasFixture.ts";
import { useEffect, useState } from "react";
import type { Agent } from "../../types.ts";
import type { AccountsState } from "../../components/accounts/support/Accounts.tsx";
import { ChatSettingsDialog } from "../../components/agents/ChatSettingsDialog/ChatSettingsDialog.tsx";
import { useServerStore } from "../../components/servers/ServerServicesProvider/ServerServicesProvider.tsx";
import { createServerServices } from "../../servers/serverServices.ts";
import { resourceData } from "../../store/runtime.ts";

/** Test-only adapter for existing controlled generated-model witnesses. */
export function ChatSettingsFixture({
  agent,
  lead = agent,
  settingsOpen,
  accountModalOpen,
  accounts,
  setSettingsOpen,
  setClaudeLoginKey,
}: {
  agent: Agent;
  lead?: Agent;
  settingsOpen: boolean;
  accountModalOpen: boolean;
  setSettingsOpen: (value: boolean) => void;
  accounts: { data: AccountsState };
  notify: (message: string) => unknown;
  agentModels: unknown;
  data: Snapshot;
  refresh: () => Promise<void>;
  workspaceId: string;
  creating: boolean;
  newChat: () => unknown;
  project: () => void;
  setClaudeLoginKey: (key: string) => void;
}) {
  const store = useServerStore();
  const [fixture] = useState(() => {
    const services = createServerServices({
      id: "fixture",
      origin: "http://fixture.test",
      label: "Fixture",
    });
    const scope = { store, services, screen: { active: true, channel: null } };
    const areas = settingsAreasFixture({
      scope,
      navigation: {
        get selectedAgent() {
          return resourceData(store.state().current)?.agents.get(agent.id);
        },
        get selectedLead() {
          return resourceData(store.state().current)?.agents.get(lead.id);
        },
      },
      startup: { workspaceId: "fixture" },
      layout: { isSettingsFullscreen: false },
      refresh: async () => {},
      creation: { isCreating: false, newChat() {} },
      dialogs: { changeProjectFolder() {} },
    });
    return { areas, close: services.close };
  });
  useEffect(() => () => fixture.close(), [fixture]);
  return (
    <ChatSettingsDialog
      areas={fixture.areas}
      accounts={
        accounts as import("../../components/accounts/Accounts/Accounts.tsx").AccountsController
      }
      settings={{
        chatOpen: settingsOpen,
        accountModalOpen,
        closeChatSettings: () => setSettingsOpen(false),
      }}
      onSignIn={setClaudeLoginKey}
    />
  );
}
