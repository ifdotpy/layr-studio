import { z } from "zod";
import { DeliveryMomentInput } from "../generated/api.ts";
import type { CommandRequest } from "../commands.ts";
import type { components } from "../generated/api.ts";

type SendCommand = Extract<CommandRequest, { kind: "send" }>;
const commandRequestSchema: z.ZodType<SendCommand> = z.strictObject({
  id: z.string(),
  client: z.string(),
  number: z.number().int().safe(),
  kind: z.literal("send"),
  target: z.string(),
  body: z.strictObject({
    text: z.string(),
    assets: z.array(z.string()).optional(),
    delivery: z.enum(DeliveryMomentInput).optional(),
  }),
});
const attachmentSchema = z.strictObject({
  id: z.string(),
  name: z.string(),
  mime: z.string(),
  image: z.boolean(),
  size: z.number(),
  preview: z.string().optional(),
});
const recordingSchema = z.strictObject({
  id: z.string(),
  chatId: z.string(),
  created: z.number(),
  sampleRate: z.number(),
  samples: z.number(),
  state: z.enum(["recording", "ready"]),
  transcript: z.string().optional(),
  reviewedText: z.string().optional(),
  transcriptionAttempt: z.string().optional(),
  error: z.string().optional(),
  deletedAt: z.number().optional(),
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
  .refine((row) => row.size === row.bytes.size);
export type StoredUpload = z.infer<typeof uploadSchema>;
const localSendSchema = z.discriminatedUnion("kind", [
  z.strictObject({ kind: z.literal("stored") }),
  z.strictObject({ kind: z.literal("in_flight") }),
  z.strictObject({ kind: z.literal("transport_failed"), error: z.string() }),
  z.strictObject({ kind: z.literal("receipt_expired"), error: z.string() }),
]);
export type LocalSend = z.infer<typeof localSendSchema>;
const sendSchema = z.strictObject({
  request: commandRequestSchema,
  localState: localSendSchema,
  workspaceId: z.string(),
  created: z.number(),
  attachments: z.array(attachmentSchema),
  displayText: z.string().optional(),
});
export type StoredSend = z.infer<typeof sendSchema>;
// Unsent record format version, measured in format revisions.
export const UNSENT_FORMAT_VERSION = 1;
const version = z.literal(UNSENT_FORMAT_VERSION);
const draftSchema: z.ZodType<components["schemas"]["SyncDocument"]> =
  z.strictObject({
    id: z.string(),
    payload: z.string(),
    seq: z.number().int().safe(),
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
    index: z.number().int().nonnegative(),
    value: z.instanceof(Blob).refine((blob) => blob.size % 2 === 0),
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
