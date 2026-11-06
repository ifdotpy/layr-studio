// The functional build keeps the validators out of the startup bundle size:
// the full zod API added about 85 kB to the first chunk.
import { z } from "zod/mini";
import { DeliveryMomentInput } from "../generated/api.ts";
import type { CommandRequest } from "../commands.ts";
import type { components } from "../generated/api.ts";

type SendCommand = Extract<CommandRequest, { kind: "send" }>;
const commandRequestSchema: z.ZodMiniType<SendCommand> = z.strictObject({
  id: z.string(),
  client: z.string(),
  number: z.int(),
  kind: z.literal("send"),
  target: z.string(),
  body: z.strictObject({
    text: z.string(),
    asTextFile: z.optional(z.boolean()),
    assets: z.optional(z.array(z.string())),
    delivery: z.optional(z.enum(DeliveryMomentInput)),
  }),
});
const attachmentSchema = z.strictObject({
  id: z.string(),
  name: z.string(),
  mime: z.string(),
  image: z.boolean(),
  size: z.number(),
  preview: z.optional(z.string()),
});
const recordingSchema = z.strictObject({
  id: z.string(),
  chatId: z.string(),
  created: z.number(),
  sampleRate: z.number(),
  samples: z.number(),
  state: z.enum(["recording", "ready"]),
  transcript: z.optional(z.string()),
  reviewedText: z.optional(z.string()),
  transcriptionAttempt: z.optional(z.string()),
  error: z.optional(z.string()),
  deletedAt: z.optional(z.number()),
});
export type Recording = z.infer<typeof recordingSchema>;
const uploadSchema = z
  .strictObject({
    id: z.string(),
    workspace: z.string(),
    stateDir: z.string(),
    agent: z.string(),
    name: z.string(),
    size: z.number(),
    mime: z.string(),
    created: z.number(),
    bytes: z.instanceof(Blob),
  })
  .check(z.refine((row) => row.size === row.bytes.size));
export type StoredUpload = z.infer<typeof uploadSchema>;
const localSendSchema = z.discriminatedUnion("kind", [
  z.strictObject({ kind: z.literal("stored") }),
  z.strictObject({ kind: z.literal("in_flight") }),
  z.strictObject({ kind: z.literal("transport_failed"), error: z.string() }),
]);
export type LocalSend = z.infer<typeof localSendSchema>;
const sendSchema = z.strictObject({
  request: commandRequestSchema,
  localState: localSendSchema,
  workspaceId: z.string(),
  created: z.number(),
  attachments: z.array(attachmentSchema),
  displayText: z.optional(z.string()),
});
export type StoredSend = z.infer<typeof sendSchema>;
// Unsent record format version, measured in format revisions.
export const UNSENT_FORMAT_VERSION = 1;
const version = z.literal(UNSENT_FORMAT_VERSION);
const draftSchema: z.ZodMiniType<components["schemas"]["SyncDocument"]> =
  z.strictObject({
    id: z.string(),
    payload: z.string(),
    seq: z.int(),
    _deleted: z.boolean(),
  });
export const unsentRecordSchema = z.discriminatedUnion("kind", [
  z.strictObject({ version, kind: z.literal("text"), value: z.string() }),
  z.strictObject({
    version,
    kind: z.literal("draft"),
    value: draftSchema,
  }),
  z.strictObject({ version, kind: z.literal("upload"), value: uploadSchema }),
  z.strictObject({
    version,
    kind: z.literal("record"),
    value: recordingSchema,
  }),
  z.strictObject({
    version,
    kind: z.literal("chunk"),
    id: z.string(),
    index: z.int().check(z.nonnegative()),
    value: z.instanceof(Blob).check(z.refine((blob) => blob.size % 2 === 0)),
  }),
  z.strictObject({ version, kind: z.literal("send"), value: sendSchema }),
]);
export type UnsentRecord = z.infer<typeof unsentRecordSchema>;
export type RecordingValue =
  Extract<UnsentRecord, { kind: "record" | "chunk" }> extends infer T
    ? T extends { version: number }
      ? Omit<T, "version">
      : never
    : never;
export function isUnsentRecord(value: unknown): value is UnsentRecord {
  return unsentRecordSchema.safeParse(value).success;
}
