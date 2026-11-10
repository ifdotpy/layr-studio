import { action } from "mobx";
import {
  useConversationStore,
  modelCommandPattern,
  exactModelCommandPattern,
} from "../support/ConversationStore.ts";
import { observer } from "mobx-react-lite";
import { Button, ActionIcon, Loader } from "@mantine/core";
import { ArrowUp, Square, ListEnd, Ellipsis } from "lucide-react";
import { errorText } from "../../../api.ts";
import { nativeThreadError } from "../../../nativeErrors.ts";
import { ProviderKind } from "../../../generated/providerKind.ts";
import { busy } from "../../../types.ts";
import ComposerAttachments from "../ComposerAttachments/ComposerAttachments.tsx";
import { codePointsExceed, sendsTextFile } from "../../../commands.ts";
import { usePrimaryTouch } from "../../../hooks/touchCapability.ts";
import {
  MAX_TURN_ATTACHMENTS,
  MAX_DRAFT_TEXT_CODEPOINTS,
} from "../../../generated/sharedValues.ts";
import { DeliveryMoment } from "../../../generated/deliveryMoment.ts";
import DraftVersions from "../../shell/DraftVersions/DraftVersions.tsx";
import PromptComposer from "../prompt-composer/PromptComposer/PromptComposer.tsx";
import PromptInput from "../prompt-composer/PromptInput/PromptInput.tsx";
import { Dictation } from "../Dictation/Dictation.tsx";
import RealtimeVoice from "../RealtimeVoice/RealtimeVoice.tsx";
import ConversationSettings from "../prompt-composer/ConversationSettings/ConversationSettings.tsx";
import { AccountTransferChip } from "../../accounts/AccountTransferChip/AccountTransferChip.tsx";
export function renderConversationComposerArea() {
  const control = useConversationStore();
  const { actions, uploadRecovery, opened, stateDir, input } = control.context;
  const { navigation, notices, selected, delivery, scope, layout, startup } =
    actions;
  const {
    selectedRoom: room,
    selectedAgent: agent,
    selectedLead: lead,
  } = navigation;
  const managed = control.managed;
  const { canSend, detail } = selected;
  const sending = delivery.isSending;
  const threadBlock = nativeThreadError(agent, detail);
  const services = scope.services;
  const drafts = services.drafts;
  const mobileClient = layout.isMobile;
  const touchInput = usePrimaryTouch();
  if (room) return null;
  const canStopAgent = !!agent && busy.has(agent.status || "");
  const executionControls = managed && agent && (
    <ConversationSettings
      agent={agent}
      lead={lead}
      openRequest={control.modelCommandRequest}
    />
  );
  const attachTool = managed && (
    <ComposerAttachments
      notify={notices.notify}
      assets={control.assets}
      uploading={control.uploading}
      disabled={!canSend || sending}
      add={control.addFiles}
      remove={(id) => void control.removeAttachment(id, id || "")}
    />
  );
  const voiceTool = managed &&
    agent?.isLead &&
    agent?.provider === ProviderKind.Codex &&
    opened &&
    !threadBlock && (
      <RealtimeVoice
        key={`voice:${opened}`}
        agentId={opened}
        notify={notices.notify}
      />
    );
  const transferChip = managed && agent?.isLead && (
    <ObservedAccountTransferChip transfer={agent.accountTransferProgress} />
  );
  const draftVersions = (
    <div className="composer-draft-control">
      <DraftVersions
        key={`drafts:${opened || "new"}`}
        versions={delivery.selectedDraftConflicts || []}
        useVersion={(version, mode) =>
          delivery.editDraft((current) =>
            mode === "append" && current
              ? `${current}\n\n${version.text}`
              : version.text,
          )
        }
        dismiss={(version) => drafts.dismissDraft?.(version)}
      />
    </div>
  );
  const pendingUploads = managed && (
    <>
      {!!control.pendingFiles.length && (
        <div className="attachment-list" role="status">
          {control.pendingFiles.map((file) => (
            <div className="attachment-chip" key={file.id}>
              <span>{file.name}: saved on this device, waiting for upload</span>
              <Button
                size="compact-xs"
                onClick={() =>
                  void control.removeAttachment(file.id, file.agent)
                }
              >
                Remove pending {file.name}
              </Button>
            </div>
          ))}
        </div>
      )}
      {uploadRecovery.error && (
        <p role="status">
          {uploadRecovery.error}{" "}
          <button type="button" onClick={uploadRecovery.retry}>
            Retry file upload
          </button>
        </p>
      )}
    </>
  );
  const dictation = managed && opened && (
    <Dictation
      key={`${stateDir}:${opened}`}
      chatId={`${stateDir}:${opened}`}
      disabled={!canSend || sending}
      onInsert={(text) =>
        delivery.editDraft(
          (current) =>
            current + (current && !current.endsWith("\n") ? "\n" : "") + text,
        )
      }
    />
  );
  const stopControl = canStopAgent ? (
    <ActionIcon
      type="button"
      id="stop"
      aria-label="Stop agent"
      title="Stop agent"
      disabled={control.stopShown}
      aria-busy={control.stopShown}
      onClick={control.stop}
    >
      {control.stopShown ? (
        <Loader size={14} color="currentColor" />
      ) : (
        <Square size={14} fill="currentColor" />
      )}
    </ActionIcon>
  ) : (
    <span className="composer-stop-slot" aria-hidden="true" />
  );
  return (
    <PromptComposer
      session={opened || "new"}
      getDraft={drafts.getDraft}
      subscribeDraft={drafts.subscribeDraft}
      className={control.dragging ? "attachment-drop" : ""}
      onDragOver={action((event) => {
        if (managed) {
          event.preventDefault();
          control.dragging = true;
        }
      })}
      onDragLeave={action((event) => {
        if (!event.currentTarget.contains(event.relatedTarget as Node))
          control.dragging = false;
      })}
      onDrop={action((event) => {
        event.preventDefault();
        control.dragging = false;
        void control
          .addFiles(Array.from(event.dataTransfer.files))
          .catch((error) => notices.notify(errorText(error)));
      })}
      onSubmit={(e) => {
        e.preventDefault();
        void control.submit();
      }}
    >
      {(draft) => {
        const draftTooLong = codePointsExceed(draft, MAX_DRAFT_TEXT_CODEPOINTS);
        const willAttachText = sendsTextFile(draft);
        const modelCommand =
          !willAttachText && modelCommandPattern.test(draft.trim());
        const exactModelCommand = exactModelCommandPattern.test(draft.trim());
        const queueControl = (className: string) =>
          managed ? (
            <ActionIcon
              className={className}
              type="button"
              variant="subtle"
              aria-label="Queue for next turn"
              title="Queue for the next turn (Tab)"
              aria-keyshortcuts="Tab"
              disabled={control.cannotSend(draft)}
              onClick={() => void control.submit(DeliveryMoment.NextTurn)}
            >
              <ListEnd size={18} />
            </ActionIcon>
          ) : null;
        return (
          <>
            {transferChip}
            <PromptInput
              value={draft}
              draftResetVersion={
                delivery.draftResetVersions.get(opened || "new") ?? 0
              }
              session={opened || "new"}
              getDraft={drafts.getDraft}
              setDraft={delivery.editDraft}
              input={input}
              mobile={mobileClient || touchInput}
              managed={!!managed}
              canSend={canSend}
              blocked={!!threadBlock}
              sending={sending}
              uploading={control.uploading}
              draftTooLong={draftTooLong}
              modelCommand={modelCommand}
              hasAttachments={!!control.assets.length}
              onChange={(value) => {
                control.resetRecall();
                delivery.editDraft(value);
              }}
              onPasteFiles={(files) =>
                void control
                  .addFiles(files)
                  .catch((error) => notices.notify(errorText(error)))
              }
              onSend={() => void control.submit()}
              onQueue={() => void control.submit(DeliveryMoment.NextTurn)}
              onRecallKeyDown={control.recallPrompt}
              skillCatalog={{
                enabled: !!managed && !room && !!opened && opened === agent?.id,
                agentId: opened || "",
                workspace: startup.workspaceId || stateDir,
                account: agent?.accountKey || "default",
                cwd: agent?.cwd || "",
                provider: String(agent?.provider || ""),
              }}
            />
            {exactModelCommand && (
              <Button
                type="button"
                variant="subtle"
                size="compact-sm"
                aria-label="Choose model with /model"
                onClick={() => void control.submit()}
              >
                /model · Choose model and reasoning
              </Button>
            )}
            {willAttachText && (
              <div
                id="draft-length-error"
                className="draft-length-error"
                role="alert"
              >
                <span>
                  {draftTooLong
                    ? "Draft exceeds the size limit."
                    : "This text will be sent as a file."}
                </span>
                {managed && (
                  <Button
                    type="button"
                    size="compact-xs"
                    variant="subtle"
                    disabled={
                      !canSend ||
                      sending ||
                      control.uploading ||
                      draftTooLong ||
                      control.assets.length >= MAX_TURN_ATTACHMENTS
                    }
                    onClick={() =>
                      void control.submit(DeliveryMoment.CurrentTurn, true)
                    }
                  >
                    Attach text as a file
                  </Button>
                )}
              </div>
            )}
            <div className="composer-bar">
              {draftVersions}
              <div className="composer-tools">
                {attachTool}
                <div className="composer-tools-inline">{voiceTool}</div>
                {managed && (
                  <details className="composer-tools-disclosure">
                    <summary
                      aria-label="Composer tools"
                      title="Voice conversation and queue"
                    >
                      <Ellipsis size={18} />
                    </summary>
                    <div className="composer-tools-popover">
                      {voiceTool}
                      {queueControl("composer-queue-disclosure")}
                    </div>
                  </details>
                )}
                {executionControls}

                {pendingUploads}
                {dictation}
              </div>
              <span id="send-state" role="status" aria-live="polite">
                {sending ? "Sending…" : ""}
              </span>
              <div className="composer-submit-actions">
                {managed && (
                  <span className="composer-timing">
                    <span>Send to current turn</span>
                    <span>Queue for next turn</span>
                  </span>
                )}
                {queueControl("composer-queue-primary")}
                {/* The stop slot keeps its width while idle, so Stop
                        appearing during a turn moves no other control. */}
                {stopControl}
                <ActionIcon
                  type="submit"
                  variant="filled"
                  color="gray"
                  radius="xl"
                  id="send"
                  className="send"
                  disabled={control.cannotSend(draft)}
                  aria-label="Send message"
                  title={
                    managed ? "Send after tool calls (Enter)" : "Send message"
                  }
                >
                  {sending ? (
                    <Loader size={19} color="currentColor" />
                  ) : (
                    <ArrowUp size={19} />
                  )}
                </ActionIcon>
              </div>
            </div>
          </>
        );
      }}
    </PromptComposer>
  );
}

export const ConversationComposerArea = observer(
  renderConversationComposerArea,
);

const ObservedAccountTransferChip = observer(AccountTransferChip);
