import "../../src/rendererStyles.ts";
import { observableFixture } from "../../src/components/shell/integration-support/observableFixture.tsx";
import type { QueueEntityDto } from "../../src/generated/observableResources.ts";
import { createRoot } from "react-dom/client";
import type { Root } from "react-dom/client";
import { useMessageQueue } from "../../src/components/conversation/support/useMessageQueue.ts";
import type { removeSendingMessage } from "../../src/app/conversation/delivery/removeSendingMessages.ts";

declare global {
  interface Window {
    receiveDeadlineQueue(view: QueueEntityDto): void;
    queue: ReturnType<typeof useMessageQueue>;
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

function Harness({ workspaceId }: { workspaceId: string }) {
  window.queue = useMessageQueue({
    id: "chat",
    enabled: true,
    scope: "deadline-fixture",
    workspaceId,
    observed: (ids) => window.observed.push(ids),
    edited: (id, text) => window.edited.push({ id, text }),
  });
  return null;
}

export function mount(workspaceId: string) {
  const models = observableFixture();
  window.receiveDeadlineQueue = (value) =>
    models.apply([{ collection: "queue", id: "chat", value }]);
  const root = createRoot(document.body);
  root.render(
    <models.Provider>
      <Harness workspaceId={workspaceId} />
    </models.Provider>,
  );
  return root;
}
