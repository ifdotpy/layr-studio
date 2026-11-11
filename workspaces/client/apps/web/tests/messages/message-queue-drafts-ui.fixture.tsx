import "../../src/rendererStyles.ts";
import type { QueueItem } from "../../src/components/conversation/MessageQueue/MessageQueue.tsx";

import { makeAutoObservable } from "mobx";
import { createMessageQueueStore } from "../../src/components/conversation/MessageQueue/MessageQueueStore.ts";
import { createRoot } from "react-dom/client";
import MessageQueue from "../../src/components/conversation/MessageQueue/MessageQueue.tsx";
const state = makeAutoObservable({
  items: [
    {
      id: "q1",
      text: "Original message",
      created: 1,
      status: "queued",
      kind: "user",
    },
  ] as QueueItem[],
});
const store = createMessageQueueStore(
  () => ({
    scope: "journal-fixture",
    items: state.items,
    canReorder: true,
    onEdit: async (item, text) => {
      state.items = [{ ...item, text }];
    },
    onCancel: async () => {
      state.items = [];
    },
    onReorder: async () => {},
  }),
  "local",
);
createRoot(document.getElementById("root")!).render(
  <MessageQueue store={store} />,
);
