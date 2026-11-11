import "../../src/rendererStyles.ts";
import { observableFixture } from "../../src/components/shell/integration-support/observableFixture.tsx";
import type { QueueEntityDto } from "../../src/generated/observableResources.ts";
import { createRoot } from "react-dom/client";
import type { Root } from "react-dom/client";
import { createMessageQueue } from "../../src/servers/net/queue.ts";
import * as api from "../../src/servers/net/api.ts";
import { updateQueueRequest } from "../../src/sync/localDraft.ts";
import { reaction } from "mobx";
import type { removeSendingMessage } from "../../src/app/conversation/delivery/removeSendingMessages.ts";

declare global {
  interface Window {
    receiveDeadlineQueue(view: QueueEntityDto): void;
    queue: ReturnType<typeof createMessageQueue>;
    observed: string[][];
    edited: Array<{ id: string; text: string }>;
    advance: number;
    // Renamed from reactRoot: terminal/terminal-stub.ts declares it as Root | null.
    queueDeadlineRoot: Root;
    outcome: string;
    operation: Promise<string>;
    removal: typeof import("../../src/app/conversation/delivery/removeSendingMessages.ts");
    removed: typeof import("../../src/app/conversation/delivery/removedMessages.ts");
    // Renamed from scope: files/upload-recovery-browser.fixture.tsx declares it as a string.
    queueDeadlineScope: Parameters<typeof removeSendingMessage>[0];
    target: Parameters<typeof removeSendingMessage>[1];
  }
}

export function mount(workspaceId: string) {
  const models = observableFixture();
  window.queue = createMessageQueue(
    {
      id: "chat",
      enabled: true,
      scope: "deadline-fixture",
      workspaceId,
      observed: (ids) => window.observed.push(ids),
      edited: (id, text) => window.edited.push({ id, text }),
    },
    models.store,
    api,
    localStorage,
    updateQueueRequest,
    new Set(),
  );
  reaction(
    () => window.queue.items.map((item) => item.id),
    (ids) => window.observed.push(ids),
    { fireImmediately: true },
  );
  window.receiveDeadlineQueue = (value) =>
    models.apply([{ collection: "queue", id: "chat", value }]);
  const root = createRoot(document.body);
  root.render(null);
  return root;
}
