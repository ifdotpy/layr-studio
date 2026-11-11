import "../../src/rendererStyles.ts";
import React from "react";
import { createRoot } from "react-dom/client";
import { createMessageQueueStore } from "../../src/components/conversation/MessageQueue/MessageQueueStore.ts";
import MessageQueue from "../../src/components/conversation/MessageQueue/MessageQueue.tsx";
import type { QueueItemDto } from "../../src/api.ts";

export function mount() {
  const node = document.createElement("div");
  document.body.appendChild(node);
  const item = {
    id: "one",
    text: "original",
    created: 1,
    status: "queued",
    kind: "user",
  } as QueueItemDto;
  const store = createMessageQueueStore(
    () => ({
      scope: "test",
      items: [item],
      canReorder: false,
      onEdit: async (_item, text) => {
        document.title = text;
      },
      onCancel: async () => {},
      onReorder: async () => {},
    }),
    "local",
  );
  createRoot(node).render(<MessageQueue store={store} />);
}
