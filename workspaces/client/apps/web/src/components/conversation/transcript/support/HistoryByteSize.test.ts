import { autorun, observable, runInAction } from "mobx";
import { expect, it, vi } from "vitest";
import { TranscriptHistory } from "../../../../conversation/historySession.ts";
import type { Message } from "../../../../types.ts";

it("serializes only the changed retained item and observes nested metadata", () => {
  const history = new TranscriptHistory(
    null,
    undefined,
    {
      syncGet: async () => {
        throw new Error("unused");
      },
    },
    {
      watchResourceConnection: () => () => {},
      watchLiveDeltas: () => () => {},
    },
  );
  const error = observable({ message: "old" });
  const first = observable<Message>({
    id: "a",
    role: "assistant",
    text: "a",
    nativeError: error,
  });
  const second = observable<Message>({ id: "b", role: "assistant", text: "b" });
  history.items = [first, second];
  let bytes = 0;
  const stop = autorun(() => {
    bytes = history.size;
  });
  const stringify = vi.spyOn(JSON, "stringify");
  try {
    runInAction(() => {
      second.text += "😀";
    });
    expect(stringify.mock.calls.map(([item]) => item)).toEqual([second]);
    stringify.mockClear();
    runInAction(() => {
      error.message = "longer reason";
    });
    expect(stringify.mock.calls.map(([item]) => item)).toEqual([first]);
    expect(bytes).toBe(
      history.items.reduce(
        (sum, item) => sum + JSON.stringify(item).length * 2,
        0,
      ),
    );
  } finally {
    stringify.mockRestore();
    stop();
  }
});
