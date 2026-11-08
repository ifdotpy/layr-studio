import { setStartupFixture } from "../../components/shell/integration-support/observableFixture.tsx";
// @vitest-environment happy-dom
import { afterEach, expect, it, vi } from "vitest";
import { autorun, runInAction } from "mobx";
import {
  createTranscriptMirror,
  createRoomMirror,
  serializeStateCollections,
} from "../../generated/observableResources.ts";
import { MessageSendStatus, AgentSource } from "../../generated/api.ts";
import {
  observableFixture,
  conversationActionsFixture,
} from "../../components/shell/integration-support/observableFixture.tsx";
import { componentSnapshot as snapshotFixture } from "../../components/conversation/ComposerAttachments/componentTestFixtures.ts";
import { fixtureGetResult } from "../../../tests/misc-fixture.ts";
const cleanups: (() => void)[] = [];
afterEach(() => {
  cleanups
    .splice(0)
    .reverse()
    .forEach((stop) => stop());
  vi.restoreAllMocks();
});
function fixture(nativeB = false) {
  const models = observableFixture(
    ["A", "B", ...Array.from({ length: 12 }, (_, i) => `other-${i}`)].map(
      (id) => ({
        id,
        isLead: true,
        rootId: id,
        source:
          nativeB && id === "B"
            ? AgentSource["app-server"]
            : AgentSource.managed,
      }),
    ),
  );
  const room = createRoomMirror({
    room: { id: "R" },
    messages: [
      {
        id: "room-tail",
        room: "R",
        seq: 2,
        sender: "A",
        senderName: "A",
        created: 2,
        text: "Room tail",
        deliveries: {},
      },
    ],
    nextBefore: 2,
  });
  models.apply([
    ...serializeStateCollections(models.mirror.current),
    { collection: "room", id: "R", value: { id: "R" } },
    { collection: "chat", id: "L", value: { id: "L" } },
  ]);
  const transcripts = new Map(
    ["A", "B"].map((id) => [
      id,
      createTranscriptMirror({
        items: [{ id: "tail", role: "assistant", text: `${id} tail` }],
        truncated: true,
        nextCursor: "tail",
        historyVersion: id,
      }),
    ]),
  );
  const store: typeof models.store = {
    ...models.store,
    room: () => ({ current: { status: "live", data: room.current } }),
    transcript: (id) => {
      const data = transcripts.get(id)?.current;
      return {
        current: data ? { status: "live", data } : { status: "loading" },
      };
    },
  };
  const a = conversationActionsFixture(() => {}, store);
  runInAction(() => {
    setStartupFixture(a, {
      ...Object.assign(snapshotFixture(), { token: "", stateDir: "/fixture" }),
      chats: [{ id: "L" }],
    });
    a.startup.workspaceId = "workspace";
  });
  a.navigation.selectExternal("A", { requestId: "initial-A" });
  const stop = a.selected.start();
  cleanups.push(() => {
    stop();
    a.dispose();
    a.scope.services.close();
    models.store.dispose();
  });
  return { a, models };
}
it("keeps an observed canonical receipt through production A to B to A selection", () => {
  const { a, models } = fixture();
  models.apply([
    ...serializeStateCollections(models.mirror.current),
    {
      collection: "messageReceipt",
      id: "receipt-A",
      value: {
        id: "receipt-A",
        agent: "A",
        status: MessageSendStatus.delivered,
      },
    },
  ]);
  let receipt: string | undefined;
  const stop = autorun(() => {
    receipt = a.selected.receipts.get("receipt-A")?.status;
  });
  cleanups.push(stop);
  expect(receipt).toBe(MessageSendStatus.delivered);
  a.navigation.selectExternal("B", { requestId: "select-B" });
  a.navigation.selectExternal("A", { requestId: "return-A" });
  expect(receipt).toBe(MessageSendStatus.delivered);
});
it("retains fetched older pages and model identities when returning to A offline", async () => {
  const { a } = fixture();
  const get = vi.spyOn(a.scope.services.api, "syncGet").mockResolvedValueOnce(
    fixtureGetResult<"/api/transcript/page">({
      items: [
        { id: "older", role: "user", text: "Earlier page" },
        { id: "tail", role: "assistant", text: "A tail" },
      ],
      truncated: false,
      historyVersion: "A",
    }),
  );
  const session = a.selected.history;
  const snapshots: string[][] = [];
  const stop = autorun(() => {
    snapshots.push(a.selected.history.items.map((item) => item.id));
  });
  cleanups.push(stop);
  await a.selected.history.older();
  const earlier = a.selected.history.items[0];
  expect(snapshots.at(-1)).toEqual(["older", "tail"]);
  get.mockRejectedValue(new TypeError("offline"));
  vi.spyOn(navigator, "onLine", "get").mockReturnValue(false);
  a.navigation.selectExternal("B", { requestId: "select-B" });
  a.navigation.selectExternal("A", { requestId: "return-A" });
  expect(a.selected.history).toBe(session);
  expect(a.selected.history.items[0]).toBe(earlier);
  expect(snapshots.at(-1)).toEqual(["older", "tail"]);
  expect(get).toHaveBeenCalledOnce();
});

it("evicts the oldest history after twelve retained chats and resets at workspace changes", () => {
  const { a } = fixture();
  const original = a.selected.history;
  for (let i = 0; i < 11; i++)
    a.navigation.selectExternal(`other-${i}`, { requestId: `visit-${i}` });
  a.navigation.selectExternal("A", { requestId: "within-bound" });
  expect(a.selected.history).toBe(original);
  for (let i = 0; i < 12; i++)
    a.navigation.selectExternal(`other-${i}`, { requestId: `evict-${i}` });
  a.navigation.selectExternal("A", { requestId: "after-eviction" });
  expect(a.selected.history).not.toBe(original);
  const current = a.selected.history;
  runInAction(() => {
    a.startup.workspaceId = "other-workspace";
  });
  runInAction(() => {
    a.startup.workspaceId = "workspace";
  });
  expect(a.selected.history).not.toBe(current);
});

it("invalidates managed history across a native conversation visit", () => {
  const { a } = fixture(true);
  const managed = a.selected.history;
  expect(a.selected.isManagedConversation).toBe(true);
  a.navigation.selectExternal("B", { requestId: "native-B" });
  expect(a.selected.isManagedConversation).toBe(false);
  a.navigation.selectExternal("A", { requestId: "managed-A" });
  expect(a.selected.isManagedConversation).toBe(true);
  expect(a.selected.history).not.toBe(managed);
});

it("retains observed room pages through a managed agent visit while offline", async () => {
  const { a } = fixture();
  a.navigation.selectExternal("R", { requestId: "room-R" });
  expect(a.selected.kind).toBe("room");
  const session = a.selected.history;
  const snapshots: string[][] = [];
  cleanups.push(
    autorun(() => {
      snapshots.push(a.selected.history.items.map((item) => item.id));
    }),
  );
  const get = vi.spyOn(a.scope.services.api, "syncGet").mockResolvedValueOnce(
    fixtureGetResult<"/api/agent-chat">({
      room: { id: "R" },
      messages: [
        {
          id: "room-older",
          room: "R",
          seq: 1,
          sender: "A",
          senderName: "A",
          created: 1,
          text: "Older room page",
          deliveries: {},
        },
      ],
    }),
  );
  await session.older();
  const earlier = session.items[0];
  expect(snapshots.at(-1)).toEqual(["room-older", "room-tail"]);
  get.mockRejectedValue(new TypeError("offline"));
  vi.spyOn(navigator, "onLine", "get").mockReturnValue(false);
  a.navigation.selectExternal("A", { requestId: "managed-A" });
  expect(a.selected.isManagedConversation).toBe(true);
  a.navigation.selectExternal("R", { requestId: "return-room-R" });
  expect(a.selected.history).toBe(session);
  expect(a.selected.history.items[0]).toBe(earlier);
  expect(snapshots.at(-1)).toEqual(["room-older", "room-tail"]);
  expect(get).toHaveBeenCalledOnce();
});

const cacheDependencies = [
  { dependency: "store", transcript: true, room: true },
  { dependency: "api", transcript: true, room: true },
  { dependency: "resources", transcript: true, room: false },
  { dependency: "managed", transcript: true, room: false },
  { dependency: "legacy", transcript: false, room: true },
  { dependency: "workspace", transcript: true, room: true },
  { dependency: "syncWorkspaceId", transcript: true, room: false },
  { dependency: "enabled", transcript: false, room: false },
  { dependency: "selection", transcript: false, room: false },
] as const;
it.each(cacheDependencies)(
  "matches base transcript and room cache resets for $dependency",
  ({ dependency, transcript, room }) => {
    for (const [id, reset] of [
      ["B", transcript],
      ["R", room],
    ] as const) {
      const { a } = fixture(true);
      a.navigation.selectExternal(id, { requestId: "initial" });
      const session = a.selected.history;
      const stop = autorun(() => {
        void a.selected.history.items.length;
      });
      cleanups.push(stop);
      if (
        dependency === "store" ||
        dependency === "api" ||
        dependency === "resources"
      ) {
        const replacement = fixture(true).a.scope;
        Object.defineProperty(
          dependency === "store" ? a.scope : a.scope.services,
          dependency,
          {
            value:
              dependency === "store"
                ? replacement.store
                : replacement.services[dependency],
            configurable: true,
          },
        );
      } else if (dependency === "workspace") {
        runInAction(() => {
          setStartupFixture(a, {
            ...Object.assign(snapshotFixture(), {
              token: "",
              stateDir: "/fixture",
            }),
            stateDir: "/different",
            chats: [{ id: "L" }],
          });
        });
      } else if (dependency === "syncWorkspaceId") {
        runInAction(() => {
          a.startup.workspaceId = "different-sync-workspace";
        });
      } else if (dependency === "enabled") {
        a.selected.suspendReaders(new Set([id]));
        a.selected.resumeReaders(new Set([id]));
      }
      const via =
        dependency === "managed"
          ? "A"
          : dependency === "legacy"
            ? "L"
            : id === "B"
              ? "R"
              : "B";
      a.navigation.selectExternal(via, { requestId: "dependency-visit" });
      a.navigation.selectExternal(id, { requestId: "return" });
      if (reset) expect(a.selected.history).not.toBe(session);
      else expect(a.selected.history).toBe(session);
    }
  },
);

it("resets legacy room history after visiting a managed agent offline", () => {
  const { a } = fixture();
  a.navigation.selectExternal("L", { requestId: "legacy-L" });
  expect(a.selected.kind).toBe("legacy");
  const session = a.selected.history;
  cleanups.push(
    autorun(() => {
      void a.selected.history.items.length;
    }),
  );
  vi.spyOn(navigator, "onLine", "get").mockReturnValue(false);
  a.navigation.selectExternal("A", { requestId: "legacy-to-managed" });
  a.navigation.selectExternal("L", { requestId: "legacy-return" });
  expect(a.selected.history).not.toBe(session);
});
