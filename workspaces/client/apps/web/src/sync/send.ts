import { useEffect, useState } from "react";
import {
  migrateMessageSends,
  reportUnsentFailure,
  unsentStore,
} from "../unsent/store";
import { serverViewId } from "../servers/environment";
import { serverLocalStorage } from "../servers/storage";
import { serverMutationResponseIsUncertain } from "../servers/mutationIdentity";
import { ApiError, command, errorText, getStreamSession } from "../api";
import {
  messageReceipt,
  type CommandRequest,
  type MessageDelivery,
  type SendMessageBody,
} from "../commands";
import type { Attachment } from "../components/ComposerAttachments";
import { watchResourceConnection } from "./subscriptions/events";
import { MessageSendStatus } from "../generated/api";
import type { MessageReceipt } from "./messageReceipts";

type SendCommand = Extract<CommandRequest, { kind: "send" }>;
type Intention = {
  body: SendMessageBody;
  status: MessageSendStatus;
  attempted?: boolean;
  error?: string | undefined;
  receipt?: MessageDelivery | undefined;
  created: number;
  displayPending?: boolean;
  attachments?: Attachment[] | undefined;
  displayText?: string;
};
export type OutboxEntry = Intention & { id: string };
export type OutgoingMessage = OutboxEntry;
type SavedSend = Intention & { request: SendCommand; workspaceId: string };
type SendResult =
  | (MessageDelivery & { kind: "server" })
  | {
      kind: "local";
      id: string;
      status: Intention["status"];
      error?: string | undefined;
    };

const statuses = new Set<string>(Object.values(MessageSendStatus));
export function matchesMessageDelivery(value: unknown, id: string) {
  return (
    !!value &&
    typeof value === "object" &&
    "id" in value &&
    value.id === id &&
    "status" in value &&
    typeof value.status === "string" &&
    statuses.has(value.status)
  );
}

export function createOutbox(options: {
  api: Pick<import("../api").ServerApi, "command" | "getStreamSession">;
  storage: Storage;
  buffer?: ReturnType<typeof unsentStore>;
  serverId?: string;
  watchResourceConnection: typeof watchResourceConnection;
}) {
  const { command } = options.api;
  const buffer =
    options.buffer ??
    unsentStore(options.serverId ?? serverViewId ?? "local", "message");
  const saved = new Map<string, string>();
  const removed = new Set<string>();
  const initialWrites = new Map<string, Promise<void>>();
  let initialization: Promise<void> | undefined;
  const ready = () =>
    (initialization ??= (async () => {
      await migrateMessageSends(options.storage, buffer);
      for (const [id, value] of await buffer.loadAll())
        if (typeof value === "string" && !saved.has(id) && !removed.has(id))
          saved.set(id, value);
    })().catch((error: unknown) => {
      initialization = undefined;
      throw error;
    }));
  const listeners = new Set<() => void>();
  const active = new Map<string, Promise<SendResult>>();
  const key = (room: string, id: string) => JSON.stringify([room, id]);
  const publish = () => listeners.forEach((listener) => listener());
  function list(): OutboxEntry[] {
    return [...saved.keys()]
      .flatMap((suffix) => {
        let identity: unknown;
        try {
          identity = JSON.parse(suffix);
        } catch {
          return [];
        }
        if (
          !Array.isArray(identity) ||
          identity.length !== 2 ||
          !identity.every((value) => typeof value === "string")
        )
          return [];
        const [room, id] = identity as [string, string];
        const value = read(room, id);
        return [
          value
            ? { ...value, id }
            : {
                id,
                body: { id, room, text: "Saved message could not be read." },
                status: MessageSendStatus.failed,
                error:
                  "This saved message cannot be read. Discard it to remove it.",
                created: 0,
                displayPending: true,
              },
        ];
      })
      .sort((a, b) => a.created - b.created);
  }
  function read(room: string, id: string): SavedSend | undefined {
    const raw = saved.get(key(room, id));
    if (!raw) return;
    try {
      const row: unknown = JSON.parse(raw);
      if (!row || typeof row !== "object") return;
      const value = row as SavedSend;
      // Older saved commands used queued or paused for the device buffer.
      if (
        value.status === MessageSendStatus.queued ||
        ("status" in row && row.status === "paused")
      )
        value.status = MessageSendStatus.buffered;
      if (
        value.body?.id !== id ||
        value.body.room !== room ||
        typeof value.body.text !== "string" ||
        value.request?.id !== id ||
        value.request.kind !== "send" ||
        value.request.target !== room ||
        typeof value.request.client !== "string" ||
        !Number.isSafeInteger(value.request.number) ||
        typeof value.workspaceId !== "string" ||
        typeof value.created !== "number" ||
        (value.status !== MessageSendStatus.buffered &&
          value.status !== MessageSendStatus.uncertain &&
          value.status !== MessageSendStatus.failed)
      )
        return;
      const expected = {
        text: value.body.text,
        ...(value.body.assets ? { assets: value.body.assets } : {}),
        ...(value.body.delivery ? { delivery: value.body.delivery } : {}),
      };
      if (JSON.stringify(value.request.body) !== JSON.stringify(expected))
        return;
      return value;
    } catch {
      return;
    }
  }
  function write(value: SavedSend, initial = false) {
    const id = key(value.body.room, value.body.id);
    const text = JSON.stringify(value);
    saved.set(id, text);
    publish();
    const pending = initial
      ? buffer.put(id, text)
      : (initialWrites.get(id) ?? Promise.resolve()).then(() =>
          buffer.putIfExists(id, text),
        );
    if (initial) {
      initialWrites.set(id, pending);
      void pending.then(
        () => {
          if (initialWrites.get(id) === pending) initialWrites.delete(id);
        },
        () => {
          if (initialWrites.get(id) === pending) initialWrites.delete(id);
        },
      );
    }
    void pending.catch(reportUnsentFailure);
    return pending;
  }
  async function remove(room: string, id: string) {
    const identity = key(room, id);
    removed.add(identity);
    saved.delete(identity);
    publish();
    await initialWrites.get(identity)?.catch(() => {});
    initialWrites.delete(identity);
    await buffer.delete(identity);
  }
  function find(id: string) {
    return list().find((entry) => entry.id === id);
  }
  function result(value: SavedSend): SendResult {
    return {
      kind: "local",
      id: value.body.id,
      status: value.status,
      error: value.error,
    };
  }
  function send(
    value: SavedSend,
    onPersist?: () => void | Promise<void>,
    persisted?: Promise<void>,
  ): Promise<SendResult> {
    const id = value.body.id;
    const existing = active.get(id);
    if (existing) return existing;
    const task = (async (): Promise<SendResult> => {
      let notified = false;
      let refused = false;
      const notifyPersisted = () => {
        if (notified || refused) return;
        notified = true;
        void Promise.resolve()
          .then(() => onPersist?.())
          .catch(reportUnsentFailure);
      };
      if (persisted) void persisted.then(notifyPersisted, reportUnsentFailure);
      else notifyPersisted();
      try {
        if (!read(value.body.room, id))
          return {
            kind: "local",
            id,
            status: MessageSendStatus.cancelled,
          };
        const current = options.api.getStreamSession();
        if (!current) throw new Error("The server connection is not ready.");
        if (current.workspaceId !== value.workspaceId)
          throw new Error(
            "The server workspace changed. Reload before sending.",
          );
        void write({ ...value, attempted: true });
        if (!read(value.body.room, id))
          return {
            kind: "local",
            id,
            status: MessageSendStatus.cancelled,
          };
        const response = await command.run(value.request, {
          workspaceId: value.workspaceId,
        });
        const receipt = messageReceipt(response);
        if (!receipt || !matchesMessageDelivery(receipt, id))
          throw new Error("The delivery response does not match this message.");
        if (
          receipt.status === MessageSendStatus.failed ||
          receipt.status === MessageSendStatus.cancelled
        ) {
          refused = true;
          const failed = {
            ...value,
            status: receipt.status,
            attempted: true,
            error: receipt.error || "The message was not sent.",
          };
          if (read(value.body.room, id))
            void remove(value.body.room, id).catch(reportUnsentFailure);
          return result(failed);
        }
        notifyPersisted();
        void remove(value.body.room, id).catch(reportUnsentFailure);
        return {
          ...receipt,
          kind: "server",
        };
      } catch (error) {
        const definitive =
          error instanceof ApiError &&
          !serverMutationResponseIsUncertain(error.status, error.response);
        if (definitive) refused = true;
        const failed = {
          ...value,
          attempted: true,
          status: definitive
            ? MessageSendStatus.failed
            : MessageSendStatus.uncertain,
          error: errorText(error),
        };
        if (read(value.body.room, id)) {
          if (definitive)
            void remove(value.body.room, id).catch(reportUnsentFailure);
          else void write(failed);
        }
        return result(failed);
      }
    })().finally(() => active.delete(id));
    active.set(id, task);
    return task;
  }
  async function durableSend(
    body: SendMessageBody,
    attachments: Attachment[] = [],
    onPersist?: () => void | Promise<void>,
  ) {
    void ready().catch(reportUnsentFailure);
    if (!body.id) throw new Error("A message identity is required");
    let value = read(body.room, body.id);
    if (value && JSON.stringify(value.body) !== JSON.stringify(body))
      throw new Error("Message identity already has different content");
    if (!value) {
      if (saved.has(key(body.room, body.id)))
        throw new Error(
          "This saved message cannot be read. Discard it to remove it.",
        );
      const session = options.api.getStreamSession();
      if (!session) throw new Error("The server connection is not ready.");
      const { workspaceId } = session;
      const request = command.make(
        "send",
        body.room,
        {
          text: body.text,
          ...(body.assets ? { assets: body.assets } : {}),
          ...(body.delivery ? { delivery: body.delivery } : {}),
        },
        body.id,
      );
      value = {
        body,
        request,
        workspaceId,
        status: MessageSendStatus.buffered,
        attempted: false,
        created: Date.now(),
        displayPending: true,
        attachments,
      };
      removed.delete(key(body.room, body.id));
      const persisted = write(value, true);
      return send(value, onPersist, persisted);
    }
    return send(value, onPersist);
  }
  async function changeOutbox(id: string, action: "cancel" | "resume") {
    const inFlight = active.get(id);
    if (action === "resume" && inFlight) return inFlight;
    await ready();
    const entry = find(id);
    if (!entry) throw new Error("This message is no longer on this device.");
    const value = read(entry.body.room, id);
    if (action === "cancel") {
      await remove(entry.body.room, id);
      return;
    }
    if (!value) throw new Error("This saved message cannot be retried.");
    return send(value);
  }
  async function stopRemovedMessage(id: string) {
    await ready();
    const entry = find(id);
    if (!entry) return false;
    const neverAttempted = entry.attempted === false;
    await remove(entry.body.room, id);
    return neverAttempted;
  }
  async function acknowledgeOutbox(ids: string[]) {
    await ready();
    for (const id of ids) {
      const entry = find(id);
      if (entry) await remove(entry.body.room, id);
    }
  }
  async function reconcileOutboxReceipts(
    room: string,
    receipts: MessageReceipt[],
    workspaceId: string,
    signal?: AbortSignal,
  ) {
    await ready();
    if (signal?.aborted) return;
    for (const receipt of receipts) {
      const value = read(room, receipt.id);
      if (value?.workspaceId === workspaceId) await remove(room, receipt.id);
    }
  }
  async function editOutboxDisplay(id: string, text: string) {
    await ready();
    const entry = find(id);
    const value = entry && read(entry.body.room, id);
    if (value) await write({ ...value, displayText: text });
  }
  function useOutbox() {
    const [entries, setEntries] = useState<OutboxEntry[]>([]);
    const [error, setError] = useState("");
    useEffect(() => {
      const update = () => {
        try {
          setEntries(list());
          setError("");
        } catch (failure) {
          setError(errorText(failure));
        }
      };
      const retry = () => {
        try {
          for (const entry of list()) {
            const value = read(entry.body.room, entry.id);
            if (
              value &&
              (value.status === MessageSendStatus.buffered ||
                value.status === MessageSendStatus.uncertain)
            )
              void send(value).catch((failure) => setError(errorText(failure)));
          }
        } catch (failure) {
          setError(errorText(failure));
        }
      };
      listeners.add(update);
      let wasLive = false;
      const stopConnection = options.watchResourceConnection((status) => {
        const reconnected = status === "live" && !wasLive;
        wasLive = status === "live";
        if (reconnected) retry();
      });
      void ready()
        .then(() => {
          update();
          if (wasLive) retry();
        })
        .catch((failure) => setError(errorText(failure)));
      return () => {
        listeners.delete(update);
        stopConnection();
      };
    }, []);
    return { entries, error };
  }
  return {
    durableSend,
    acknowledgeOutbox,
    reconcileOutboxReceipts,
    editOutboxDisplay,
    changeOutbox,
    stopRemovedMessage,
    useOutbox,
  };
}

export const {
  durableSend,
  acknowledgeOutbox,
  reconcileOutboxReceipts,
  editOutboxDisplay,
  changeOutbox,
  stopRemovedMessage,
  useOutbox,
} = createOutbox({
  api: { command, getStreamSession },
  storage: serverLocalStorage,
  watchResourceConnection,
});
