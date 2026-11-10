import { action } from "mobx";
import { useConversationStore } from "../support/ConversationStore.ts";
import { observer } from "mobx-react-lite";
import { Button, ActionIcon } from "@mantine/core";
import { Trash2 } from "lucide-react";
import { useEffect, useCallback, useMemo, useRef } from "react";
import { errorText } from "../../../api.ts";
import { currentCapacityRetry } from "../../../capacityRetry.ts";
import { nativeThreadError } from "../../../nativeErrors.ts";
import { resourceData } from "../../../store/runtime.ts";
import { TranscriptProvider } from "../support/navigation.ts";
import { serverStatus } from "../../../messageSendStatus.ts";
import {
  MessageSendStatus,
  AgentSource,
  AgentStatus,
} from "../../../generated/api.ts";
import {
  ProviderKind,
  providerSupports,
} from "../../../generated/providerKind.ts";
import { IdleCompactionNotice } from "../IdleCompactionNotice/IdleCompactionNotice.tsx";
import AgentAvatar from "../../agents/AgentAvatar/AgentAvatar.tsx";
import { useVisibleChatResult } from "../support/useChatReadState.ts";
import {
  NativeNotice,
  isNonBlockingWarning,
} from "../../shell/NativeNotice/NativeNotice.tsx";
import FullOutput from "../transcript/FullOutput/FullOutput.tsx";
import SafetyBuffering from "../transcript/SafetyBuffering/SafetyBuffering.tsx";
import { useDisplayPhase } from "../support/useDisplayPhase.ts";
import {
  deliveryLabel,
  messageRenderKey,
  explicitQueue,
} from "../support/messageDelivery.ts";
import { type Message } from "../../../types.ts";
import { SelectedTitle } from "../SelectedTitle/SelectedTitle.tsx";
import { reportPromptComposerRender } from "../../../perf/promptComposerRenderProbe.ts";
import Requests, {
  RequestArea,
  RequestCard,
} from "../questions/Requests/Requests.tsx";
import MessageDate from "../transcript/MessageDate/MessageDate.tsx";
import ClaudeRepairNotice from "../ClaudeRepairNotice/ClaudeRepairNotice.tsx";
import TurnHistory from "../transcript/TurnHistory/TurnHistory.tsx";
import OutboxControls from "../OutboxControls/OutboxControls.tsx";
import SettledDelivery from "../message-delivery/SettledDelivery/SettledDelivery.tsx";
import StreamingText from "../transcript/StreamingText/StreamingText.tsx";
import { selectedExcerpt } from "../transcript/SelectionQuote/SelectionQuote.tsx";
import MessageActions from "../transcript/MessageActions/MessageActions.tsx";
import { LiveAgentPhase } from "../../agents/AgentPhase/AgentPhase.tsx";
import { MessageAttachments } from "../ComposerAttachments/ComposerAttachments.tsx";
import { LiveConversationStart } from "../LiveConversationStart/LiveConversationStart.tsx";
export function renderConversationTranscript() {
  const control = useConversationStore();
  const { actions, opened, stateDir } = control.context;
  const { navigation, selected, read, startup, delivery, notices, layout } =
    actions;
  const {
    selectedAgent: agent,
    selectedRoom: room,
    selectedJumpTarget: jumpTarget,
  } = navigation;
  const {
    scroll,
    setFollow,
    revealRequest,
    remember,
    getFollow,
    hasRecentScrollInput,
    content,
  } = control.context.scrolling;
  const {
    visibleMessages,
    history: historySession,
    detail,
    planType,
    removed,
  } = selected;
  const { loaded, historical, after, connection, notice } = historySession;
  const { onHistoryScroll, pageAtEdge, returnToLatest } = control;
  const mobileClient = layout.isMobile;
  useVisibleChatResult(
    scroll,
    agent,
    visibleMessages,
    loaded,
    read.observeRead,
    startup.workspaceId,
    !historical && !after,
  );
  const threadBlock = nativeThreadError(agent, detail);
  const capacityRetry = agent ? currentCapacityRetry(agent) : null;
  const displayPhase = useDisplayPhase(
    opened,
    detail?.activity?.phase || "thinking",
  );
  const detailedActivity =
    !!threadBlock ||
    capacityRetry?.status === "scheduled" ||
    !!connection?.message ||
    !!delivery.offlinePending ||
    !!detail?.error ||
    !!detail?.startAttempt?.prepareError ||
    !!detail?.startAttempt?.responseError ||
    ["retrying", "auth", "safety", "error"].includes(
      detail?.activity?.phase || "",
    ) ||
    !["starting", "running", "queued", "idle", "completed"].includes(
      agent?.status || "idle",
    );
  const reasoningVisible =
    detail?.activity?.phase === "thinking" &&
    visibleMessages.at(-1)?.turnId === agent?.turnId &&
    visibleMessages.at(-1)?.role === "reasoning" &&
    typeof visibleMessages.at(-1)?.reasoningSince === "number";
  const managed = agent?.source === AgentSource.managed;
  const questionCards = useCallback(
    (item: Message) => selected.requestArea.cards(item),
    [selected],
  );
  const inlineRequestIds = new Set(
    [...control.inlineRequests.values()].flatMap((request) =>
      request ? [request.id] : [],
    ),
  );
  const renderTranscriptMessage = useCallback(
    (item: Message) =>
      item.idleCompaction ? (
        <IdleCompactionNotice key={item.id} text={item.text} />
      ) : item.nativeNotice ? (
        isNonBlockingWarning(item) ? null : (
          <ObservedNativeNotice key={item.id} item={item} planType={planType} />
        )
      ) : (
        <MessageRow key={messageRenderKey(item)} item={item} />
      ),
    [planType],
  );
  const inlineRequests = control.inlineRequests;
  const onAnswerOpen = useCallback(() => setFollow(false), [setFollow]);
  const requestCards = useMemo(
    () =>
      new Map(
        [...inlineRequests].map(([messageId, request]) => [
          messageId,
          request && (
            <RequestCard
              key={request.id}
              request={request}
              messageId={messageId}
              showDates={!!room}
            />
          ),
        ]),
      ),
    [inlineRequests, room],
  );
  const transcriptActions = useMemo(
    () => ({
      renderMessage: renderTranscriptMessage,
      questionCards,
      requestCards,
      jumpToMessage: control.jumpToPrompt,
    }),
    [renderTranscriptMessage, questionCards, requestCards, control],
  );
  const phase = (
    <LiveAgentPhase
      agentId={agent?.id}
      phaseOverride={detailedActivity ? undefined : displayPhase}
      connection={connection}
      savedMessagePending={delivery.offlinePending}
      connectionDetails={delivery.offlineDetails}
    />
  );
  const transcript = (
    <TranscriptProvider value={transcriptActions}>
      {managed && !room && agent?.provider === ProviderKind.Claude && (
        <ObservedClaudeRepairNotice agent={agent} detail={detail} />
      )}
      <TurnHistory
        key={stateDir + ":" + opened}
        history={historySession}
        items={control.renderedMessages}
        scrollContainer={scroll}
        rememberScroll={remember}
        getFollow={getFollow}
        followTail={!after}
        hasRecentScrollInput={hasRecentScrollInput}
        agent={managed && !room ? agent : undefined}
        currentTurn={agent?.turnId || undefined}
        enabled={managed && !room}
        storageKey={"studio-turns:" + stateDir + ":" + opened}
        agentId={managed ? agent?.id : undefined}
      />
    </TranscriptProvider>
  );
  const touchHistoryY = useRef<number | null>(null);
  useEffect(
    action(() => {
      if (!jumpTarget || control.handledJump === jumpTarget.requestId) return;
      control.handledJump = jumpTarget.requestId;
      void control
        .navigateToMessage(jumpTarget.messageId)
        .catch((error) => notices.notify(errorText(error)));
    }),
    [jumpTarget?.requestId, opened, control],
  );
  return (
    <div
      id="messages"
      ref={scroll}
      onScroll={onHistoryScroll}
      onWheel={action((event) => {
        control.lastHistoryTop = scroll.current?.scrollTop ?? 0;
        if (event.deltaY) pageAtEdge(event.deltaY < 0 ? "older" : "newer");
      })}
      onTouchStart={action((event) => {
        control.lastHistoryTop = scroll.current?.scrollTop ?? 0;
        touchHistoryY.current = event.touches[0]?.clientY ?? null;
      })}
      onTouchMove={(event) => {
        const y = event.touches[0]?.clientY;
        if (
          y !== undefined &&
          touchHistoryY.current !== null &&
          y !== touchHistoryY.current
        )
          pageAtEdge(y > touchHistoryY.current ? "older" : "newer");
        touchHistoryY.current = y ?? null;
      }}
      onKeyDown={action((event) => {
        control.lastHistoryTop = scroll.current?.scrollTop ?? 0;
        if (
          (event.target as HTMLElement).closest(
            "input, textarea, [contenteditable=true]",
          )
        )
          return;
        if (
          ["ArrowUp", "PageUp", "Home"].includes(event.key) ||
          (event.key === " " && event.shiftKey)
        )
          pageAtEdge("older");
        else if (["ArrowDown", "PageDown", "End", " "].includes(event.key))
          pageAtEdge("newer");
      })}
    >
      <RequestArea
        scope={stateDir}
        notify={notices.notify}
        onAnswerOpen={onAnswerOpen}
        onAnswerPosition={revealRequest}
      >
        <div ref={content} className="message-content">
          {layout.railGeometry && (
            <div className="transcript-heading">
              <SelectedTitle />
            </div>
          )}
          {removed.hasRemoved && (
            <Button size="compact-xs" onClick={() => removed.restore()}>
              Restore removed messages
            </Button>
          )}
          {notice && <p className="notice">{notice}</p>}
          {historical && (
            <p className="historical-chat-note">
              Earlier part of this chat.
              <Button
                type="button"
                size="compact-xs"
                variant="subtle"
                onClick={returnToLatest}
              >
                Return to latest messages
              </Button>
            </p>
          )}
          <PageIndicator direction="older" />
          {loaded &&
            !visibleMessages.length &&
            !notice &&
            !["running", "starting"].includes(agent?.status || "") &&
            (room ? (
              <div className="empty-chat">
                <h2>No messages yet</h2>
                <p>Agent messages will appear here.</p>
              </div>
            ) : (
              <LiveConversationStart agentId={opened || ""} />
            ))}
          {transcript}
          <PageIndicator direction="newer" />
          {!room && agent && (
            <ObservedSafetyBuffering
              key={`safety:${agent.id}`}
              agent={agent}
              detail={detail}
            />
          )}
          <div className="chat-activity">
            {!detailedActivity &&
              !reasoningVisible &&
              agent &&
              !(
                agent.status === AgentStatus.queued &&
                control.nextTurnQueue.length > 0
              ) &&
              phase}
          </div>
          {!room && detailedActivity && phase}
          <Requests
            agentId={opened}
            familyId={room?.rootId}
            includeTeam={!mobileClient}
            showDates={!!room}
            excludeIds={inlineRequestIds}
          />
        </div>
      </RequestArea>
    </div>
  );
}

export const ConversationTranscript = observer(renderConversationTranscript);

export function renderMessageRow({ item: m }: { item: Message }) {
  const control = useConversationStore();
  const {
    actions,
    opened,
    stateDir,
    scrolling: { scroll },
  } = control.context;
  const { selected, delivery, navigation, scope, notices } = actions;
  const { selectedAgent: agent, selectedRoom: room } = navigation;
  const agents = selected.team,
    outgoing = delivery.selectedOutgoing,
    managed = control.managed;
  const first =
    room?.members?.[0] ||
    selected.visibleMessages.find((message) => message.sender)?.sender;
  const canBranch =
    !room &&
    !explicitQueue(m) &&
    managed &&
    m.role === "assistant" &&
    m.turnId &&
    m.turnId !== agent?.turnId &&
    control.lastAssistantByTurn.get(m.turnId) === m.id;
  const status = serverStatus(m.deliveryStatus);
  const pending =
    (m.deliveryStatus?.source === "device" &&
      ["stored", "in_flight"].includes(m.deliveryStatus.state.kind)) ||
    status === MessageSendStatus.sending;
  const settledRemoval =
    m.localDelivery &&
    [
      MessageSendStatus.uncertain,
      MessageSendStatus.failed,
      MessageSendStatus.cancelled,
    ].some((state) => state === status);
  const remove = m.role === "user" && (pending || settledRemoval) && (
    <ActionIcon
      size="sm"
      {...(pending ? {} : { variant: "subtle" })}
      aria-label={pending ? "Remove sending message" : "Remove message"}
      title={
        pending
          ? "Remove from this device and stop retries. Work already sent continues."
          : "Remove from this device. Delivery is not cancelled."
      }
      disabled={pending && control.removingSending}
      onClick={() =>
        pending
          ? void control.removeSending([m])
          : void control.removeMessage(m)
      }
    >
      <Trash2 size={14} />
    </ActionIcon>
  );
  reportPromptComposerRender("message", m.id);
  return (
    <article
      key={messageRenderKey(m)}
      data-message={m.id}
      data-source-message={m.sourceId}
      data-found={control.highlighted === m.id || undefined}
      className={`message ${m.role === "user" ? "user" : "assistant"} ${room ? "bubble " + (m.sender !== first ? "outgoing" : "incoming") : ""}`}
    >
      {pending && (
        <SettledDelivery transient>
          <span
            className="message-delivery-status message-delivery-inline"
            role="status"
          >
            {deliveryLabel(m)}
            {remove}
          </span>
        </SettledDelivery>
      )}
      {/* The main chat has two speakers; only team rooms name each sender. */}
      {room && (
        <span className="chat-message-author">
          <AgentAvatar
            id={m.sender || agent?.id || opened || "agent"}
            size={24}
          />
        </span>
      )}
      {deliveryLabel(m) && !pending && (
        <SettledDelivery transient={status === MessageSendStatus.queued}>
          <div className="message-delivery-heading">
            {/* Delivery is the normal outcome: "Sent" stays for assistive
                  technology only, so history is not a column of "Sent". */}
            <span
              className={`message-label message-delivery-status${status === MessageSendStatus.delivered ? " sr-only" : ""}`}
              role="status"
            >
              {deliveryLabel(m)}
            </span>
            {remove}
          </div>
        </SettledDelivery>
      )}
      {m.role === "user" ? (
        <div className="prose plain">{m.text}</div>
      ) : (
        <StreamingText
          message={m}
          streaming={!!m.streaming}
          agentId={
            room
              ? agents.find((item) => item.id === m.sender)?.id
              : managed
                ? agent?.id
                : undefined
          }
        />
      )}
      {Array.isArray(m.assets) && (
        <MessageAttachments assets={m.assets} notify={notices.notify} />
      )}
      {m.localDelivery && (
        <OutboxControls
          entry={outgoing?.find((entry) => entry.id === m.clientMessageId)}
          onResolved={(id) => delivery.observeSends?.([id])}
        />
      )}
      {m.deliveryError &&
        !(
          delivery.offlinePending &&
          m.localDelivery &&
          m.deliveryStatus?.source === "device" &&
          m.deliveryStatus.state.kind === "transport_failed"
        ) &&
        (status === MessageSendStatus.failed ||
          status === MessageSendStatus.uncertain ||
          (m.deliveryStatus?.source === "device" &&
            (m.deliveryStatus.state.kind === "transport_failed" ||
              m.deliveryStatus.state.kind === "receipt_expired"))) && (
          <div className="message-delivery-error">
            <p>{m.deliveryError}</p>
            {status === MessageSendStatus.failed && (
              <Button
                size="compact-xs"
                variant="subtle"
                onClick={() => control.restoreDraft(m)}
              >
                Restore draft
              </Button>
            )}
          </div>
        )}
      {m.truncated && <p className="notice">This message is clipped.</p>}
      {m.fullTextBlob && <FullOutput blob={m.fullTextBlob} />}
      {(room || m.role === "user") && (
        <MessageDate at={m.at ?? m.created ?? m.timestamp} />
      )}
      <MessageActions
        hidden={!!m.streaming}
        info={
          m.role === "assistant"
            ? {
                message: m,
                agentId: !room && managed ? agent?.id : undefined,
                stateDir: stateDir,
                rootId:
                  !room && managed ? agent?.rootId || agent?.id : undefined,
                accounts:
                  resourceData(
                    scope.store.state().current,
                  )?.accountsSummary.get("current")?.accounts || [],
                onCopy: (value) => void control.copy(value),
              }
            : undefined
        }
        onCopy={() => void control.copy(m.text)}
        onQuote={
          !room && !explicitQueue(m)
            ? () => {
                const excerpt = selectedExcerpt(scroll.current);
                control.quote(
                  excerpt?.messageId === m.id ? excerpt.text : m.text,
                );
                window.getSelection()?.removeAllRanges();
              }
            : undefined
        }
        onEdit={
          !room &&
          !explicitQueue(m) &&
          managed &&
          m.role === "user" &&
          m.turnId &&
          m.turnStatus !== "uncertain" &&
          (!m.deliveryStatus || status === MessageSendStatus.delivered)
            ? () => void control.editMessage(m)
            : undefined
        }
        editLoading={control.editLoading === m.id}
        editDisabled={
          control.branching ||
          !!control.editLoading ||
          m.turnId === agent?.turnId
        }
        onBranch={
          canBranch &&
          providerSupports(agent?.provider, "fork") &&
          m.turnStatus !== "uncertain"
            ? () => void control.branch(m)
            : undefined
        }
        branchDisabled={m.turnId === agent?.turnId && !!agent?.inFlight}
        onAnotherAnswer={
          canBranch
            ? action(
                () =>
                  (control.branchDraft = {
                    message: m,
                    before: false,
                    text: "Give another answer to my previous request. Use the existing results. Do not run tools or commands unless I explicitly ask.",
                  }),
              )
            : undefined
        }
        anotherAnswerDisabled={control.branching}
      />
    </article>
  );
}

const MessageRow = observer(renderMessageRow);

const PageIndicator = observer(function PageIndicator({
  direction,
}: {
  direction: "older" | "newer";
}) {
  const control = useConversationStore();
  const status = control.status;
  if (!status || status.direction !== direction) return null;
  return status.failed ? (
    <Button size="compact-sm" onClick={() => void control.loadPage(direction)}>
      Retry loading {direction === "older" ? "earlier" : "later"} messages
    </Button>
  ) : (
    <p className="transcript-page-link" role="status">
      Loading messages…
    </p>
  );
});

const ObservedClaudeRepairNotice = observer(ClaudeRepairNotice);

const ObservedSafetyBuffering = observer(SafetyBuffering);

const ObservedNativeNotice = observer(NativeNotice);
