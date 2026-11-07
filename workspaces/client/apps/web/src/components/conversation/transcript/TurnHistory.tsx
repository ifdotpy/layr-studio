import { computed } from "mobx";
import { useConversationActions } from "../navigation";
import { HistoryLiveText } from "../../../conversation/liveText";
import type { createLiveText } from "../../../conversation/liveText";
import { observer } from "mobx-react-lite";
import { reportPromptComposerRender } from "../../prompt-composer/renderProbe";
import {
  Fragment,
  useCallback,
  useMemo,
  useLayoutEffect,
  useRef,
  useState,
  type ReactNode,
  type RefObject,
} from "react";
import {
  ChevronRight,
  CircleAlert,
  CircleStop,
  LoaderCircle,
  Wrench,
} from "lucide-react";
import { save, saved } from "../../../api";
import type { Agent, Message } from "../../../types";
import { useTurnErrors } from "../../useTurnErrors";
import Activity, {
  ToolCard,
  activitySummary,
  isFileChange,
  isPastCommand,
} from "./Activity";
import { toolLimitNotice } from "../../toolLimitNotice";
import { turnFailureReason } from "../../turnFailureReason";
import ConversationResults from "./ConversationResults";
import {
  historyPresentationGroups,
  historyGroups,
  isEmptyAssistantMessage,
  type HistoryGroup,
} from "../../turnHistoryModel";
import "./turn-history.css";
import { messageRenderKey } from "../../message-delivery/messageDelivery";
import ReasoningDuration from "./ReasoningDuration";
import ProviderUnknownItem, { providerUnknown } from "./ProviderUnknownItem";
import HistoryWindow from "./HistoryWindow";
import { windowHistoryRows } from "./historyWindowModel";
import QuestionCard from "../../questions/QuestionCard";
import { openRequestAnswer } from "../../questions/Requests";
import {
  isQuestionToolItem,
  questionCard,
} from "../../questions/questionCardModel";

function useTranscriptActions() {
  const actions = useConversationActions().transcript;
  if (!actions)
    throw new Error("Transcript actions must be provided by the conversation");
  return actions;
}

// Group presentation depends on model fields, but unchanged membership/order and
// outcome must keep the list asleep while a displayed text field changes.
function sameGroups(left: HistoryGroup[], right: HistoryGroup[]): boolean {
  return (
    left.length === right.length &&
    left.every((group, index) => {
      const next = right[index]!;
      return (
        group.id === next.id &&
        group.outcome === next.outcome &&
        group.result === next.result &&
        group.items.length === next.items.length &&
        group.items.every((item, row) => item === next.items[row]) &&
        (group.turns || next.turns
          ? !!group.turns && !!next.turns && sameGroups(group.turns, next.turns)
          : true)
      );
    })
  );
}

type CommandVisibility = boolean | ReadonlyMap<string, boolean>;
type RequestCards = ReadonlyMap<string, ReactNode>;

const answerQuestion = (requestId: string) => {
  openRequestAnswer(requestId);
};

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
      onAnswer={answerQuestion}
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

/** Rows that join the collapsible work log. Question cards stay visible. */
const workRow = (item: Message) =>
  ["tool", "output"].includes(item.role) &&
  !isFileChange(item) &&
  !isQuestionToolItem(item);

function messageGroups(
  items: Message[],
  showCompletedCommands: CommandVisibility = false,
  includeReasoning = false,
  revealedMessage?: string,
  requestCards?: RequestCards,
) {
  const joinsWork = (item: Message) =>
    !requestCards?.has(item.id) &&
    (workRow(item) || (includeReasoning && item.role === "reasoning"));
  const showItem = (item: Message) =>
    item.id === revealedMessage ||
    (typeof showCompletedCommands === "boolean"
      ? showCompletedCommands
      : item.turnId
        ? (showCompletedCommands.get(item.turnId) ?? true)
        : true);
  const groups: (Message | [Message, ...Message[]])[] = [];
  for (let index = 0; index < items.length; index++) {
    const item = items[index]!;
    if (!requestCards?.has(item.id) && isEmptyAssistantMessage(item)) continue;
    if (!requestCards?.has(item.id) && !showItem(item) && isPastCommand(item))
      continue;
    if (joinsWork(item)) {
      const run: [Message, ...Message[]] = [item];
      while (index + 1 < items.length) {
        const next = items[index + 1]!;
        if (
          (!requestCards?.has(next.id) && isEmptyAssistantMessage(next)) ||
          (!requestCards?.has(next.id) &&
            !showItem(next) &&
            isPastCommand(next))
        ) {
          index++;
          continue;
        }
        if (!joinsWork(next)) break;
        run.push(next);
        index++;
      }
      if (run.some((entry) => ["tool", "output"].includes(entry.role)))
        groups.push(run);
      else groups.push(...run);
    } else groups.push(item);
  }
  return groups;
}

function messages(
  items: Message[],
  agentId?: string,
  cwd?: string,
  revealedMessage?: string,
  requestCards?: RequestCards,
) {
  return messageGroups(items, false, false, revealedMessage, requestCards).map(
    (item) =>
      Array.isArray(item) ? (
        <Activity key={item[0].id} items={item} agentId={agentId} />
      ) : (
        <Fragment key={messageRenderKey(item)}>
          <TranscriptRow
            item={item}

            agentId={agentId}
            cwd={cwd}
          />
        </Fragment>
      ),
  );
}

const WorkBlock = observer(function WorkBlock({
  items,
  storageKey,
  storageIds,
  agentId,
  active,
}: {
  items: [Message, ...Message[]];
  storageKey: string;
  storageIds: string[];
  agentId?: string | undefined;
  active: boolean;
}) {
  const key = `${storageKey}:tools-v3`;
  const tools = items.filter((item) => ["tool", "output"].includes(item.role));
  const id = storageIds[0] || items[0].id;
  const [open, setOpen] = useState(
    () =>
      saved<Record<string, boolean>>(key, {})[id] ??
      (active || tools.length < 3),
  );
  const summary = activitySummary(tools);
  const hasLimit = tools.some((item) => toolLimitNotice(item));
  const update = (value: boolean) => {
    setOpen(value);
    save(
      key,
      Object.fromEntries([
        ...Object.entries(saved<Record<string, boolean>>(key, {}))
          .filter(([entry]) => !storageIds.includes(entry))
          .slice(-499),
        ...storageIds.map((entry) => [entry, value]),
      ]),
    );
  };
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
            <strong className="activity-limit">Account limit reached · </strong>
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
});

const Turn = observer(function Turn({
  group,
  storageKey,
  agentId,
  cwd,
  failureReason,
  failurePending,
  failureLookupFailed,
  retryFailure,
  revealedMessage,
}: {
  group: HistoryGroup;
  storageKey: string;
  agentId?: string | undefined;
  cwd?: string | undefined;
  failureReason: string;
  failurePending: boolean;
  failureLookupFailed: boolean;
  retryFailure: () => void;
  revealedMessage?: string | undefined;
}) {
  const { requestCards, jumpToMessage } = useTranscriptActions();
  const result = group.result;
  const turns = group.turns || [group];
  const [savedCommandVisibility, setSavedCommandVisibility] = useState(() => {
    const visibility = new Map<string, boolean>();
    for (const turn of turns) {
      const turnId = turn.items[0].turnId;
      if (turnId) visibility.set(turnId, !turn.outcome);
    }
    return visibility;
  });
  let commandVisibility = savedCommandVisibility;
  for (const turn of turns) {
    const turnId = turn.items[0].turnId;
    if (!turnId || commandVisibility.has(turnId)) continue;
    commandVisibility = new Map(commandVisibility).set(turnId, !turn.outcome);
  }
  if (commandVisibility !== savedCommandVisibility)
    setSavedCommandVisibility(commandVisibility);
  const groupedMessages = useMemo(
    () =>
      computed(() =>
        messageGroups(
          group.items,
          commandVisibility,
          true,
          revealedMessage,
          requestCards,
        ),
      ),
    [group.items, commandVisibility, revealedMessage, requestCards],
  ).get();
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
  return (
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
      {groupedMessages.map((item) =>
        Array.isArray(item) ? (
          <WorkBlock
            key={item[0].id}
            items={item}
            storageKey={storageKey}
            storageIds={turns.flatMap((turn) => {
              const turnId = turn.items[0].turnId;
              const contributor = item.find((entry) => entry.turnId === turnId);
              return contributor ? [contributor.id] : [];
            })}
            agentId={agentId}
            active={!group.outcome}
          />
        ) : (
          <div
            key={item.id}
            className={item.id === result?.id ? "turn-answer" : undefined}
          >
            <TranscriptRow
              item={item}

              agentId={agentId}
              cwd={cwd}
            />
          </div>
        ),
      )}
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
              Review the saved evidence before you continue. This turn will not
              be replayed.
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
    </section>
  );
});

const HistoryContent = observer(function HistoryContent({
  group,
  revealedMessage,
  enabled,
  agentId,
  cwd,
  storageKey,
  failurePending,
  failureLookupFailed,
  failureReason,
  retry,
}: {
  group: HistoryGroup;
  revealedMessage?: string | undefined;
  enabled: boolean;
  agentId?: string | undefined;
  cwd?: string | undefined;
  storageKey: string;
  failurePending: boolean;
  failureLookupFailed: boolean;
  failureReason: string;
  retry: (turn: string) => void;
}) {
  const { requestCards } = useTranscriptActions();
  if (typeof window !== "undefined" && window.__studioPromptComposerRenderProbe)
    reportPromptComposerRender("history-row", group.id);
  const turnId = (group.turns?.at(-1) || group).items[0].turnId;
  if (
    !enabled ||
    (group.items[0].role === "user" && group.outcome !== "uncertain") ||
    typeof group.items[0].turnId !== "string" ||
    typeof turnId !== "string"
  )
    return (
      <Fragment key={messageRenderKey(group.items[0])}>
        {messages(group.items, agentId, cwd, revealedMessage, requestCards)}
      </Fragment>
    );
  return (
    <Turn
      key={group.id}
      group={group}
      revealedMessage={revealedMessage}
      failurePending={failurePending}
      failureLookupFailed={failureLookupFailed}
      retryFailure={() => retry(turnId)}
      failureReason={failureReason}
      storageKey={storageKey}

      agentId={agentId}
      cwd={cwd}
    />
  );
});

function TurnHistory({
  history,
  items: sourceItems,
  currentTurn,
  enabled,
  storageKey,
  agentId,
  agent,
  scrollContainer,
  rememberScroll,
  getFollow,
  followTail = true,
  hasRecentScrollInput,
}: {
  history?:
    | { readonly live: ReturnType<typeof createLiveText> | null }
    | undefined;
  items: Message[];
  agent?: Agent | undefined;
  currentTurn?: string | undefined;
  enabled: boolean;
  storageKey: string;
  agentId?: string | undefined;
  scrollContainer?: RefObject<HTMLDivElement | null>;
  rememberScroll?: () => void;
  getFollow?: () => boolean;
  followTail?: boolean;
  hasRecentScrollInput?: () => boolean;
}) {
  const { requestCards } = useTranscriptActions();
  reportPromptComposerRender("turn-history", agentId);
  const { items, loading, failed, retry } = useTurnErrors(
    sourceItems,
    enabled ? agent : undefined,
    storageKey,
  );
  const groups = useMemo(
    () =>
      computed(
        () => {
          reportPromptComposerRender("history-groups", agentId);
          return historyPresentationGroups(historyGroups(items, currentTurn));
        },
        { equals: sameGroups },
      ),
    [items, currentTurn, agentId],
  ).get();
  const failureReasonEntries = useMemo(
    () =>
      computed(() => {
        const failedTurns = new Set(
          groups
            .flatMap((group) => group.turns || [group])
            .filter((turn) =>
              ["failed", "uncertain"].includes(turn.outcome || ""),
            )
            .flatMap((turn) => {
              const turnId = turn.items[0].turnId;
              return turnId && typeof turnId === "string" ? [turnId] : [];
            }),
        );
        const byTurn = new Map<string, Message[]>();
        for (const item of items) {
          if (typeof item.turnId !== "string") continue;
          if (!failedTurns.has(item.turnId)) continue;
          const turn = byTurn.get(item.turnId) || [];
          turn.push(item);
          byTurn.set(item.turnId, turn);
        }
        return [...byTurn]
          .map(
            ([turn, messages]) =>
              [turn, turnFailureReason(messages)] as [string, string],
          )
          .sort(([a], [b]) => a.localeCompare(b));
      }),
    [items, groups],
  ).get();
  // Keep the map identity stable across streamed deltas with unchanged failures.
  const failureReasonKey = failureReasonEntries
    .map(([turn, reason]) => `${turn.length}:${turn}${reason.length}:${reason}`)
    .join("|");
  const failureReasons = useMemo(
    () => new Map(failureReasonEntries),
    [failureReasonKey],
  );
  const previousRows = useRef<HistoryGroup[]>([]);
  const rows = useMemo(() => {
    const partitioned = scrollContainer
      ? windowHistoryRows(groups, previousRows.current)
      : groups;
    return partitioned;
  }, [groups, scrollContainer]);
  useLayoutEffect(() => {
    previousRows.current = rows;
  }, [rows]);
  const requiredRows = useMemo(
    () =>
      rows
        .filter((row) => row.items.some((item) => requestCards?.has(item.id)))
        .map((row) => row.id),
    [rows, requestCards],
  );
  const renderRow = useCallback(
    (group: HistoryGroup, revealedMessage?: string) => {
      const turnId = (group.turns?.at(-1) || group).items[0].turnId;
      return (
        <HistoryContent
          group={group}
          revealedMessage={revealedMessage}
          enabled={enabled}

          agentId={agentId}
          cwd={agent?.cwd ?? undefined}
          storageKey={storageKey}

          failurePending={
            typeof turnId === "string" && loading.includes(turnId)
          }
          failureLookupFailed={
            typeof turnId === "string" && failed.includes(turnId)
          }
          failureReason={
            typeof turnId === "string" ? failureReasons.get(turnId) || "" : ""
          }
          retry={retry}
        />
      );
    },
    [
      enabled,
      requestCards,
      agentId,
      agent?.cwd,
      storageKey,
      loading,
      failed,
      failureReasons,
      retry,
    ],
  );
  return (
    <HistoryLiveText.Provider value={history?.live || null}>
      <HistoryWindow
        rows={rows}
        requiredRows={requiredRows}
        storageKey={storageKey}
        rememberScroll={rememberScroll}
        getFollow={getFollow}
        followTail={followTail}
        hasRecentScrollInput={hasRecentScrollInput}
        scrollContainer={scrollContainer}
        render={renderRow}
      />
    </HistoryLiveText.Provider>
  );
}

export default observer(TurnHistory);
