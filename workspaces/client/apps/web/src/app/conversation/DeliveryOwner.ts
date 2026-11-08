import type { ServerAreas } from "../ServerAreasProvider.tsx";
import {
  makeAutoObservable,
  observable,
  observableRef,
  reaction,
  runInAction,
} from "mobx";
import { createSendingMessageRemoval } from "./delivery/removeSendingMessages.ts";
import { ApiError, errorText } from "../../api.ts";
import {
  codePointsExceed,
  sendsTextFile,
  type SendMessageBody,
} from "../../commands.ts";
import { requestId as randomUUID } from "studio-claude-bridge/request-id.ts";
import {
  AgentSource as GeneratedApiAgentSource,
  MessageSendStatus,
} from "../../generated/api.ts";
import { DeliveryMoment } from "../../generated/deliveryMoment.ts";
import { MAX_DRAFT_TEXT_CODEPOINTS } from "../../generated/sharedValues.ts";
import { renameCommand, studioCommand } from "../../nativeCommands.ts";
import { mergeOutgoing } from "../../components/conversation/support/messageDelivery.ts";
import type { ConversationSendOptions } from "../../components/conversation/support/navigation.ts";
import type { OutgoingMessage } from "../../sync/send.ts";

export function connectionFailureText(value: string) {
  return /^(Failed to fetch|Load failed|Network request failed|NetworkError\b|The device is offline\b)/i.test(
    value.trim(),
  );
}

export class DeliveryOwner {
  outgoing = observable.map<string, OutgoingMessage>({}, { deep: false });
  entries: OutgoingMessage[] = [];
  outboxError = "";
  observedSends = new Set<string>();
  // Per-chat sequence of applied send resets; the composer defers these through IME composition.
  draftResetVersions = observable.map<string, number>();
  sendingLock: symbol | null = null;
  latestSend: Record<string, symbol> = {};
  sends: Record<string, SendMessageBody> = {};
  removingAllSending = false;
  online = true;
  tracked: string[] = [];
  consumed: string[] = [];
  readonly areas;
  constructor(areas: ServerAreas) {
    this.areas = areas;
    makeAutoObservable(
      this,
      {
        areas: false,
        outgoing: false,
        entries: observableRef,
        latestSend: false,
        sends: false,
      },
      { autoBind: true },
    );
  }
  get selectedId() {
    return this.areas.navigation.selectedId;
  }
  get isSending() {
    return this.sendingLock !== null;
  }
  get outgoingMessages() {
    return mergeOutgoing(
      this.entries,
      this.outgoing.values(),
      this.observedSends,
    );
  }
  get selectedOutgoing() {
    return this.outgoingMessages.filter(
      (entry) => entry.body.room === this.selectedId,
    );
  }
  get selectedDraftConflicts() {
    return this.areas.scope.services.drafts.conflicts.filter(
      (version) => version.session === (this.selectedId || "new"),
    );
  }
  get draftSyncError() {
    const { error } = this.areas.scope.services.drafts;
    return this.ownsOfflineError(error) ? "" : error;
  }
  get outboxSyncError() {
    return this.ownsOfflineError(this.outboxError) ? "" : this.outboxError;
  }
  get hasLocalPersistenceFailure() {
    return this.areas.scope.services.drafts.localPersistenceFailed;
  }
  start() {
    const stopDrafts = this.areas.scope.services.drafts.start();
    const stopOutbox = this.areas.scope.services.outbox.observeOutbox(
      (entries) =>
        runInAction(() => {
          this.entries = entries;
          entries.forEach((entry) => this.outgoing.delete(entry.id));
        }),
      (error) =>
        runInAction(() => {
          this.outboxError = error;
        }),
    );
    const stopFeedback = this.startFeedback();
    let endActivity: (() => void) | undefined;
    const activity = reaction(
      () => this.hasLocalPersistenceFailure || this.isSending,
      (busy) => {
        endActivity?.();
        endActivity = busy
          ? this.areas.scope.services.activity.beginServerActivity()
          : undefined;
      },
      { fireImmediately: true },
    );
    return () => {
      activity();
      endActivity?.();
      stopFeedback();
      stopOutbox();
      stopDrafts();
    };
  }
  async removeAllSending() {
    const { notices } = this.areas;
    if (this.removingAllSending) return;
    const state = this.areas.state;
    if (!state) return;
    this.removingAllSending = true;
    const { api, outbox, storage } = this.areas.scope.services;
    try {
      const removal = createSendingMessageRemoval({
        get: api.get,
        command: api.command,
        stopRemovedMessage: outbox.stopRemovedMessage,
        localStorage: storage,
      });
      const count = await removal.removeAllSendingMessages(
        {
          stateDir: this.areas.startup.creationScope,
          workspaceId: this.areas.startup.workspaceId,
        },
        state.agents,
        this.outgoingMessages,
      );
      notices.notify(
        `Removed ${count} sending messages from this device. Work already sent continues.`,
      );
    } catch (error) {
      notices.notify(errorText(error));
    } finally {
      runInAction(() => {
        this.removingAllSending = false;
      });
    }
  }
  editDraft(
    text: string | ((current: string) => string),
    session = this.selectedId || "new",
  ) {
    const value =
      typeof text === "function"
        ? text(this.areas.scope.services.drafts.getDraft(session))
        : text;
    if (codePointsExceed(value, MAX_DRAFT_TEXT_CODEPOINTS)) {
      this.areas.notices.notify(
        `Drafts can contain at most ${MAX_DRAFT_TEXT_CODEPOINTS.toLocaleString()} characters. Previous text was kept.`,
      );
      return;
    }
    this.areas.scope.services.drafts.setDrafts((old) => ({
      ...old,
      [session]: value,
    }));
  }
  recordDraftReset(session: string) {
    this.draftResetVersions.set(
      session,
      (this.draftResetVersions.get(session) ?? 0) + 1,
    );
  }
  showOutgoing(entry: OutgoingMessage) {
    this.outgoing.set(entry.id, entry);
  }
  dropOutgoing(id: string) {
    this.outgoing.delete(id);
  }
  observeSends(ids: string[]) {
    ids.forEach((id) => {
      this.observedSends.add(id);
      this.outgoing.delete(id);
    });
  }
  editSend(id: string, text: string) {
    void this.areas.scope.services.outbox
      .editOutboxDisplay(id, text)
      .catch(() => {});
  }
  matchesRequest(
    request: SendMessageBody | undefined,
    text: string,
    options?: ConversationSendOptions,
  ) {
    return (
      !!request &&
      request.text === text &&
      JSON.stringify(request.assets || []) ===
        JSON.stringify(options?.assets || []) &&
      request.delivery === (options?.delivery || DeliveryMoment.CurrentTurn) &&
      !!request.asTextFile === !!options?.asTextFile
    );
  }
  async send(options?: ConversationSendOptions) {
    const {
      scope: { services },
      creation,
      rename,
      native,
    } = this.areas;
    const { notify } = this.areas.notices,
      drafts = services.drafts;
    const opened = this.selectedId;
    const agent = this.areas.navigation.selectedAgent;
    const schemaMismatch = this.areas.startup.schemaMismatch;
    const { isApiSchemaMismatch, command: serverCommand } = services.api;
    const { durableSend } = services.outbox;
    const { setDrafts, getDraft } = drafts;
    const refresh = this.areas.refresh;
    if (schemaMismatch || isApiSchemaMismatch()) return;
    const draftKey = opened || "new",
      draft = getDraft(draftKey),
      fileSend = sendsTextFile(draft, options?.asTextFile),
      text = fileSend ? draft : draft.trim();
    if ((!text && !options?.assets?.length) || this.sendingLock) return;
    const attempt = Symbol();
    this.sendingLock = attempt;
    this.latestSend[draftKey] = attempt;
    const release = () =>
      runInAction(() => {
        if (this.sendingLock !== attempt) return;
        this.sendingLock = null;
      });
    let request: SendMessageBody | undefined;
    try {
      if (!fileSend && renameCommand(text) !== undefined && !opened)
        throw new Error("Open a chat before using /rename.");
      const target = opened || (await creation.ensureConversation());
      if (target && typeof target === "object") {
        await this.areas.ui.chooseProject(
          target.cwd,
          target.folder,
          target.binding,
        );
        return;
      }
      const id = target;
      if (!id) return;
      this.latestSend[id] = attempt;
      const clearCapturedDraft = (
        matches: (value: string | undefined) => boolean,
      ) => {
        const cleared = new Set<string>();
        setDrafts((old) => {
          const next = { ...old };
          for (const key of new Set([draftKey, id])) {
            if (!matches(next[key])) continue;
            delete next[key];
            cleared.add(key);
          }
          return next;
        });
        cleared.forEach(this.recordDraftReset);
      };
      if (
        !fileSend &&
        agent?.source === GeneratedApiAgentSource.managed &&
        studioCommand(text, agent.provider ?? undefined)
      ) {
        const [command] = text.split(/\s+/);
        if (options?.assets?.length)
          throw new Error(
            "Commands cannot include files. Remove the attachments or send a normal message.",
          );
        if (command !== "/rename" && text !== command)
          throw new Error(
            "Use the command without additional text, or send a normal message.",
          );
        if (command === "/rename") {
          await rename.submit(id, text, renameCommand(text));
        } else if (command === "/stop" || command === "/stop-team")
          await serverCommand.stop(
            command === "/stop-team" ? (agent.rootId ?? id) : id,
            command === "/stop-team",
          );
        else
          await native.submit(agent, command.slice(1) as "compact" | "review");
      } else {
        const pending = this.entries.some(
          (entry) =>
            entry.id === this.sends[id]?.id &&
            !(
              entry.status.source === "device" &&
              entry.status.state.kind === "receipt_expired"
            ),
        );
        if (pending && this.matchesRequest(this.sends[id], text, options)) {
          notify("This message is still pending on this device.");
          return;
        }
        this.sends[id] = this.matchesRequest(this.sends[id], text, options)
          ? this.sends[id]!
          : {
              id: randomUUID(),
              room: id,
              text,
              assets: options?.assets?.slice() || [],
              ...(options?.asTextFile ? { asTextFile: true } : {}),
              // Enter delivers after the current tool call; Tab after the turn.
              delivery: options?.delivery || DeliveryMoment.CurrentTurn,
            };
        request = this.sends[id];
        if (!request) return;
        const entry: OutgoingMessage = {
          id: request.id,
          body: request,
          status: { source: "device", state: { kind: "stored" } },
          created: Date.now(),
          attachments: options?.attachments?.slice() || [],
        };
        this.showOutgoing(entry);
        const result = await durableSend(
          request,
          entry.attachments,
          async () => {
            // Keep the draft until the request is saved.
            clearCapturedDraft((value) => value === draft);
            // The saved command owns this message. A new user submission,
            // including identical text, gets its own identity.
            if (this.sends[id]?.id === entry.id) {
              delete this.sends[id];
            }
            await options?.onPersist?.();
            release();
          },
        );
        if (result.kind === "confirmed") {
          this.dropOutgoing(entry.id);
        } else if (result.kind === "removed") {
          throw new Error("This message was removed.");
        } else if (result.kind === "server") {
          this.showOutgoing({
            ...entry,
            status: { source: "server", status: result.status },
            receipt: result,
          });
          if (
            result.status === MessageSendStatus.cancelled ||
            result.status === MessageSendStatus.failed
          )
            throw new ApiError(
              result.error || "The message was not sent.",
              400,
            );
        }
        if (
          result.kind === "server" &&
          Object.values(result.deliveries || {}).some(
            (v) => v.status !== MessageSendStatus.queued,
          )
        )
          notify("Message saved. Some deliveries are not confirmed.");
      }
      if (!request) clearCapturedDraft((value) => value?.trim() === text);
      void refresh().catch((error) => notify(errorText(error)));
    } catch (e) {
      if (request) {
        // Never replace text the user typed while this request was in flight.
        const targetKey = request.room || draftKey;
        setDrafts((old) => {
          if (this.latestSend[targetKey] !== attempt || old[targetKey]?.trim())
            return old;
          const next = { ...old, [targetKey]: text };
          return next;
        });
        if (
          e instanceof ApiError &&
          e.status < 500 &&
          ![408, 429].includes(e.status) &&
          this.sends[request.room]?.id === request.id
        )
          delete this.sends[request.room];
        if (!this.entries.some((entry) => entry.id === request!.id)) {
          const entry = this.outgoing.get(request.id);
          if (entry && entry.status.source !== "server")
            this.showOutgoing({
              ...entry,
              status: {
                source: "device",
                state: { kind: "transport_failed", error: errorText(e) },
              },
            });
        }
      }
      if (!request) notify(errorText(e));
      throw e;
    } finally {
      release();
    }
  }
  get transportFailures() {
    return this.outgoingMessages.flatMap((entry) =>
      entry.status.source === "device" &&
      entry.status.state.kind === "transport_failed"
        ? [{ entry, error: entry.status.state.error }]
        : [],
    );
  }
  get currentErrors() {
    return [
      ...new Set(
        [
          this.areas.startup.error,
          this.areas.notices.toast,
          this.areas.scope.services.drafts.error,
          this.outboxError,
          ...this.transportFailures.map((failure) => failure.error),
        ].filter(connectionFailureText),
      ),
    ];
  }
  get disconnected() {
    return !this.online || this.currentErrors.length > 0;
  }
  get offlinePending() {
    return this.transportFailures.some(
      ({ entry }) =>
        entry.body.room === this.selectedId &&
        (this.disconnected || this.tracked.includes(entry.id)),
    );
  }
  get offlineDetails() {
    return [
      ...new Set([
        ...this.consumed,
        ...(this.offlinePending ? this.currentErrors : []),
      ]),
    ].join("\n");
  }
  ownsOfflineError(error: string) {
    return (
      connectionFailureText(error) &&
      (this.offlinePending || this.consumed.includes(error))
    );
  }
  updateNetwork() {
    this.online = navigator.onLine;
  }
  retainDiagnostics() {
    this.tracked = this.transportFailures
      .filter(
        ({ entry }) => this.disconnected || this.tracked.includes(entry.id),
      )
      .map(({ entry }) => entry.id);
    this.consumed = this.offlinePending
      ? [...new Set([...this.consumed, ...this.currentErrors])]
      : this.currentErrors.length
        ? this.consumed
        : [];
  }
  startFeedback() {
    this.updateNetwork();
    window.addEventListener("online", this.updateNetwork);
    window.addEventListener("offline", this.updateNetwork);
    const stop = reaction(
      () => [
        this.outgoingMessages,
        this.disconnected,
        this.currentErrors.join("\n"),
        this.selectedId,
      ],
      this.retainDiagnostics,
      { fireImmediately: true },
    );
    return () => {
      stop();
      window.removeEventListener("online", this.updateNetwork);
      window.removeEventListener("offline", this.updateNetwork);
    };
  }
}
