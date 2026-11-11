import { z } from "zod";
import { createRef } from "react";
import { flow, makeAutoObservable, observableRef } from "mobx";
import { UnsentText } from "../../../unsent/useUnsentText.ts";
import { serverMutationResponseIsUncertain } from "../../../servers/mutationIdentity.ts";
import {
  ApiError,
  errorText,
  type QueueItemDto as ApiQueueItemDto,
} from "../../../api.ts";
import type { DeepReadonly } from "../../../deepReadonly.ts";
import type { Message } from "../../../types.ts";
import { copyText } from "../../../clipboard/clipboard.ts";

type QueueItemDto = DeepReadonly<ApiQueueItemDto>;
export type LocalQueueItem = Pick<
  Message,
  "assets" | "id" | "requestedDelivery" | "text" | "deliveryStatus"
> & { localDelivery: true };
export type QueueItem =
  | (QueueItemDto & { localDelivery?: false })
  | LocalQueueItem;
export function isServerQueueItem(item: QueueItem): item is QueueItemDto {
  return item.localDelivery !== true;
}
export type QueueProps = {
  items: readonly QueueItem[];
  scope: string;
  onEdit: (item: QueueItemDto, text: string) => Promise<void>;
  onCancel: (item: QueueItemDto) => Promise<void>;
  onSendNow?: ((item: QueueItemDto) => Promise<void>) | undefined;
  onReorder: (ids: string[]) => Promise<void>;
  canReorder: boolean;
  refreshing?: boolean;
  loading?: boolean;
  error?: string;
};
const savedQueueEdit = z.object({ id: z.string(), text: z.string() });
export type EditDraft = z.infer<typeof savedQueueEdit> & {
  item?: QueueItemDto | undefined;
};

export function createMessageQueueStore(
  inputs: () => QueueProps,
  server: string,
) {
  const buffer = new UnsentText(server);
  return makeAutoObservable(
    {
      buffer,
      original: null as QueueItemDto | null,
      collapsed: false,
      busy: false,
      error: "",
      copied: false,
      expanded: null as string | null,
      dragged: null as string | null,
      dropTarget: null as string | null,
      textarea: createRef<HTMLTextAreaElement>(),
      get props(): QueueProps {
        return inputs();
      },
      get disabled() {
        return this.busy || !!this.props.refreshing || !!this.props.loading;
      },
      get absent(): EditDraft | undefined {
        const current = this.current;
        if (current && !inputs().items.some((item) => item.id === current.id))
          return current;
        return undefined;
      },
      update(values: Partial<MessageQueueStore>) {
        Object.assign(this, values);
      },
      get current(): EditDraft | null {
        const raw = buffer.read("queue-edit", inputs().scope);
        if (!raw) return null;
        try {
          const saved = savedQueueEdit.parse(JSON.parse(raw));
          const item =
            this.original?.id === saved.id
              ? this.original
              : inputs().items.find(
                  (entry): entry is QueueItemDto =>
                    entry.id === saved.id && isServerQueueItem(entry),
                );
          return { id: saved.id, text: saved.text, item };
        } catch {
          return null;
        }
      },
      dismiss() {
        buffer.update("queue-edit", inputs().scope, null);
        this.update({ original: null, error: "" });
      },
      beginEdit(item: QueueItem) {
        if (!isServerQueueItem(item)) return;
        this.original = item;
        buffer.update(
          "queue-edit",
          inputs().scope,
          JSON.stringify({ id: item.id, text: item.text }),
        );
        this.update({ collapsed: false, error: "", copied: false });
      },
      updateDraft(draft: EditDraft, text: string) {
        buffer.update(
          "queue-edit",
          inputs().scope,
          JSON.stringify({ id: draft.id, text }),
        );
        this.copied = false;
      },
      *mutate(operation: () => Promise<void>) {
        if (this.busy || this.props.refreshing) return;
        this.update({ busy: true, error: "" });
        try {
          yield operation();
        } catch (error) {
          this.error = errorText(error);
        } finally {
          this.busy = false;
        }
      },
      *saveEdit(draft: EditDraft) {
        const item = draft.item;
        if (
          !item ||
          inputs().loading ||
          !inputs().items.some((row) => row.id === draft.id) ||
          (!draft.text.trim() && !item.assets?.length)
        )
          return;
        const { dismiss } = this;
        const expected = buffer.read("queue-edit", inputs().scope);
        yield this.mutate(
          flow(function* () {
            try {
              yield inputs().onEdit(item, draft.text);
              dismiss();
            } catch (error) {
              if (
                error instanceof ApiError &&
                !serverMutationResponseIsUncertain(error.status, error.response)
              )
                yield buffer.clear("queue-edit", inputs().scope, expected);
              throw error;
            }
          }),
        );
      },
      move(id: string, targetIndex: number) {
        const { items, canReorder, onReorder } = this.props;
        if (
          !canReorder ||
          this.disabled ||
          targetIndex < 0 ||
          targetIndex >= items.length
        )
          return;
        const ids = items.map((item) => item.id),
          from = ids.indexOf(id);
        if (from < 0 || from === targetIndex) return;
        ids.splice(from, 1);
        ids.splice(targetIndex, 0, id);
        void this.mutate(() => onReorder(ids));
      },
      *copyDraft(draft: EditDraft) {
        try {
          yield copyText(draft.text);
          this.copied = true;
        } catch {
          this.textarea.current?.focus();
          this.textarea.current?.select();
          this.error =
            "Copy is unavailable. Select and copy the text from the editor.";
        }
      },
      cancel(item: QueueItem) {
        if (isServerQueueItem(item))
          void this.mutate(() => this.props.onCancel(item));
      },
    },
    { buffer: false, original: observableRef, textarea: false },
    { autoBind: true },
  );
}
export type MessageQueueStore = ReturnType<typeof createMessageQueueStore>;
