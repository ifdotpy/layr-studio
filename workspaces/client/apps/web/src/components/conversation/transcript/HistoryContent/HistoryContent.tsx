import { ownedView } from "../support/ownedView.tsx";
import {
  TranscriptStore,
  turnPresentation,
  WorkLog,
  messageGroups,
} from "../../support/TurnHistoryStore.ts";
import { useTranscriptActions } from "../../support/navigation.ts";
import { observer } from "mobx-react-lite";
import { reportPromptComposerRender } from "../../../../perf/promptComposerRenderProbe.ts";
import { Fragment } from "react";
import {
  ChevronRight,
  CircleAlert,
  CircleStop,
  LoaderCircle,
  Wrench,
} from "lucide-react";
import type { Message } from "../../../../types.ts";
import Activity, { ToolCard, isFileChange } from "../Activity/Activity.tsx";
import { turnFailureReason } from "../../support/turnFailureReason.ts";
import ConversationResults from "../ConversationResults/ConversationResults.tsx";
import { type HistoryGroup } from "../../support/turnHistoryModel.ts";

import { messageRenderKey } from "../../support/messageDelivery.ts";
import ReasoningDuration from "../ReasoningDuration/ReasoningDuration.tsx";
import ProviderUnknownItem, {
  providerUnknown,
} from "../ProviderUnknownItem/ProviderUnknownItem.tsx";
import QuestionCard from "../../questions/QuestionCard/QuestionCard.tsx";
import { openRequestAnswer } from "../../questions/Requests/Requests.tsx";
import { questionCard } from "../../support/questionCardModel.ts";

/** A transcript row: a question card replaces its tool row and follows its message. */
const TranscriptRow = observer(function TranscriptRow({
  item,
  agentId,
  cwd,
}: {
  item: Message;
  agentId?: string | undefined;
  cwd?: string | undefined;
}) {
  const {
    renderMessage: render,
    questionCards: questions = questionCard,
    requestCards,
  } = useTranscriptActions();
  if (requestCards?.has(item.id)) return requestCards.get(item.id);
  const unknown = providerUnknown(item);
  if (unknown) return <ProviderUnknownItem {...unknown} />;
  if (isFileChange(item))
    return <ToolCard item={item} agentId={agentId} cwd={cwd} />;
  if (item.role === "reasoning") return <ReasoningDuration item={item} />;
  const card = questions(item);
  if (!card) return render(item);
  const question = (
    <QuestionCard
      card={card}
      messageId={item.id}
      messageText={card.source === "message" ? item.text : undefined}
      onAnswer={openRequestAnswer}
    />
  );
  if (card.source === "tool") return question;
  return (
    <>
      {render(item)}
      {question}
    </>
  );
});

const WorkBlock = ownedView(
  (props: {
    items: [Message, ...Message[]];
    storageKey: string;
    storageIds: string[];
    agentId?: string | undefined;
    active: boolean;
  }) =>
    new WorkLog(
      `${props.storageKey}:tools-v3`,
      props.storageIds[0] || props.items[0].id,
      props.active,
      props.items.filter((item) => ["tool", "output"].includes(item.role))
        .length,
    ),
  function WorkBlock({ items, storageKey, storageIds, agentId, model }) {
    const { open } = model;
    const { summary, hasLimit } = model.summary(items);
    const update = (value: boolean) =>
      model.update(value, `${storageKey}:tools-v3`, storageIds);
    return (
      <details
        className="turn-work"
        data-running={summary.running}
        data-failed={summary.failed}
        open={open}
        onToggle={(event) => {
          if (
            event.target === event.currentTarget &&
            event.currentTarget.open !== open
          )
            update(event.currentTarget.open);
        }}
      >
        <summary
          aria-label={`Work log: ${summary.label || "Agent updates"}`}
          onClick={(event) => {
            event.preventDefault();
            update(!open);
          }}
        >
          <ChevronRight size={14} className="turn-expand-icon" />
          {summary.running ? (
            <LoaderCircle size={14} className="spin" />
          ) : (
            <Wrench size={14} />
          )}
          <span className="turn-work-label">
            {hasLimit && (
              <strong className="activity-limit">
                Account limit reached ·{" "}
              </strong>
            )}
            {summary.label || "Agent updates"}
          </span>
          {!!summary.running && (
            <span className="activity-running">{summary.running} running</span>
          )}
          {!!summary.failed && (
            <span className="activity-failed">{summary.failed} failed</span>
          )}
        </summary>
        <div className="turn-work-body">
          {items.map((item) => {
            return open ? (
              item.role === "reasoning" ? (
                <ReasoningDuration key={item.id} item={item} />
              ) : (
                <ToolCard key={item.id} item={item} agentId={agentId} />
              )
            ) : (
              <span
                key={item.id}
                data-message={item.id}
                data-source-message={item.sourceId}
                data-lazy-message
                hidden
              />
            );
          })}
        </div>
      </details>
    );
  },
);

export const HistoryContent = ownedView(
  (_props: {
    store: TranscriptStore;
    group: HistoryGroup;
    revealedMessage?: string | undefined;
  }) => turnPresentation(),
  function HistoryContent({ store, group, revealedMessage, model }) {
    const { enabled, agentId, storageKey, agent } = store.presentation;
    const cwd = agent?.cwd ?? undefined;
    const { items, loading, failed, retry } = store.errorView;
    const { requestCards, jumpToMessage } = useTranscriptActions();
    if (
      typeof window !== "undefined" &&
      window.__studioPromptComposerRenderProbe
    )
      reportPromptComposerRender("history-row", group.id);
    const turnId = (group.turns?.at(-1) || group).items[0].turnId;
    const structured =
      enabled &&
      (group.items[0].role !== "user" || group.outcome === "uncertain") &&
      typeof group.items[0].turnId === "string" &&
      typeof turnId === "string";
    const failurePending = structured && loading.includes(turnId);
    const failureLookupFailed = structured && failed.includes(turnId);
    const retryFailure = () => retry(turnId ?? "");
    const failureReason =
      structured && ["failed", "uncertain"].includes(group.outcome || "")
        ? turnFailureReason(items.filter((item) => item.turnId === turnId))
        : "";
    const result = group.result;
    const turns = group.turns || [group];
    const groupedMessages = structured
      ? model(group, revealedMessage, requestCards)
      : messageGroups(group.items, false, false, revealedMessage, requestCards);
    const visibleError = group.items.some(
      (item) =>
        item.nativeNotice === "error" ||
        (item.nativeError && !["tool", "output"].includes(item.role)),
    );
    if (
      !groupedMessages.length &&
      !["failed", "interrupted", "uncertain"].includes(group.outcome || "")
    )
      return null;
    const content = (
      <>
        {groupedMessages.map((item) => {
          if (Array.isArray(item))
            return structured ? (
              <WorkBlock
                key={item[0].id}
                items={item}
                storageKey={storageKey}
                storageIds={turns.flatMap((turn) => {
                  const contributor = item.find(
                    (entry) => entry.turnId === turn.items[0].turnId,
                  );
                  return contributor ? [contributor.id] : [];
                })}
                agentId={agentId}
                active={!group.outcome}
              />
            ) : (
              <Activity key={item[0].id} items={item} agentId={agentId} />
            );
          const row = <TranscriptRow item={item} agentId={agentId} cwd={cwd} />;
          return structured ? (
            <div
              key={item.id}
              className={item.id === result?.id ? "turn-answer" : undefined}
            >
              {row}
            </div>
          ) : (
            <Fragment key={messageRenderKey(item)}>{row}</Fragment>
          );
        })}
        {structured && (
          <>
            {group.outcome === "failed" &&
              !visibleError &&
              (failurePending ? (
                <div
                  className="turn-error-pending"
                  role="status"
                  aria-label="Loading error details"
                >
                  <span />
                  <span />
                  <span />
                </div>
              ) : (
                <p className="turn-problem" role="status">
                  <CircleAlert size={14} />
                  <span>
                    {failureLookupFailed
                      ? "Could not load error details."
                      : failureReason}
                  </span>
                  {failureLookupFailed && (
                    <button className="turn-error-retry" onClick={retryFailure}>
                      Retry
                    </button>
                  )}
                </p>
              ))}
            {group.outcome === "uncertain" && (
              <div className="turn-problem" role="status">
                <CircleAlert size={14} />
                <div>
                  <strong>Outcome unconfirmed</strong>
                  <p>{failureReason}</p>
                  <p>
                    Review the saved evidence before you continue. This turn
                    will not be replayed.
                  </p>
                </div>
              </div>
            )}
            {group.outcome === "interrupted" && (
              <p className="turn-problem">
                <CircleStop size={14} />
                Turn interrupted
              </p>
            )}
            <ConversationResults
              messages={group.items}
              agentId={agentId}
              onJump={jumpToMessage}
            />
          </>
        )}
      </>
    );
    return structured ? (
      <section
        className="turn-history"
        data-turn={
          !group.outcome ||
          ["failed", "interrupted", "uncertain"].includes(group.outcome)
            ? turns.at(-1)?.items[0].turnId
            : group.items[0].turnId
        }
        data-turns={turns
          .map((turn) => turn.items[0].turnId)
          .filter(Boolean)
          .join(" ")}
        data-outcome={group.outcome || "active"}
      >
        {content}
      </section>
    ) : (
      content
    );
  },
);
