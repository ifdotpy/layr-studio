import { expect, it } from "vitest";
import { createRecordingStorage, type Recording } from "./storage";

it("restores recordings and undo after reload and rejects late transcription", async () => {
  const records = new Map<string, string | Blob>();
  const buffer = {
    put: async (key: string, value: string | Blob) => {
      records.set(key, value);
    },
    delete: async (key: string) => {
      records.delete(key);
    },
    loadAll: async (): Promise<[string, string | Blob][]> => [...records],
  };
  const storage = createRecordingStorage("test", buffer);
  const row: Recording = {
    id: "one",
    chatId: "chat",
    created: 1,
    sampleRate: 16000,
    samples: 2,
    state: "recording",
  };
  await storage.saveChunk(row, 0, new Int16Array([1, 2]));
  expect(await storage.listRecordings("chat")).toEqual([row]);
  expect((await storage.recordingAudio(row)).size).toBe(48);
  const restored = createRecordingStorage("test", buffer);
  expect(await restored.listRecordings("chat")).toEqual([row]);
  expect((await restored.recordingAudio(row)).size).toBe(48);
  await restored.trashRecording(row);
  expect(
    await createRecordingStorage("test", buffer).deletedRecordings("chat"),
  ).toHaveLength(1);
  await storage.deleteRecording(row.id);
  await storage.updateRecording({
    id: row.id,
    chatId: row.chatId,
    transcript: "late",
  });
  expect(await storage.listRecordings("chat")).toEqual([]);
});
