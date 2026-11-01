import { useEffect, useState } from "react";
import {
  reportUnsentFailure,
  sendStore,
  type StoredSend,
  type LocalSend,
} from "../unsent/store";
import { serverViewId } from "../servers/environment";
import { serverLocalStorage } from "../servers/storage";
import { serverMutationResponseIsUncertain } from "../servers/mutationIdentity";
import { ApiError, command, errorText, getStreamSession } from "../api";
import {
  messageReceipt,
  type MessageDelivery,
  type SendMessageBody,
} from "../commands";
import type { Attachment } from "../components/ComposerAttachments";
import { watchResourceConnection } from "./subscriptions/events";
import { CommandErrorCode, MessageSendStatus } from "../generated/api";
import type { MessageReceipt } from "./messageReceipts";

export type DeliveryState =
  | { source: "device"; state: LocalSend | { kind: "removed" } }
  | { source: "server"; status: MessageSendStatus };
type Intention = {
  body: SendMessageBody;
  status: DeliveryState;
  receipt?: MessageDelivery | undefined;
  created: number;
  attachments?: Attachment[] | undefined;
  displayText?: string | undefined;
};
export type OutboxEntry = Intention & { id: string };
export type OutgoingMessage = OutboxEntry;
type SavedSend = StoredSend & { body: SendMessageBody };
type SendResult =
  | { kind: "confirmed"; id: string }
  | { kind: "removed"; id: string }
  | (MessageDelivery & { kind: "server" })
  | { kind: "local"; id: string; state: LocalSend };

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
  buffer?: ReturnType<typeof sendStore>;
  serverId?: string;
  watchResourceConnection: typeof watchResourceConnection;
}) {
  const { command } = options.api;
  const buffer =
    options.buffer ?? sendStore(options.serverId ?? serverViewId ?? "local");
  const saved = new Map<string, SavedSend>();
  const removed = new Set<string>();
  const initialWrites = new Map<string, Promise<void>>();
  let initialization: Promise<void> | undefined;
  const ready = () =>
    (initialization ??= (async () => {
      for (const [id, value] of await buffer.loadAll()) {
        if (!saved.has(id) && !removed.has(id)) {
          const body = {
            id: value.request.id,
            room: value.request.target,
            ...value.request.body,
          };
          saved.set(id, { ...value, body });
        }
      }
    })().catch((error: unknown) => {
      initialization = undefined;
      throw error;
    }));
  const listeners = new Set<() => void>();
  const active = new Map<string, Promise<SendResult>>();
  const key = (room: string, id: string) => JSON.stringify([room, id]);
  const publish = () => listeners.forEach((listener) => listener());
  function list(): OutboxEntry[] {
    return [...saved.values()]
      .map((value) => ({
        ...value,
        id: value.request.id,
        status: { source: "device" as const, state: value.localState },
      }))
      .sort((a, b) => a.created - b.created);
  }
  function read(room: string, id: string): SavedSend | undefined {
    return saved.get(key(room, id));
  }
  function write(value: SavedSend, initial = false) {
    const id = key(value.body.room, value.body.id);
    const record: StoredSend = {
      request: value.request,
      localState: value.localState,
      workspaceId: value.workspaceId,
      created: value.created,
      attachments: value.attachments ?? [],
      ...(value.displayText === undefined
        ? {}
        : { displayText: value.displayText }),
    };
    saved.set(id, value);
    publish();
    const pending = initial
      ? buffer.put(id, record)
      : (initialWrites.get(id) ?? Promise.resolve()).then(() =>
          buffer.putIfExists(id, record),
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
      state: value.localState,
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
      if (value.localState.kind === "receipt_expired") return result(value);
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
            kind: "removed",
            id,
          };
        const current = options.api.getStreamSession();
        if (!current) throw new Error("The server connection is not ready.");
        if (current.workspaceId !== value.workspaceId)
          throw new Error(
            "The server workspace changed. Reload before sending.",
          );
        const inFlight: LocalSend = { kind: "in_flight" };
        value = { ...value, localState: inFlight };
        void write(value);
        if (!read(value.body.room, id))
          return {
            kind: "removed",
            id,
          };
        const response = await command.run(value.request, {
          workspaceId: value.workspaceId,
        });
        const receipt = messageReceipt(response);
        if (
          !receipt &&
          response.status === "applied" &&
          response.ref_kind === "message" &&
          response.ref_id === id
        ) {
          notifyPersisted();
          void remove(value.body.room, id).catch(reportUnsentFailure);
          return { kind: "confirmed", id };
        }
        if (!receipt || !matchesMessageDelivery(receipt, id))
          throw new Error("The delivery response does not match this message.");
        if (
          receipt.status === MessageSendStatus.failed ||
          receipt.status === MessageSendStatus.cancelled
        ) {
          refused = true;
          if (read(value.body.room, id))
            void remove(value.body.room, id).catch(reportUnsentFailure);
          return { ...receipt, kind: "server" };
        }
        notifyPersisted();
        if (receipt.status === MessageSendStatus.uncertain) {
          if (read(value.body.room, id))
            await write({ ...value, localState: { kind: "stored" } });
        } else {
          void remove(value.body.room, id).catch(reportUnsentFailure);
        }
        return {
          ...receipt,
          kind: "server",
        };
      } catch (error) {
        const expired =
          error instanceof ApiError &&
          error.status === 410 &&
          !!error.response &&
          typeof error.response === "object" &&
          "code" in error.response &&
          error.response.code === CommandErrorCode.receipt_expired;
        const definitive =
          error instanceof ApiError &&
          !serverMutationResponseIsUncertain(error.status, error.response);
        if (definitive) refused = true;
        const localState: LocalSend = {
          kind: expired ? "receipt_expired" : "transport_failed",
          error: errorText(error),
        };
        const failed: SavedSend = { ...value, localState };
        if (read(value.body.room, id)) {
          if (definitive)
            void remove(value.body.room, id).catch(reportUnsentFailure);
          else if (expired) await write(failed);
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
        localState: { kind: "stored" },
        created: Date.now(),
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
    await remove(entry.body.room, id);
    return false;
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
      if (
        value?.workspaceId === workspaceId &&
        receipt.status !== MessageSendStatus.uncertain
      )
        await remove(room, receipt.id);
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
            if (value)
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
