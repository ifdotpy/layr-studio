import { serverLocalStorage } from "../../../servers/storage.ts";
import { observable, observableRef, makeAutoObservable, action } from "mobx";
import type { Message } from "../../../types.ts";

type Removed = {
  keys: Set<string>;
  copies: Message[];
  restored: Message[];
};
const owners = new WeakMap<Storage, Map<string, Removed>>();
function record(storage: Storage, key: string): Removed {
  let records = owners.get(storage);
  if (!records) owners.set(storage, (records = new Map()));
  let value = records.get(key);
  if (!value)
    records.set(
      key,
      (value = observable(
        {
          keys: observable.set<string>(),
          copies: [] as Message[],
          restored: [] as Message[],
        },
        { copies: observableRef, restored: observableRef },
      )),
    );
  return value;
}

export function messageRemovalKeys(message: Message, chat: string) {
  const keys = [`item:${message.id}`];
  const clientId = message.clientMessageId;
  if (clientId)
    keys.push(
      `client:${clientId}`,
      `item:${clientId}`,
      `item:${chat}:${clientId}`,
      `item:${chat}:${clientId}:0`,
    );
  return keys;
}
export const removedMessagesStorageKey = (
  workspace: string,
  kind: string,
  chat: string | null,
) => `studio-removed-messages:${JSON.stringify([workspace, kind, chat])}`;
export function isRemovedMessage(
  workspace: string,
  kind: string,
  chat: string,
  message: Message,
  storage = serverLocalStorage,
) {
  const value = record(
    storage,
    removedMessagesStorageKey(workspace, kind, chat),
  );
  return messageRemovalKeys(message, chat).some((key) => value.keys.has(key));
}
export const saveRemovedMessage = action(function saveRemovedMessage(
  workspace: string,
  kind: string,
  chat: string,
  message: Message,
  storage = serverLocalStorage,
) {
  const value = record(
    storage,
    removedMessagesStorageKey(workspace, kind, chat),
  );
  value.copies = value.copies.filter(
    (item) =>
      item.id !== message.id &&
      (!message.clientMessageId ||
        item.clientMessageId !== message.clientMessageId),
  );
  value.copies.push(message);
});
export const removeMessageFromDevice = action(function removeMessageFromDevice(
  workspace: string,
  kind: string,
  chat: string,
  message: Message,
  storage = serverLocalStorage,
) {
  const key = removedMessagesStorageKey(workspace, kind, chat);
  saveRemovedMessage(workspace, kind, chat, message, storage);
  const value = record(storage, key);
  for (const item of messageRemovalKeys(message, chat)) value.keys.add(item);
});
export function createRemovedMessages(
  workspace: string,
  kind: string,
  chat: string | null,
  storage = serverLocalStorage,
) {
  const value = record(
    storage,
    removedMessagesStorageKey(workspace, kind, chat),
  );
  return makeAutoObservable(
    {
      hidden(message: Message) {
        return messageRemovalKeys(message, chat || "").some((key) =>
          value.keys.has(key),
        );
      },
      get restored() {
        return value.restored.map((message) => ({
          ...message,
          localDelivery: false,
          materialized: true,
        }));
      },
      get hasRemoved() {
        return value.keys.size > 0;
      },
      restore() {
        value.restored = value.copies;
        value.keys.clear();
      },
      remove(message: Message) {
        removeMessageFromDevice(workspace, kind, chat || "", message, storage);
      },
    },
    {},
    { autoBind: true },
  );
}
