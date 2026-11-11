import { observable, action } from "mobx";
import { RoomHistory } from "../../../conversation/roomHistory.ts";
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

const history = vi.spyOn(RoomHistory.prototype, "start");
vi.mock("../support/useConversationScroll.ts", () => ({
  createConversationScroll: () => ({
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
            { value: conversationActionsFixture(undefined, f.store) },
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
  const draft = observable.box("");
  const actions = conversationActionsFixture(undefined, f.store);
  actions.scope.services.drafts.getDraft = () => draft.get();
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
      action(() => draft.set("A new instruction"))();
    });
    expect(host.querySelector("textarea")).toBe(input);
    expect(input?.value).toBe(draft.get());
    expect(history.mock.calls.length).toBe(before);
  } finally {
    await act(async () => root.unmount());
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
      actions = conversationActionsFixture(undefined, f.store);
    actions.scope.services.drafts.getDraft = () => "instruction";
    const host = document.createElement("div");
    document.body.append(host);
    const root = createRoot(host);
    radioApi.post
      .mockReset()
      .mockRejectedValue(new ApiError("Request failed", status));
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
      const original = radioApi.post.mock.calls[0]?.[1];
      radioApi.post.mockResolvedValue({});
      await act(async () => {
        if (!(button instanceof HTMLButtonElement))
          throw new Error("Retry missing");
        button.click();
        button.click();
      });
      expect(radioApi.post).toHaveBeenCalledTimes(status === 408 ? 2 : 1);
      if (status === 408)
        expect(radioApi.post.mock.calls[1]?.[1]).toEqual(original);
      expect(serverLocalStorage.getItem(key)).toBe("null");
    } finally {
      await act(async () => root.unmount());
      f.store.dispose();
      host.remove();
      serverLocalStorage.removeItem(key);
      vi.unstubAllGlobals();
    }
  },
);

it("renders radio-turn requests and acts through the selected conversation request owner", async () => {
  vi.stubGlobal("IS_REACT_ACT_ENVIRONMENT", true);
  Object.defineProperty(document, "fonts", {
    configurable: true,
    value: new EventTarget(),
  });
  const { requestFixture } =
    await import("../questions/support/requestFixtures.ts");
  const { approval, question } = await import("../../../approvalFixtures.ts");
  const f = requestFixture([
    approval({
      id: "radio-question",
      agent: "speaker",
      turn: "turn",
      request: {
        kind: "agent_question",
        questions: [question("choice", "Radio answer?")],
      },
    }),
  ]);
  f.state.apply([
    {
      collection: "room",
      id: room.id,
      value: {
        ...room,
        radio: {
          ...room.radio!,
          active: {
            agentId: "speaker",
            turnId: "turn",
            epoch: 1,
            eventId: "event",
            identity: [null, null, 1],
            threadId: null,
            through: 1,
          },
        },
      },
    },
  ]);
  const actions = conversationActionsFixture(() => {}, f.store);
  actions.selected.requestOwner.start();
  const host = document.body.appendChild(document.createElement("div"));
  const root = createRoot(host);
  try {
    await act(async () =>
      root.render(
        createElement(
          MantineProvider,
          null,
          createElement(
            ConversationProvider,
            { value: actions },
            createElement(
              (
                await import("../../servers/ServerServicesProvider/ServerServicesProvider.tsx")
              ).ServerStoreProvider,
              { value: f.store },
              createElement(RadioChat, {
                room: f.state.current.rooms.get(room.id)!,
              }),
            ),
          ),
        ),
      ),
    );
    expect(
      host.querySelector('[data-request="radio-question"]'),
    ).not.toBeNull();
    const view = actions.selected.requestOwner.views.get("radio-question")!;
    await act(async () =>
      host.querySelector<HTMLButtonElement>("[data-answer]")!.click(),
    );
    expect(view.open).toBe(true);
    expect(
      host.querySelector('textarea[aria-label="Radio answer?"]'),
    ).not.toBeNull();
  } finally {
    await act(async () => root.unmount());
    actions.selected.closeConversation();
    actions.scope.services.close();
    f.store.dispose();
  }
});

it("uses replacement API and draft owners when the same radio room rerenders", async () => {
  vi.stubGlobal("IS_REACT_ACT_ENVIRONMENT", true);
  Object.defineProperty(document, "fonts", {
    configurable: true,
    value: new EventTarget(),
  });
  const key = "studio-radio-command:radio-test:room";
  serverLocalStorage.removeItem(key);
  const f = fixture({
    ...room,
    projectPath: "/p",
    radio: {
      ...room.radio!,
      controls: { ...room.radio!.controls, inputBlocked: false },
    },
  });
  const first = conversationActionsFixture(undefined, f.store);
  const second = conversationActionsFixture(undefined, f.store);
  first.scope.services.drafts.getDraft = () => "old draft";
  second.scope.services.drafts.getDraft = () => "new draft";
  const clearFirst = vi.spyOn(first.scope.services.drafts, "setDrafts");
  const clearSecond = vi.spyOn(second.scope.services.drafts, "setDrafts");
  const oldPost = vi.fn().mockResolvedValue({});
  const newPost = vi.fn().mockResolvedValue({});
  const previousPost = radioApi.post;
  const host = document.createElement("div"),
    root = createRoot(host);
  const show = (actions: typeof first) =>
    act(async () =>
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
  try {
    radioApi.post = oldPost;
    await show(first);
    radioApi.post = newPost;
    await show(second);
    await act(async () => {
      host
        .querySelector("form")
        ?.dispatchEvent(
          new Event("submit", { bubbles: true, cancelable: true }),
        );
    });
    expect(oldPost).not.toHaveBeenCalled();
    expect(newPost).toHaveBeenCalledOnce();
    expect(newPost.mock.calls[0]?.[1]).toMatchObject({ text: "new draft" });
    expect(clearFirst).not.toHaveBeenCalled();
    expect(clearSecond).toHaveBeenCalledOnce();
  } finally {
    await act(async () => root.unmount());
    radioApi.post = previousPost;
    first.scope.services.close();
    second.scope.services.close();
    f.store.dispose();
    serverLocalStorage.removeItem(key);
    vi.unstubAllGlobals();
  }
});
