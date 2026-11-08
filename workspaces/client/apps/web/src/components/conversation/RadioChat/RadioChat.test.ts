import { resourceData } from "../../../store/runtime.ts";
import { required } from "../ComposerAttachments/componentTestFixtures.ts";
// @vitest-environment happy-dom
import { RadioStatus, RoomKind } from "../../../generated/api.ts";
import { act, createElement } from "react";
import { createRoot } from "react-dom/client";
import { renderToStaticMarkup } from "react-dom/server";
import { MantineProvider } from "@mantine/core";
import { expect, it, vi } from "vitest";
import {
  observableFixture,
  conversationActionsFixture,
} from "../../shell/integration-support/observableFixture.tsx";
import { ConversationProvider } from "../support/navigation.ts";
import { ApiError } from "../../../api.ts";
import { serverLocalStorage } from "../../../servers/storage.ts";
import type { RoomEntityDtoFields } from "../../../generated/observableFieldTypes.ts";

const radioApi = vi.hoisted(() => ({ post: vi.fn() }));
vi.mock(
  "../../servers/ServerServicesProvider/ServerServicesProvider.tsx",
  async (importOriginal) => {
    const actual =
      await importOriginal<
        typeof import("../../servers/ServerServicesProvider/ServerServicesProvider.tsx")
      >();
    return {
      ...actual,
      useServerApi: () => ({ ...actual.useServerApi(), post: radioApi.post }),
    };
  },
);

const history = vi.hoisted(() =>
  vi.fn(() => ({ items: [], loaded: true, before: null, reload() {} })),
);
vi.mock("../../../conversation/roomHistory.ts", () => ({
  useRoomHistory: history,
}));
vi.mock("../support/useConversationScroll.ts", () => ({
  useConversationScroll: () => ({
    scroll: { current: null },
    content: { current: null },
    onScroll() {},
  }),
}));
import RadioChat from "./RadioChat.tsx";

const room: RoomEntityDtoFields = {
  id: "room",
  kind: RoomKind.private,
  members: [],
  radio: {
    teamId: "team",
    revision: 1,
    status: RadioStatus.idle,
    speaker: null,
    next: [],
    active: null,
    error: null,
    controls: {
      busy: true,
      inputBlocked: true,
      canPass: true,
      stopping: false,
      speaking: false,
    },
  },
};
function fixture(value = room) {
  const f = observableFixture();
  f.apply([
    { collection: "room", id: value.id, value },
    {
      collection: "workspace",
      id: "workspace",
      value: { stateDir: "radio-test" },
    },
  ]);
  return f;
}
it("renders controls from the room projection when raw radio status disagrees", () => {
  const f = fixture();
  try {
    const markup = renderToStaticMarkup(
      createElement(
        MantineProvider,
        {},
        createElement(
          f.Provider,
          {},
          createElement(
            ConversationProvider,
            { value: conversationActionsFixture() },
            createElement(RadioChat, {
              room: required(
                resourceData(f.store.state().current)?.rooms.get(room.id),
              ),
            }),
          ),
        ),
      ),
    );
    expect(markup).toContain(">Stop<");
    expect(markup).toMatch(/<button[^>]*disabled[^>]*>[^]*?Send/);
  } finally {
    f.store.dispose();
  }
});

it("typing a radio draft does not render the screen or reacquire history", async () => {
  vi.stubGlobal("IS_REACT_ACT_ENVIRONMENT", true);
  Object.defineProperty(document, "fonts", {
    configurable: true,
    value: new EventTarget(),
  });
  const f = fixture();
  const host = document.createElement("div");
  document.body.append(host);
  const root = createRoot(host);
  let draft = "";
  const listeners = new Set<() => void>();
  const actions = conversationActionsFixture();
  actions.scope.services.drafts.getDraft = () => draft;
  actions.scope.services.drafts.subscribeDraft = (_id, listener) => {
    listeners.add(listener);
    return () => {
      listeners.delete(listener);
    };
  };
  try {
    await act(async () =>
      root.render(
        createElement(
          MantineProvider,
          {},
          createElement(
            f.Provider,
            {},
            createElement(
              ConversationProvider,
              { value: actions },
              createElement(RadioChat, {
                room: required(
                  resourceData(f.store.state().current)?.rooms.get(room.id),
                ),
              }),
            ),
          ),
        ),
      ),
    );
    const before = history.mock.calls.length;
    const input = host.querySelector("textarea");
    await act(async () => {
      draft = "A new instruction";
      listeners.forEach((listener) => listener());
    });
    expect(host.querySelector("textarea")).toBe(input);
    expect(input?.value).toBe(draft);
    expect(history.mock.calls.length).toBe(before);
  } finally {
    await act(async () => root.unmount());
    expect(listeners.size).toBe(0);
    f.store.dispose();
    host.remove();
    vi.unstubAllGlobals();
  }
});

it.each([408, 409])(
  "radio classifies HTTP %s through its existing command boundary",
  async (status) => {
    vi.stubGlobal("IS_REACT_ACT_ENVIRONMENT", true);
    Object.defineProperty(document, "fonts", {
      configurable: true,
      value: new EventTarget(),
    });
    const key = "studio-radio-command:radio-test:room";
    serverLocalStorage.removeItem(key);
    if (!room.radio) throw new Error("Radio missing");
    const value: RoomEntityDtoFields = {
      ...room,
      projectPath: "/p",
      radio: {
        ...room.radio,
        controls: { ...room.radio.controls, inputBlocked: false },
      },
    };
    const f = fixture(value),
      actions = conversationActionsFixture();
    actions.scope.services.drafts.getDraft = () => "instruction";
    const host = document.createElement("div");
    document.body.append(host);
    const root = createRoot(host);
    radioApi.post.mockRejectedValue(new ApiError("Request failed", status));
    try {
      await act(async () =>
        root.render(
          createElement(
            MantineProvider,
            {},
            createElement(
              f.Provider,
              {},
              createElement(
                ConversationProvider,
                { value: actions },
                createElement(RadioChat, {
                  room: required(
                    resourceData(f.store.state().current)?.rooms.get(room.id),
                  ),
                }),
              ),
            ),
          ),
        ),
      );
      await act(async () => {
        host
          .querySelector("form")
          ?.dispatchEvent(
            new Event("submit", { bubbles: true, cancelable: true }),
          );
      });
      const button = host.querySelector(".radio-retry button");
      expect(button?.textContent).toBe(status === 408 ? "Retry" : "Refresh");
      const stored = serverLocalStorage.getItem(key);
      expect(stored).toContain('"text":"instruction"');
    } finally {
      await act(async () => root.unmount());
      f.store.dispose();
      host.remove();
      serverLocalStorage.removeItem(key);
      vi.unstubAllGlobals();
    }
  },
);
