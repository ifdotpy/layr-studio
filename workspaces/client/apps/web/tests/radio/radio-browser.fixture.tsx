import { observable, action } from "mobx";
import "../../src/rendererStyles.ts";
import type { ComponentSnapshot as Snapshot } from "../../src/components/conversation/ComposerAttachments/componentTestFixtures.ts";
import "@mantine/core/styles.css";
declare global {
  interface Window {
    radio: () => NonNullable<Room["radio"]>;
    setRoom: React.Dispatch<React.SetStateAction<Room>>;
    currentRoom: Room;
    // opened is SessionActivityItem[] in tests/chat/session-activity-browser.fixture.tsx.
    radioOpened: string | null;
    refreshCalls: number;
    holdRefresh: boolean;
    releaseRefresh: () => void;
    rejectRefresh: (reason: Error) => void;
    failRefresh: boolean;
    notifications: string[];
  }
}
const rootElement = document.getElementById("root")!;
import { componentSnapshot as snapshotFixture } from "../../src/components/conversation/ComposerAttachments/componentTestFixtures.ts";
import type { Room, PeerTeam } from "../../src/types.ts";

import { messageModels } from "../messages/message-models.fixture";
import { conversationActionsFixture } from "../../src/components/shell/integration-support/observableFixture.tsx";
import React, { useLayoutEffect, useState } from "react";
import { createRoot } from "react-dom/client";
import { MantineProvider } from "@mantine/core";
import RadioChat from "../../src/components/conversation/RadioChat/RadioChat.tsx";
import {
  PeerTeamGroup,
  PeerTeamActionsProvider,
} from "../../src/components/shell/messages/PeerTeamGroup/PeerTeamGroup.tsx";
import "../../src/style.css";
import "../../src/appearance.css";
import { theme } from "../../src/theme.ts";
const team: PeerTeam = {
  id: "t",
  name: "Research",
  projectPath: "/p",
  members: ["a", "b"],
};
const room: Room = {
  id: "radio:t",
  name: "Research",
  kind: "private",
  projectPath: "/p",
  peerTeamId: "t",
  members: ["a", "b"],
  radio: {
    controls: {
      busy: false,
      inputBlocked: false,
      canPass: false,
      stopping: false,
      speaking: false,
      busyNotice: null,
    },
    teamId: "t",
    revision: 1,
    status: "idle",
    speaker: null,
    next: [],
    active: null,
    error: null,
  },
};
const initial: Snapshot = {
  ...Object.assign(snapshotFixture(), { token: "", stateDir: "/fixture" }),
  stateDir: "radio-test",
  threads: [
    { id: "a", name: "Astra", model: "gpt-6-astra", provider: "codex" },
    { id: "b", name: "Claude", model: "claude-opus-5-5", provider: "claude" },
  ],
  runtime: {
    ...Object.assign(snapshotFixture(), { token: "", stateDir: "/fixture" })
      .runtime,
    requests: [],
    peerTeams: [team],
  },
};
window.radio = () => {
  const state = window.currentRoom.radio;
  if (!state) throw new Error("Missing radio state");
  return state;
};
window.setRoom = () => {};
window.radioOpened = null;
const models = messageModels(initial.stateDir, initial.threads);
models.apply([
  { collection: "room", id: room.id, value: room },
  { collection: "peerTeam", id: team.id, value: team },
]);
const draftValues = observable.map<string, string>();
function Fixture() {
  const [value, setRoom] = useState(room);
  useLayoutEffect(
    () =>
      models.apply([
        { collection: "room", id: value.id, value },
        { collection: "peerTeam", id: team.id, value: team },
      ]),
    [value],
  );
  const getDraft = (session: string) => draftValues.get(session) || "";
  const setDraft = (
    value: string | ((before: string) => string),
    session = room.id,
  ) => {
    const previous = draftValues.get(session) || "";
    const text = typeof value === "function" ? value(previous) : value;
    action(() => draftValues.set(session, text))();
  };
  window.setRoom = setRoom;
  window.currentRoom = value;
  window.radioOpened = null;

  const [area] = useState(() => conversationActionsFixture());
  area.scope.services.drafts.getDraft = getDraft;
  area.scope.services.drafts.setDrafts = (update) => {
    const previous = Object.fromEntries(draftValues);
    const next = typeof update === "function" ? update(previous) : update;
    for (const session of new Set([
      ...Object.keys(previous),
      ...Object.keys(next),
    ]))
      if (previous[session] !== next[session])
        setDraft(next[session] || "", session);
  };
  area.notices.notify = (error) => {
    (window.notifications ??= []).push(error);
  };
  return (
    <MantineProvider theme={theme} defaultColorScheme="dark">
      <div
        style={{
          height: "100%",
          display: "flex",
          flexDirection: "column",
          width: "100%",
        }}
      >
        <style>{`.project-tree-action { inline-size: var(--studio-control-size); opacity: 1; pointer-events: auto; }`}</style>
        <PeerTeamActionsProvider
          value={{
            scope: "radio-test",
            openRoom: (id) => {
              window.radioOpened = id;
            },
            room: () => value.id,
            disabled: () => false,
            closeForm() {},
            edit() {},
            toggle() {},
            candidates: () => [],
          }}
        >
          <PeerTeamGroup team={team} closed={false}>
            <></>
          </PeerTeamGroup>
        </PeerTeamActionsProvider>
        <models.Provider commands={area}>
          <RadioChat room={value} />
        </models.Provider>
      </div>
    </MantineProvider>
  );
}
createRoot(rootElement).render(<Fixture />);
