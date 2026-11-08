import { reportPromptComposerRender } from "../perf/promptComposerRenderProbe.ts";
import type { Message } from "../types.ts";
import type { createTranscriptMirror } from "../generated/observableResources.ts";

type RecordModel = ReturnType<
  typeof createTranscriptMirror
>["current"]["items"][number];
type InputModel = NonNullable<RecordModel["inputs"]>[number];

const titles: Record<string, string> = {
  agent_message: "Agent message received",
  child_result: "Worker result received",
  monitor_exit: "Command finished",
  monitor_cancelled: "Command cancelled",
  complaint_response: "Complaint response received",
  followup: "Agent follow-up",
  complaint: "Complaint requires a response",
  work_released: "Task released from a failed worker",
};
function inputTitle(input: InputModel) {
  if (input.kind === "user") return "You";
  if (input.kind === "agent_message" || input.kind === "child_result") {
    try {
      const event = JSON.parse(input.text || "");
      const name = event?.sender_name || event?.name;
      if (typeof name === "string" && name.trim())
        return `${input.kind === "child_result" ? "Result" : "Message"} from ${name.trim()}`;
    } catch {
      /* A legacy input can contain plain text. */
    }
  }
  return titles[input.kind] || "Team activity";
}

/** A presentation reference, not a server-data copy. Unchanged fields read the
 * generated record directly; only input flattening and renamed fields are derived.
 * The proxy preserves enumerable fields for existing local receipt/outbox overlays. */
function messageView(
  record: RecordModel,
  owner: string | null,
  input?: InputModel,
  index = 0,
): Message {
  reportPromptComposerRender("message-view", record.id);
  const inputIndex = () =>
    input ? (record.inputs?.indexOf(input) ?? index) : index;
  const derived: Message = {
    get text() {
      return record.text ?? "";
    },
    get role() {
      return record.role ?? "system";
    },
    get id() {
      return input
        ? input.id
          ? `${owner}:${input.id}`
          : `${record.id}:${inputIndex()}`
        : record.id;
    },
    get sourceId() {
      return input ? record.id : record.sourceId;
    },
    get threadId() {
      return record.threadId ?? undefined;
    },
    get clientMessageId() {
      return input
        ? input.clientMessageId ||
            input.id ||
            (inputIndex() === 0 ? record.clientMessageId : undefined)
        : record.clientMessageId;
    },
    get deliveryStatus() {
      const status = (input || record).deliveryStatus;
      return status ? { source: "server" as const, status } : undefined;
    },
    get streamPending() {
      return (input ? input.kind === "user" : record.role === "user")
        ? undefined
        : ((input || record).pending ?? undefined);
    },
  };
  Object.defineProperties(
    derived,
    Object.getOwnPropertyDescriptors(
      input
        ? {
            get text() {
              return input.text;
            },
            get role() {
              return input.kind === "user" ? "user" : "tool";
            },
            get title() {
              return inputTitle(input);
            },
            get truncated() {
              return input.truncated;
            },
            get requestedDelivery() {
              return input.requestedDelivery;
            },
            get deliveryError() {
              return input.deliveryError;
            },
            get materialized() {
              return input.materialized ?? record.materialized;
            },
            get assets() {
              return input.assets || (inputIndex() === 0 ? record.assets : []);
            },
          }
        : {},
    ),
  );
  // Keep getter descriptors: spreading `derived` would acquire all fields now.
  return new Proxy(derived, {
    get(_target, key) {
      if (key === "pending") return undefined;
      return key in derived
        ? Reflect.get(derived, key)
        : Reflect.get(record, key);
    },
    ownKeys() {
      return [
        ...new Set([...Reflect.ownKeys(record), ...Reflect.ownKeys(derived)]),
      ].filter((key) => key !== "pending");
    },
    has(_target, key) {
      return key !== "pending" && (key in derived || key in record);
    },
    getOwnPropertyDescriptor(_target, key) {
      const descriptor =
        Reflect.getOwnPropertyDescriptor(derived, key) ||
        Reflect.getOwnPropertyDescriptor(record, key);
      return descriptor && { ...descriptor, configurable: true };
    },
  });
}

/** One factory per history owner. Weak references cannot retain departed models. */
export function createMessageViews(owner: string | null) {
  const references = new WeakMap<object, Message>();
  return (records: readonly RecordModel[]): Message[] =>
    records.flatMap((record) => {
      const view = (key: object, input?: InputModel, index?: number) => {
        let result = references.get(key);
        if (!result) {
          result = messageView(record, owner, input, index);
          references.set(key, result);
        }
        return result;
      };
      return record.inputs
        ? record.inputs.map((input, index) => view(input, input, index))
        : [view(record)];
    });
}

/** One-shot compatibility for existing editing/removal callers. */
export function transcriptMessages(
  source: readonly import("../types.ts").TranscriptWireItem[],
  owner: string | null,
): Message[] {
  return createMessageViews(owner)(source);
}
