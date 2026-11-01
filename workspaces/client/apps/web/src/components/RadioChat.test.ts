import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { MantineProvider } from "@mantine/core";
import { expect, it, vi } from "vitest";
import { RadioStatus, RoomKind } from "../generated/api";
import { componentSnapshot } from "./componentTestFixtures";
import type { Room } from "../types";

vi.mock("../servers/ServerServicesContext", () => ({
  useServerApi: () => ({ post: vi.fn(), save: vi.fn(), saved: () => null }),
}));
vi.mock("../hooks", () => ({
  useMessages: () => ({ items: [], loaded: true, before: null }),
}));
vi.mock("./useConversationScroll", () => ({
  useConversationScroll: () => ({
    scroll: { current: null },
    content: { current: null },
    onScroll: () => {},
  }),
}));
vi.mock("./prompt-composer/PromptComposer", () => ({
  usePromptDraft: () => "draft",
}));
import RadioChat from "./RadioChat";

it("renders controls from the room projection when raw radio status disagrees", () => {
  const room = {
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
        busyNotice: "Current reply finishes first.",
      },
    },
  } as Room;
  const markup = renderToStaticMarkup(
    createElement(
      MantineProvider,
      {},
      createElement(RadioChat, {
        room,
        data: componentSnapshot([]),
        getDraft: () => "draft",
        subscribeDraft: () => () => {},
        setDraft: () => {},
        refresh: async () => {},
        notify: () => {},
      }),
    ),
  );
  expect(markup).toContain(">Stop<");
  expect(markup).toMatch(/<button[^>]*disabled[^>]*>[^]*?Send/);
});
