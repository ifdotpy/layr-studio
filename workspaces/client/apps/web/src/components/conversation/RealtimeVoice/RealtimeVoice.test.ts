import { autorun } from "mobx";
import { expect, it, vi } from "vitest";
import { agentId } from "../../../generated/observableResources.ts";
import { resourceData } from "../../../store/runtime.ts";
import { transientTestFixture } from "../../../sync/subscriptions/transientTestFixture.ts";
import { VoiceRecordKind, type components } from "../../../generated/api.ts";

type Batch = components["schemas"]["VoiceRecordBatch"];
const row = (id: string): Batch["records"][number] => ({
  id,
  agent: "chat",
  created: 1,
  item_id: id,
  kind: VoiceRecordKind.user,
  payload: "",
  previous_item_id: "",
  seq: 1,
  session: "session",
  text: id,
});
it("the observed voice facade merges inline intervals, recovers only announced gaps and resets on epoch change", async () => {
  vi.useFakeTimers();
  const recover = vi.fn(
    async (_agent: string, after: number): Promise<Batch> => ({
      after,
      cursor: 4,
      records: [row("recovered")],
      session: null,
    }),
  );
  const f = transientTestFixture({ voice: recover, authProgress: vi.fn() });
  const slot = f.store.voice(agentId("chat"));
  const stop = autorun(() => {
    void slot.current;
  });
  const emit = (
    revision: number,
    after: number,
    cursor: number,
    ids: string[],
    epoch = "one",
  ) =>
    f.emit(
      { kind: "voice", agentId: "chat" },
      {
        epoch,
        revision,
        voiceRecords: { after, cursor, records: ids.map(row), session: null },
      },
    );
  const ids = () => resourceData(slot.current)?.records.map((row) => row.id);
  try {
    await vi.advanceTimersByTimeAsync(100);
    expect(recover).not.toHaveBeenCalled();
    emit(1, 0, 1, ["first"]);
    await vi.advanceTimersByTimeAsync(0);
    emit(2, 1, 2, ["second", "first"]);
    await vi.advanceTimersByTimeAsync(0);
    expect(ids()).toEqual(["first", "second"]);
    expect(recover).not.toHaveBeenCalled();
    emit(3, 3, 4, []);
    await vi.advanceTimersByTimeAsync(0);
    expect(recover).toHaveBeenCalledExactlyOnceWith("chat", 2);
    expect(ids()).toEqual(["first", "second", "recovered"]);
    emit(1, 0, 1, ["new"], "two");
    await vi.advanceTimersByTimeAsync(0);
    expect(ids()).toEqual(["new"]);
    expect(recover).toHaveBeenCalledOnce();
  } finally {
    stop();
    f.close();
    vi.useRealTimers();
  }
});
