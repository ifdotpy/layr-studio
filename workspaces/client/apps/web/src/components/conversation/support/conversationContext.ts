import { DictationStore } from "../DictationPanel/DictationPanel.tsx";
import { createNativeDictation } from "../Dictation/Dictation.tsx";
import { createBrowserDictation } from "../BrowserDictation/BrowserDictation.tsx";
import { createRef } from "react";
import type { ServerAreas } from "../../../app/ServerAreasProvider.tsx";
import { AgentSource } from "../../../generated/api.ts";
import { AttachmentDrafts } from "./useAttachmentDrafts.ts";
import { UploadRecovery } from "./useUploadRecovery.ts";
import { createConversationScroll } from "./useConversationScroll.ts";
import { createSendingMessageRemoval } from "../../../app/conversation/delivery/removeSendingMessages.ts";
import { onResume } from "../../../sync/resume.ts";

export function createConversationContext(
  actions: ServerAreas,
  attachmentDrafts: AttachmentDrafts,
  previousRecovery?: UploadRecovery,
) {
  const {
    selected,
    navigation,
    startup,
    scope: { services },
  } = actions;
  const { api, uploads, outbox, net, storageEventKey } = services;
  const opened = navigation.selectedId,
    kind = selected.kind,
    stateDir = startup.creationScope || "";
  const uploadRecovery =
    previousRecovery?.stateDir === stateDir &&
    previousRecovery.workspaceId === startup.workspaceId
      ? previousRecovery
      : new UploadRecovery(
          uploads,
          stateDir,
          startup.workspaceId,
          attachmentDrafts.set,
        );
  const dictation = new DictationStore();
  const recordingKey = `${stateDir}:${opened}`;
  const nativeDictation = services.audio.dictationBridge()
    ? createNativeDictation(services, dictation, recordingKey)
    : undefined;
  const browserDictation = createBrowserDictation(dictation);
  const messageQueue = net.queue(
    {
      id: opened,
      enabled: navigation.selectedAgent?.source === AgentSource.managed,
      scope: `${stateDir}:${startup.workspaceId}:${opened}`,
      workspaceId: startup.workspaceId,
      observed: actions.delivery.observeSends,
      edited: actions.delivery.editSend,
    },
    new Set(),
  );
  return {
    dictation,
    nativeDictation,
    browserDictation,
    actions,
    opened,
    kind,
    stateDir,
    api,
    uploadRecovery,
    messageQueue,
    attachmentDrafts,
    changeOutbox: outbox.changeOutbox,
    queueUploads: uploads.queueUploads,
    removeSendingMessage: createSendingMessageRemoval({
      get: api.get,
      command: api.command,
      stopRemovedMessage: outbox.stopRemovedMessage,
      localStorage: services.storage,
    }).removeSendingMessage,
    operations: selected.operations,
    input: createRef<HTMLTextAreaElement>(),
    scrolling: createConversationScroll(
      () => selected.history.loaded,
      () => !selected.history.after,
      `studio-turns:${stateDir}:${opened}`,
    ),
    start() {
      nativeDictation?.startOwner();
      const changed = (event: StorageEvent) => {
        if (
          storageEventKey(event) === messageQueue.key ||
          storageEventKey(event) === null
        )
          messageQueue.syncPending();
      };
      window.addEventListener("storage", changed);
      const stopResume = onResume(messageQueue.syncPending);
      messageQueue.syncPending();
      return () => {
        nativeDictation?.dispose();
        browserDictation.dispose();
        stopResume();
        messageQueue.active = false;
        window.removeEventListener("storage", changed);
      };
    },
  };
}
