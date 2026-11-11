import { createMessageQueueStore } from "../MessageQueue/MessageQueueStore.ts";
import { action } from "mobx";
import { createConversationContext } from "./conversationContext.ts";
import type { Message } from "../../../types.ts";
import { TranscriptHistory } from "../../../conversation/historySession.ts";
// @vitest-environment happy-dom
import { afterEach, expect, it, vi } from "vitest";
import { autorun, flowResult } from "mobx";
import type { PostResult } from "../../../api.ts";
import { AgentSource, AgentStatus } from "../../../generated/api.ts";
import {
  conversationActionsFixture,
  observableFixture,
} from "../../shell/integration-support/observableFixture.tsx";
import { createSendingMessageRemoval } from "../../../app/conversation/delivery/removeSendingMessages.ts";
import { AttachmentDrafts } from "./useAttachmentDrafts.ts";
import { UploadRecovery } from "./useUploadRecovery.ts";
import {
  ConversationOperations,
  ConversationStore,
  type ConversationContext,
} from "./ConversationStore.ts";

const cleanups: (() => void)[] = [];
afterEach(() => {
  cleanups
    .splice(0)
    .reverse()
    .forEach((stop) => stop());
  vi.restoreAllMocks();
});
function mount() {
  const models = observableFixture([
    { id: "lead", source: AgentSource.managed, status: AgentStatus.running },
  ]);
  const areas = conversationActionsFixture(vi.fn(), models.store);
  areas.navigation.selectedId = "lead";
  const services = areas.scope.services,
    { api, uploads, outbox } = services;
  const attachmentDrafts = new AttachmentDrafts(
    uploads.attachmentCancelled,
    areas.notices.notify,
  );
  const recovery = new UploadRecovery(
    uploads,
    "fixture",
    "workspace",
    attachmentDrafts.set,
  );
  const scrolling = {
    scroll: { current: null },
    content: { current: null },
    getFollow: () => true,
    subscribeFollow: () => () => {},
    setFollow: vi.fn(),
    onScroll: vi.fn(),
    hasRecentScrollInput: () => false,
    consumeHistoryScrollInput: () => false,
    remember: vi.fn(),
    revealRequest: vi.fn(),
    getAnchorId: () => null,
  };
  const context = {
    ...createConversationContext(areas, attachmentDrafts),
    actions: areas,
    opened: "lead",
    stateDir: "fixture",
    kind: "agent",
    api,
    changeOutbox: outbox.changeOutbox,
    queueUploads: uploads.queueUploads,
    removeSendingMessage: createSendingMessageRemoval({
      get: api.get,
      command: api.command,
      stopRemovedMessage: outbox.stopRemovedMessage,
      localStorage: services.storage,
    }).removeSendingMessage,
    messageQueue: services.net.queue(
      { id: "lead", scope: "fixture", enabled: true },
      new Set(),
    ),
    uploadRecovery: recovery,
    attachmentDrafts,
    input: { current: null },
    operations: new ConversationOperations(),
    scrolling,
    start: () => () => {},
  } satisfies ConversationContext;
  const store = new ConversationStore(context);
  areas.selected.conversation = store;
  cleanups.push(() => {
    store.resetRecall();
    services.close();
    models.store.dispose();
  });
  return { store, areas, context, api, uploads };
}
it("locks a pending branch, keeps its identity on an uncertain retry, then releases it", async () => {
  const { store, api } = mount();
  const pending = Promise.withResolvers<PostResult<"/api/branch">>();
  const post = vi
    .spyOn(api, "post")
    .mockReturnValueOnce(pending.promise)
    .mockResolvedValueOnce({ id: "new-chat" });
  const message = {
    id: "answer",
    role: "assistant",
    text: "Answer",
    turnId: "done",
  } satisfies Message;
  const first = flowResult(store.branch(message));
  expect(store.branching).toBe(true);
  await flowResult(store.branch(message));
  expect(post).toHaveBeenCalledTimes(1);
  pending.reject(new TypeError("response lost"));
  await first;
  expect(store.branching).toBe(false);
  await flowResult(store.branch(message));
  expect(post).toHaveBeenCalledTimes(2);
  expect(post.mock.calls[1]).toEqual(post.mock.calls[0]);
  expect(store.branching).toBe(false);
});
it("reads pending commands directly without a component subscription", () => {
  const { store, api } = mount();
  const shown: boolean[] = [];
  const stop = autorun(() => shown.push(store.stopShown));
  cleanups.push(stop);
  api.pending.begin({ id: "stop-request", kind: "stop", target: "lead" });
  api.pending.fail("stop-request");
  expect(shown).toEqual([false, true, false]);
});
it("locks file staging until the captured upload finishes and exposes its error", async () => {
  const { store, uploads } = mount();
  const pending =
    Promise.withResolvers<Awaited<ReturnType<typeof uploads.queueUploads>>>();
  const queue = vi
    .spyOn(store.context, "queueUploads")
    .mockReturnValueOnce(pending.promise);
  const file = new File(["bytes"], "notes.txt");
  const first = store.addFiles([file]);
  expect(store.uploading).toBe(true);
  await expect(store.addFiles([file])).rejects.toThrow(
    "Wait for the selected files",
  );
  expect(queue).toHaveBeenCalledTimes(1);
  pending.reject(new Error("staging failed"));
  await expect(first).rejects.toThrow("staging failed");
  expect(store.uploading).toBe(false);
});
it("ignores a completed message jump after switching the workspace with the same chat id", async () => {
  const pending = Promise.withResolvers<boolean>();
  vi.spyOn(TranscriptHistory.prototype, "ensureMessage").mockReturnValueOnce(
    pending.promise,
  );
  const { store, areas } = mount();
  const root = document.createElement("div"),
    row = document.createElement("article");
  row.dataset.message = "target";
  root.append(row);
  store.context.scrolling.scroll.current = root;
  const jump = store.navigateToMessage("target");
  areas.selected.closeConversation();
  pending.resolve(true);
  await jump;
  expect(store.highlighted).toBeNull();
  expect(store.context.scrolling.revealRequest).not.toHaveBeenCalled();
});

it("reads an already restored edit directly without a component synchronization effect", () => {
  const owner = createMessageQueueStore(
    () => ({
      scope: "restored-queue",
      items: [],
      onEdit: async () => {},
      onCancel: async () => {},
      onReorder: async () => {},
      canReorder: false,
    }),
    "local",
  );
  const apply = action(() => {
    owner.buffer.values.set(
      owner.buffer.key("queue-edit", "restored-queue"),
      JSON.stringify({ id: "departed", text: "Saved edit" }),
    );
  });
  apply();
  expect(owner.current?.text).toBe("Saved edit");
  expect(owner.absent?.text).toBe("Saved edit");
});
