import { MessageSendStatus } from "../generated/api.ts";
import { afterEach, expect, it, vi } from "vitest";
import { serverLocalStorage } from "../servers/storage.ts";
import { createRemovedMessages } from "./removedMessages.ts";
import type { Message } from "../types";

afterEach(() => vi.unstubAllGlobals());
it("hides and restores a message only in the current page", () => {
  vi.stubGlobal("window", new EventTarget());
  const message: Message = {
    id: "id",
    role: "user",
    text: "text",
    deliveryStatus: { source: "server", status: MessageSendStatus.sending },
  };
  const initial = createRemovedMessages(
    "workspace",
    "agent",
    "chat",
    serverLocalStorage,
  );
  initial.remove(message);
  const hidden = createRemovedMessages(
    "workspace",
    "agent",
    "chat",
    serverLocalStorage,
  );
  expect(hidden.hidden(message)).toBe(true);
  hidden.restore();
  const restored = createRemovedMessages(
    "workspace",
    "agent",
    "chat",
    serverLocalStorage,
  );
  expect(restored.hidden(message)).toBe(false);
  expect(restored.restored).toMatchObject([
    {
      id: "id",
      text: "text",
      deliveryStatus: { source: "server", status: "sending" },
    },
  ]);
});
