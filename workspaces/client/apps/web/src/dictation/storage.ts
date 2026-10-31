import { unsentStore } from "../unsent/store.ts";
import { serverViewId } from "../servers/environment.ts";
import { randomUUID } from "../uuid";
export type Recording = {
  id: string;
  chatId: string;
  created: number;
  sampleRate: number;
  samples: number;
  state: "recording" | "ready";
  transcript?: string | undefined;
  reviewedText?: string | undefined;
  transcriptionAttempt?: string | undefined;
  error?: string | undefined;
  deletedAt?: number | undefined;
};
export function createRecordingStorage(
  server = serverViewId ?? "local",
  buffer = unsentStore(server, "recording"),
) {
  const records = new Map<string, Recording>();
  const chunks = new Map<string, Map<number, Int16Array>>();
  let loading: Promise<void> | undefined;
  const ready = () =>
    (loading ??= (async () => {
      for (const [key, value] of await buffer.loadAll()) {
        try {
          if (key.startsWith("record:") && typeof value === "string") {
            const row: Recording = JSON.parse(value);
            if (typeof row.id === "string" && typeof row.chatId === "string")
              records.set(row.id, row);
          } else if (key.startsWith("chunk:") && value instanceof Blob) {
            const identity: unknown = JSON.parse(key.slice(6));
            if (
              !Array.isArray(identity) ||
              typeof identity[0] !== "string" ||
              typeof identity[1] !== "number"
            )
              continue;
            let parts = chunks.get(identity[0]);
            if (!parts) chunks.set(identity[0], (parts = new Map()));
            parts.set(identity[1], new Int16Array(await value.arrayBuffer()));
          }
        } catch {
          /* Ignore damaged recording entries. */
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
    await buffer.put(recordKey(row.id), JSON.stringify(row));
    records.set(row.id, { ...row });
  }
  async function saveChunk(row: Recording, index: number, pcm: Int16Array) {
    await ready();
    await buffer.put(chunkKey(row.id, index), new Blob([pcm.slice().buffer]));
    await buffer.put(recordKey(row.id), JSON.stringify(row));
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
      await buffer.put(recordKey(row.id), JSON.stringify(next));
      records.set(row.id, next);
    }
  }
  async function beginTranscription(row: Recording, attempt: string) {
    await ready();
    const current = records.get(row.id);
    if (current?.chatId !== row.chatId) return false;
    await buffer.put(
      recordKey(row.id),
      JSON.stringify({ ...current, transcriptionAttempt: attempt }),
    );
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
