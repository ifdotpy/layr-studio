import { observer } from "mobx-react-lite";
// @vitest-environment happy-dom
import { act, createElement, type ReactNode } from "react";
import { createRoot } from "react-dom/client";
import { MantineProvider } from "@mantine/core";
import { afterEach, expect, it, vi } from "vitest";
import {
  createComplaintMirror,
  createRoomMirror,
  createTranscriptMirror,
} from "../../../../generated/observableResources.ts";
import { ServerStoreProvider } from "../../../servers/ServerServicesProvider/ServerServicesProvider.tsx";
import {
  observableFixture,
  conversationActionsFixture,
} from "../../../shell/integration-support/observableFixture.tsx";
import { ConversationProvider } from "../../support/navigation.ts";
import { requestTestEnvironment } from "../../questions/Requests/requestTestFixture.ts";
import { UserMessage } from "../../../shell/messages/UserMessage/UserMessage.tsx";
import { useConversationHistory } from "../../../../conversation/historyFixture.ts";

const h = vi.hoisted(() => ({ get: vi.fn(), connection: vi.fn() }));
vi.mock("../../../../unsent/useUnsentText.ts", async () => {
  const { useState } = await import("react");
  return {
    useUnsentText: () => {
      const [text, setText] = useState<string | null>(null);
      return [text, setText, async () => {}];
    },
  };
});
vi.mock(
  "../../../servers/ServerServicesProvider/ServerServicesProvider.tsx",
  async (original) => ({
    ...(await original<
      typeof import("../../../servers/ServerServicesProvider/ServerServicesProvider.tsx")
    >()),
    useServerApi: () => api,
    useServerResources: () => resources,
  }),
);
const api = { get: h.get, syncGet: h.get };
const resources = {
  watchLiveDeltas: () => () => {},
  watchResourceConnection: (
    accept: (health: "live", activity: "open") => void,
  ) => {
    h.connection();
    accept("live", "open");
    return () => {};
  },
};
requestTestEnvironment();
afterEach(() => vi.clearAllMocks());

function fixture() {
  const models = observableFixture();
  const transcript = createTranscriptMirror({ items: [], truncated: false });
  const room = createRoomMirror({ messages: [], room: { id: "team" } });
  const complaint = createComplaintMirror({
    id: "message",
    version: 1,
    text: "First",
    responses: [],
  });
  const store: typeof models.store = {
    ...models.store,
    transcript: () => ({
      get current() {
        return { status: "live" as const, data: transcript.current };
      },
    }),
    room: () => ({
      get current() {
        return { status: "live" as const, data: room.current };
      },
    }),
    complaint: () => ({
      get current() {
        return { status: "live" as const, data: complaint.current };
      },
    }),
  };
  const host = document.body.appendChild(document.createElement("div"));
  const root = createRoot(host);
  return {
    transcript,
    room,
    complaint,
    host,
    async render(child: ReactNode) {
      await act(async () =>
        root.render(
          createElement(
            MantineProvider,
            null,
            createElement(
              ServerStoreProvider,
              { value: store },
              createElement(
                ConversationProvider,
                { value: conversationActionsFixture() },
                child,
              ),
            ),
          ),
        ),
      );
    },
    async close() {
      await act(async () => root.unmount());
      models.store.dispose();
    },
  };
}

it("complaint version changes consume generated detail through the real message without GET", async () => {
  const f = fixture();
  try {
    await f.render(
      createElement(UserMessage, {
        message: { id: "message", version: 1 },
        scope: "scope",
        focused: false,
        requests: new Map(),
      }),
    );
    expect(f.host.textContent).toContain("First");
    await act(async () =>
      f.complaint.apply({
        id: "message",
        version: 2,
        text: "Updated",
        responses: [],
      }),
    );
    expect(f.host.textContent).toContain("Updated");
    expect(h.get).not.toHaveBeenCalled();
  } finally {
    await f.close();
  }
});

it.each(["agent", "room"] as const)(
  "%s updates reach the observer leaf without a resource GET",
  async (kind) => {
    const f = fixture();
    const View = observer(function View() {
      const messages = useConversationHistory(
        "team",
        kind,
        false,
        "scope",
        "workspace",
      );
      return createElement(
        "output",
        null,
        messages.items.map((item) => item.text).join("|"),
      );
    });
    try {
      await f.render(createElement(View));
      await act(async () => {
        if (kind === "agent")
          f.transcript.apply({
            items: [{ id: "new", role: "assistant", text: "Streamed" }],
            truncated: false,
          });
        else
          f.room.apply({
            messages: [
              {
                id: "new",
                seq: 1,
                text: "Streamed",
                created: 1,
                sender: "lead",
                senderName: "Lead",
                room: "team",
                deliveries: {},
              },
            ],
            room: { id: "team" },
          });
      });
      expect(f.host.querySelector("output")?.textContent).toBe("Streamed");
      expect(h.get).not.toHaveBeenCalled();
    } finally {
      await f.close();
    }
  },
);

it("restores agent transport after visiting a room in the same caller", async () => {
  const f = fixture();
  const View = observer(function View({ kind }: { kind: "agent" | "room" }) {
    const messages = useConversationHistory(
      "team",
      kind,
      true,
      "scope",
      "workspace",
    );
    return createElement("output", null, messages.connection?.health ?? "room");
  });
  try {
    await f.render(createElement(View, { kind: "agent" }));
    expect(f.host.querySelector("output")?.textContent).toBe("live");
    await f.render(createElement(View, { kind: "room" }));
    expect(f.host.querySelector("output")?.textContent).toBe("room");
    await f.render(createElement(View, { kind: "agent" }));
    expect(f.host.querySelector("output")?.textContent).toBe("live");
    expect(h.connection).toHaveBeenCalledTimes(2);
    expect(h.get).not.toHaveBeenCalled();
  } finally {
    await f.close();
  }
});
