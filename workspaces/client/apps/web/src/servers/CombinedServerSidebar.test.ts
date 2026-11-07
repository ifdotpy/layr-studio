import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { expect, it, vi } from "vitest";
import type { SidebarProps } from "../components/Sidebar.tsx";
import CombinedServerSidebar from "./CombinedServerSidebar.tsx";
import { mergeSidebar } from "./mergedSidebar.ts";
import { sidebarSnapshot } from "./sidebarSnapshot.ts";
import { sidebarRowFixture } from "./sidebarRowFixture.ts";
import { snapshotFixture } from "../snapshotFixture.ts";
import { navigationSnapshot } from "./navigation.ts";
import { localSidebarBackend } from "../components/sidebar/services.tsx";
import { required } from "../components/componentTestFixtures.ts";

const captured = vi.hoisted(() => ({ props: null as SidebarProps | null }));
vi.mock("../components/Sidebar", () => ({
  default: (props: SidebarProps) => {
    captured.props = props;
    return null;
  },
}));

it("routes overlapping row ids and the completed-result CAS target to their owning server", async () => {
  const target = {
    threadId: "native-thread",
    turnId: "completed-turn",
    read: true,
    revision: 7,
  };
  const rows = [
    sidebarRowFixture("same", { markRead: target, markUnread: "allowed" }),
  ];
  const model = mergeSidebar(
    ["local", "remote"].map((id) => ({
      id,
      online: true,
      sidebar: sidebarSnapshot(snapshotFixture([]), rows),
    })),
  );
  const send = vi.fn(),
    executeCommand = vi.fn(async () => {});
  renderToStaticMarkup(
    createElement(CombinedServerSidebar, {
      model,
      backends: new Map(
        ["local", "remote"].map((id) => [
          id,
          { ...localSidebarBackend, ownerId: id },
        ]),
      ),
      online: () => true,
      refreshOwner: async () => navigationSnapshot(null, null, ""),
      executeCommand,
      aliases: {},
      current: "remote",
      navigation: {},
      send,
      applyNavigation: vi.fn(),
      mobile: false,
      hidden: false,
      close: vi.fn(),
      settings: vi.fn(),
      search: vi.fn(),
      notify: vi.fn(),
      notice: "",
      hideOldChatsThreshold: 3,
    }),
  );
  const props = required(captured.props);
  expect(props.rows).toBe(model.rows);
  expect(props.data.threads).toEqual([]);
  const id = model.chatKey("remote", "same");
  props.open(id);
  props.changeProject(id);
  props.markUnread(id, target);
  expect(send.mock.calls).toEqual([
    ["remote", { action: "open", id: "same" }],
    ["remote", { action: "change-project", id: "same" }],
  ]);
  expect(executeCommand).toHaveBeenCalledExactlyOnceWith("remote", {
    action: "mark-unread",
    id: "same",
    threadId: "native-thread",
    turnId: "completed-turn",
  });
  await Promise.resolve();
});
