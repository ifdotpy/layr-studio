import { SidebarCommandsProvider } from "../../src/components/sidebar/commands.ts";
import { observableFixture } from "../../src/components/observableFixture.tsx";
const injectedStore = observableFixture();
import React, { memo, useLayoutEffect, useReducer } from "react";
import { createRoot } from "react-dom/client";
import { MantineProvider } from "@mantine/core";
import "@mantine/core/styles.css";
import "../../src/style.css";
import "../../src/appearance.css";
import Sidebar from "../../src/components/Sidebar.fixture.tsx";
import { createPilotModel } from "./sidebar-pilot-model.ts";
import { pilotAgents, pilotSize, pilotUpdate } from "./sidebar-pilot-data.ts";
import { required } from "./performance-fixture.ts";

const noop = () => {};
const counts: Record<string, number> = {};
const frame = () =>
  new Promise<void>((resolve) => requestAnimationFrame(() => resolve()));
const settled = async () => {
  await frame();
  await frame();
};
let agents = pilotAgents();
let model: ReturnType<typeof createPilotModel>;
let enabled = false;
// Deliberately a memoized sibling guard, not the actual Conversation component.
const ConversationGuard = memo(function ConversationGuard() {
  useLayoutEffect(() => window.sidebarPilot.count("conversationGuard"));
  return <main data-conversation-guard>Unchanged conversation</main>;
});
function Harness() {
  const [, notify] = useReducer((n: number) => n + 1, 0);
  const [value] = React.useState(() => createPilotModel(agents, notify));
  model = value;
  useLayoutEffect(() => {
    window.sidebarPilot.ready = true;
  }, []);
  return (
    <MantineProvider>
      <injectedStore.Provider>
        <SidebarCommandsProvider
          value={{
            open: noop,
            newChat: noop,
            addProject: noop,
            changeProject: noop,
            projectAccount: noop,
            rename: async () => {},
            remove: noop,
            onSearch: noop,
            markUnread: noop,
          }}
        >
          <Sidebar
            {...value.view()}
            opened={null}
            creating={false}
            mobile={false}
            close={noop}
            markingRead={new Set()}
            hideOldChatsThreshold={100}
          />
        </SidebarCommandsProvider>
      </injectedStore.Provider>
      <ConversationGuard />
    </MantineProvider>
  );
}
window.sidebarPilot = {
  ready: false,
  count(name) {
    if (enabled) counts[name] = (counts[name] || 0) + 1;
  },
  async validate() {
    await document.fonts.ready;
    await settled();
    const rendered = document.querySelectorAll("[data-chat]").length;
    if (agents.length !== pilotSize.chats || rendered !== pilotSize.visible)
      throw new Error(
        `Expected ${pilotSize.chats}/${pilotSize.visible}, got ${agents.length}/${rendered}`,
      );
    if (!document.querySelector("[data-conversation-guard]"))
      throw new Error("Missing conversation guard");
    return { chats: agents.length, visible: rendered };
  },
  async phase(replay) {
    for (const key of Object.keys(counts)) delete counts[key];
    enabled = true;
    const latencyMs: number[] = [];
    for (let i = 0; i < pilotSize.updates; i++) {
      if (!replay) agents = pilotUpdate(agents, i);
      const start = performance.now();
      model.apply(agents);
      await settled();
      latencyMs.push(performance.now() - start);
      const target = document.querySelector(
        `[data-chat="pilot-${i % pilotSize.visible}"]`,
      );
      if (
        !target?.textContent?.includes(
          required(agents[i % pilotSize.visible]).name || "",
        )
      )
        throw new Error("Update did not reach actual row");
      const expected = required(
        agents[i % pilotSize.visible],
      ).expectedIndicator;
      if (
        target
          .closest("[data-sidebar-item]")
          ?.getAttribute("data-studio-state") !== expected
      )
        throw new Error("Status did not reach actual row");
    }
    enabled = false;
    latencyMs.sort((a, b) => a - b);
    return {
      commits: {
        sidebar: 0,
        "sidebar-row": 0,
        conversationGuard: 0,
        ...counts,
      },
      samples: latencyMs.length,
      updateToDoubleRafP95Ms: required(
        latencyMs[Math.ceil(latencyMs.length * 0.95) - 1],
      ),
    };
  },
};
createRoot(required(document.getElementById("root"))).render(<Harness />);
export type SidebarPilot = typeof window.sidebarPilot;
declare global {
  interface Window {
    sidebarPilot: {
      ready: boolean;
      count: (name: string) => void;
      validate: () => Promise<{ chats: number; visible: number }>;
      phase: (replay: boolean) => Promise<{
        commits: Record<string, number>;
        samples: number;
        updateToDoubleRafP95Ms: number;
      }>;
    };
  }
}
