import { recordingStore } from "../unsent/store.ts";
import { serverViewId } from "../servers/environment.ts";
import { randomUUID } from "../uuid";
import type { Recording } from "../unsent/schema.ts";
export type { Recording } from "../unsent/schema.ts";
export function createRecordingStorage(
  server = serverViewId ?? "local",
  buffer: Pick<
    ReturnType<typeof recordingStore>,
    "put" | "putChunk" | "delete" | "loadAll"
  > = recordingStore(server),
) {
  const records = new Map<string, Recording>();
  const chunks = new Map<string, Map<number, Int16Array>>();
  let loading: Promise<void> | undefined;
  const ready = () =>
    (loading ??= (async () => {
      for (const [, value] of await buffer.loadAll()) {
        if (value.kind === "record") records.set(value.value.id, value.value);
        else {
          let parts = chunks.get(value.id);
          if (!parts) chunks.set(value.id, (parts = new Map()));
          parts.set(
            value.index,
            new Int16Array(await value.value.arrayBuffer()),
          );
        }
      }
    })());
  const recordKey = (id: string) => `record:${id}`;
  const chunkKey = (id: string, index: number) =>
    `chunk:${JSON.stringify([id, index])}`;
  async function listRecordings(chatId: string) {
    await ready();
    return [...records.values()]
      .filter((row) => row.chatId === chatId && !row.deletedAt)
      .sort((a, b) => b.created - a.created);
  }
  async function saveRecording(row: Recording) {
    await ready();
    await buffer.put(recordKey(row.id), { kind: "record", value: row });
    records.set(row.id, { ...row });
  }
  async function saveChunk(row: Recording, index: number, pcm: Int16Array) {
    await ready();
    await buffer.putChunk(
      recordKey(row.id),
      { kind: "record", value: row },
      chunkKey(row.id, index),
      {
        kind: "chunk",
        id: row.id,
        index,
        value: new Blob([pcm.slice().buffer]),
      },
    );
    let parts = chunks.get(row.id);
    if (!parts) chunks.set(row.id, (parts = new Map()));
    parts.set(index, pcm.slice());
    records.set(row.id, { ...row });
  }
  async function deleteRecording(id: string) {
    await ready();
    const parts = chunks.get(id);
    records.delete(id);
    chunks.delete(id);
    for (const index of parts?.keys() ?? [])
      await buffer.delete(chunkKey(id, index));
    await buffer.delete(recordKey(id));
  }
  async function trashRecording(row: Recording) {
    await updateRecording({
      id: row.id,
      chatId: row.chatId,
      deletedAt: Date.now(),
      transcriptionAttempt: randomUUID(),
    });
  }
  async function restoreRecording(row: Recording) {
    await updateRecording({
      id: row.id,
      chatId: row.chatId,
      deletedAt: undefined,
    });
  }
  async function deletedRecordings(chatId: string) {
    await ready();
    return [...records.values()].filter(
      (row) => row.chatId === chatId && row.deletedAt,
    );
  }
  function wav(chunks: Int16Array[], sampleRate: number) {
    const samples = chunks.reduce((sum, part) => sum + part.length, 0);
    const output = new ArrayBuffer(44 + samples * 2),
      view = new DataView(output);
    const word = (offset: number, value: string) =>
      [...value].forEach((c, i) => view.setUint8(offset + i, c.charCodeAt(0)));
    word(0, "RIFF");
    view.setUint32(4, 36 + samples * 2, true);
    word(8, "WAVE");
    word(12, "fmt ");
    view.setUint32(16, 16, true);
    view.setUint16(20, 1, true);
    view.setUint16(22, 1, true);
    view.setUint32(24, sampleRate, true);
    view.setUint32(28, sampleRate * 2, true);
    view.setUint16(32, 2, true);
    view.setUint16(34, 16, true);
    word(36, "data");
    view.setUint32(40, samples * 2, true);
    let offset = 44;
    for (const chunk of chunks)
      for (const value of chunk) {
        view.setInt16(offset, value, true);
        offset += 2;
      }
    return new Blob([output], { type: "audio/wav" });
  }
  async function recordingAudio(row: Recording) {
    await ready();
    const parts = [...(chunks.get(row.id) ?? new Map()).entries()]
      .sort(([a], [b]) => a - b)
      .map(([, pcm]) => pcm);
    return wav(parts, row.sampleRate);
  }
  async function updateRecording(
    row: Pick<Recording, "id" | "chatId"> & Partial<Recording>,
    attempt?: string,
  ) {
    await ready();
    const current = records.get(row.id);
    if (
      current?.chatId === row.chatId &&
      (!attempt || current.transcriptionAttempt === attempt)
    ) {
      const next = { ...current, ...row };
      await buffer.put(recordKey(row.id), { kind: "record", value: next });
      records.set(row.id, next);
    }
  }
  async function beginTranscription(row: Recording, attempt: string) {
    await ready();
    const current = records.get(row.id);
    if (current?.chatId !== row.chatId) return false;
    await buffer.put(recordKey(row.id), {
      kind: "record",
      value: { ...current, transcriptionAttempt: attempt },
    });
    records.set(row.id, { ...current, transcriptionAttempt: attempt });
    return true;
  }

  return {
    listRecordings,
    saveRecording,
    saveChunk,
    deleteRecording,
    trashRecording,
    restoreRecording,
    deletedRecordings,
    wav,
    recordingAudio,
    updateRecording,
    beginTranscription,
  };
}

export const {
  listRecordings,
  saveRecording,
  saveChunk,
  deleteRecording,
  trashRecording,
  restoreRecording,
  deletedRecordings,
  wav,
  recordingAudio,
  updateRecording,
  beginTranscription,
} = createRecordingStorage();
