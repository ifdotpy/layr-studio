import { FullOutputs } from "./FullOutputs.ts";
import { anchors } from "./historyAnchorStore.ts";
import {
  historyOffsets,
  HISTORY_WINDOW_THRESHOLD,
} from "./historyWindowModel.ts";
import { messageRenderKey } from "./messageDelivery.ts";
import type { RefObject } from "react";
import type { ConversationStore } from "./ConversationStore.ts";
import type { Agent } from "../../../types.ts";
import type { Net } from "../../../servers/net/net.ts";
import type { createServerStore } from "../../../store/server.ts";
import type { createLiveText } from "../../../conversation/liveText.ts";
import {
  compareShallow,
  compareStructural,
  observableRef,
  observableShallow,
  computed,
  makeAutoObservable,
  untracked,
} from "mobx";
import { derived } from "./derived.ts";
import { reportPromptComposerRender } from "../../../perf/promptComposerRenderProbe.ts";
import type { Message } from "../../../types.ts";
import {
  historyPresentationGroups,
  historyGroups,
  isEmptyAssistantMessage,
  type HistoryGroup,
} from "./turnHistoryModel.ts";
import { windowHistoryRows } from "./historyWindowModel.ts";
import { isQuestionToolItem } from "./questionCardModel.ts";
import {
  isFileChange,
  isPastCommand,
} from "../transcript/Activity/Activity.tsx";
import { save, saved } from "../../../api.ts";
import type { ReactNode } from "react";
import { activitySummary } from "../transcript/Activity/Activity.tsx";
import { toolLimitNotice } from "./toolLimitNotice.ts";
import { TurnErrorStore } from "./useTurnErrors.ts";
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
        compareShallow(group.items, next.items) &&
        (group.turns || next.turns
          ? !!group.turns && !!next.turns && sameGroups(group.turns, next.turns)
          : true)
      );
    })
  );
}

type CommandVisibility = boolean | ReadonlyMap<string, boolean>;
type RequestCards = ReadonlyMap<string, ReactNode>;

/** Rows that join the collapsible work log. Question cards stay visible. */
const workRow = (item: Message) =>
  ["tool", "output"].includes(item.role) &&
  !isFileChange(item) &&
  !isQuestionToolItem(item);

export function messageGroups(
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
  let work: [Message, ...Message[]] | undefined;
  const flush = () => {
    if (!work) return;
    if (work.some((item) => ["tool", "output"].includes(item.role)))
      groups.push(work);
    else groups.push(...work);
    work = undefined;
  };
  for (const item of items) {
    if (
      !requestCards?.has(item.id) &&
      (isEmptyAssistantMessage(item) ||
        (!showItem(item) && isPastCommand(item)))
    )
      continue;
    if (joinsWork(item)) {
      if (work) work.push(item);
      else work = [item];
    } else {
      flush();
      groups.push(item);
    }
  }
  flush();
  return groups;
}

export type TranscriptSource = {
  items: Message[];
  enabled: boolean;
  storageKey: string;
  agent?: Agent | undefined;
  agentId?: string | undefined;
  currentTurn?: string | undefined;
  live?: ReturnType<typeof createLiveText> | null | undefined;
  scrollContainer?: RefObject<HTMLDivElement | null> | undefined;
  rememberScroll?: (() => void) | undefined;
  getFollow?: (() => boolean) | undefined;
  followTail?: boolean | undefined;
  hasRecentScrollInput?: (() => boolean) | undefined;
  requestCards?: RequestCards | undefined;
  server?: ReturnType<typeof createServerStore>;
  net?: Net;
};

/** Presentation is computed from its conversation owner, without React prop mirrors. */
export class TranscriptStore {
  readonly outputs = new FullOutputs();
  private source: () => TranscriptSource;
  private previousRows: HistoryGroup[] = [];
  private opening = derived(
    (_server: TranscriptSource["server"], _scope: string) =>
      new TurnErrorStore(untracked(() => this.input.items)),
  );
  heights = new Map<string, number>();
  details = new Map<string, boolean>();
  visible: string[];
  requested: string | null;
  revealedMessage: string | undefined;
  pinned: string[] = [];
  readonly initialAnchor;
  constructor(source: () => TranscriptSource) {
    this.source = source;
    this.visible = this.rows.slice(-12).map((row) => row.id);
    this.initialAnchor = anchors.get(source().storageKey) || null;
    this.requested = this.initialAnchor?.id || null;
    makeAutoObservable<this, "source" | "previousRows" | "opening">(
      this,
      {
        anchor: false,
        details: false,
        initialAnchor: false,
        heights: observableShallow,
        visible: observableRef,
        pinned: observableRef,
        geometry: computed({ equals: compareStructural }),
        indices: computed({ equals: compareShallow }),
        source: false,
        previousRows: false,
        opening: false,
        groups: computed({ equals: sameGroups }),
        requiredRows: computed({ equals: compareShallow }),
        outputs: false,
        presentation: computed({ equals: compareShallow }),
      },
      { autoBind: true, deep: false },
    );
  }
  get input() {
    return this.source();
  }
  get errors() {
    const { server, storageKey, agent, enabled } = this.input;
    return this.opening(
      server,
      JSON.stringify([storageKey, agent?.id, agent?.threadId, enabled]),
    );
  }
  get errorView() {
    const { items, agent, enabled, server, net } = this.input;
    return this.errors.read(items, enabled ? agent : undefined, server, net);
  }
  get groups() {
    reportPromptComposerRender("history-groups", this.input.agentId);
    return historyPresentationGroups(
      historyGroups(this.errorView.items, this.input.currentTurn),
    );
  }
  get rows() {
    return (this.previousRows = this.input.scrollContainer
      ? windowHistoryRows(this.groups, this.previousRows)
      : this.groups);
  }
  get requiredRows() {
    return this.rows
      .filter((row) =>
        row.items.some((item) => this.input.requestCards?.has(item.id)),
      )
      .map((row) => row.id);
  }
  anchor: () => string | undefined = () => this.initialAnchor?.id;
  get presentation() {
    const { enabled, agentId, storageKey, agent, live } = this.input;
    return { enabled, agentId, storageKey, agent, live };
  }
  get geometry() {
    return measure(this.rows);
  }
  get rowsById() {
    return new Map(this.rows.map((row) => [row.id, row]));
  }
  get active() {
    return (
      !!this.input.scrollContainer &&
      (this.rows.length > HISTORY_WINDOW_THRESHOLD || this.geometry.count > 256)
    );
  }
  get offsets() {
    const estimates = this.geometry.estimates;
    const offsets = [0];
    this.rows.forEach((row, index) =>
      offsets.push(
        offsets[index]! +
          (this.heights.get(row.id) ??
            estimates[index + 1]! - estimates[index]!),
      ),
    );
    return offsets;
  }
  get indices() {
    const { anchorRows } = this.geometry;
    const keep = new Set([
      ...this.visible,
      ...this.pinned,
      ...this.requiredRows,
      ...[this.requested, this.anchor()].flatMap((id) =>
        id ? [anchorRows.get(id) || id] : [],
      ),
    ]);
    return this.rows.flatMap((row, index) => (keep.has(row.id) ? [index] : []));
  }
  setVisible(ids: string[]) {
    if (!sameIds(this.visible, ids)) this.visible = ids;
  }
  setPinned(ids: string[]) {
    if (!sameIds(this.pinned, ids)) this.pinned = ids;
  }
  request(id: string | null) {
    this.requested = id;
  }
  reveal(id?: string) {
    this.revealedMessage = id;
  }
  measure(id: string, height: number) {
    this.heights.set(id, height);
  }
}

export function conversationTranscriptSource(
  control: ConversationStore,
): TranscriptSource {
  const {
    context: { actions, opened, stateDir, scrolling },
    agent,
    managed,
  } = control;
  const enabled = managed && !actions.navigation.selectedRoom;
  return {
    getFollow: scrolling.getFollow,
    hasRecentScrollInput: scrolling.hasRecentScrollInput,
    scrollContainer: scrolling.scroll,
    rememberScroll: scrolling.remember,
    items: control.renderedMessages,
    agent: enabled ? agent : undefined,
    currentTurn: agent?.turnId ?? undefined,
    enabled,
    storageKey: `studio-turns:${stateDir}:${opened}`,
    agentId: managed ? agent?.id : undefined,
    live: control.session.live,
    followTail: !control.session.after,
    requestCards: control.transcriptActions.requestCards,
    net: actions.scope.services.net,
    server: actions.scope.store,
  };
}

/** Commands already visible when a turn first appears stay visible after it ends. */
export function turnPresentation() {
  const visibility = new Map<string, boolean>();
  return derived(
    (
      group: HistoryGroup,
      revealed: string | undefined,
      requests: RequestCards | undefined,
    ) => {
      for (const turn of group.turns || [group]) {
        const id = turn.items[0].turnId;
        if (id && !visibility.has(id)) visibility.set(id, !turn.outcome);
      }
      return messageGroups(group.items, visibility, true, revealed, requests);
    },
  );
}

/** The stored work-log preference is initialized once and shared by contributors. */
export class WorkLog {
  open: boolean;
  constructor(key: string, id: string, active: boolean, tools: number) {
    this.open =
      saved<Record<string, boolean>>(key, {})[id] ?? (active || tools < 3);
    makeAutoObservable(this, { summary: false }, { autoBind: true });
  }
  summary = derived((items: Message[]) => {
    const tools = items.filter((item) =>
      ["tool", "output"].includes(item.role),
    );
    return {
      summary: activitySummary(tools),
      hasLimit: tools.some((item) => toolLimitNotice(item)),
    };
  });
  update(value: boolean, key: string, ids: string[]) {
    this.open = value;
    save(
      key,
      Object.fromEntries([
        ...Object.entries(saved<Record<string, boolean>>(key, {}))
          .filter(([id]) => !ids.includes(id))
          .slice(-499),
        ...ids.map((id) => [id, value]),
      ]),
    );
  }
}

const sameIds = (left: string[], right: string[]) =>
  left.length === right.length && left.every((id, i) => id === right[i]);
function measure(rows: HistoryGroup[]) {
  const anchorRows = new Map<string, string>();
  for (const row of rows)
    for (const item of row.items)
      for (const id of [messageRenderKey(item), item.id, item.sourceId])
        if (id && !anchorRows.has(id)) anchorRows.set(id, row.id);
  for (const row of rows)
    if (!anchorRows.has(row.id)) anchorRows.set(row.id, row.id);
  return {
    estimates: historyOffsets(rows, new Map()),
    count: rows.reduce((count, row) => count + row.items.length, 0),
    anchorRows,
  };
}
