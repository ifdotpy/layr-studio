// @vitest-environment happy-dom
import { act, createElement } from "react";
import { createRoot } from "react-dom/client";
import { MantineProvider } from "@mantine/core";
import { autorun } from "mobx";
import { afterEach, expect, it, vi } from "vitest";
import { createRecordingStorage } from "../../../dictation/storage.ts";
import type { RecordingValue } from "../../../unsent/store.ts";
import type { DesktopBridge } from "../../../desktop.ts";
import { DictationStore } from "../DictationPanel/DictationPanel.tsx";

const fixture = vi.hoisted(() => {
  const recordings: { current?: ReturnType<typeof createRecordingStorage> } =
    {};
  return {
    recordings,
    permission: vi.fn(async () => true),
    prepare: vi.fn(async () => "permit"),
    transcribe: vi.fn(
      async (_value: Parameters<DesktopBridge["transcribeAudio"]>[0]) => ({
        text: "spoken instruction",
      }),
    ),
    cancel: vi.fn(async (_id: string) => true),
    progress: vi.fn(
      (
        _accept: (value: {
          id: string;
          completed: number;
          total: number;
        }) => void,
      ) =>
        () => {},
    ),
    stop: vi.fn(async () => {}),
    capture: vi.fn(async (_accept: (pcm: Int16Array) => void) => ({
      sampleRate: 16000,
      stop: async () => {},
    })),
  };
});
vi.mock("../../../native/client.ts", () => ({ hasNativeClient: () => false }));
vi.mock("../../../servers/activity.ts", () => ({
  useServerActivity: () => {},
}));
vi.mock(
  "../../servers/ServerServicesProvider/ServerServicesProvider.tsx",
  () => ({
    useServerRecordings: () => fixture.recordings.current,
    useServerAudio: () => ({
      dictationBridge: () => ({
        requestMicrophone: fixture.permission,
        prepareTranscription: fixture.prepare,
        transcribeAudio: fixture.transcribe,
        cancelTranscription: fixture.cancel,
        onTranscriptionProgress: fixture.progress,
      }),
      captureAudio: fixture.capture,
    }),
  }),
);
import { Dictation } from "./Dictation.tsx";

function storage() {
  const data = new Map<string, RecordingValue>();
  return createRecordingStorage("dictation-stores", {
    put: async (key, value) => {
      data.set(key, value);
    },
    putChunk: async (key, value, chunkKey, chunk) => {
      data.set(key, value);
      data.set(chunkKey, chunk);
    },
    delete: async (key) => {
      data.delete(key);
    },
    loadAll: async () => [...data],
  });
}
function button(name: string | RegExp) {
  const result = [
    ...document.querySelectorAll<HTMLButtonElement>("button"),
  ].find((button) =>
    typeof name === "string"
      ? button.textContent?.trim() === name
      : name.test(button.textContent?.trim() ?? ""),
  );
  if (!result) throw Error(`Missing button ${name}`);
  return result;
}
async function mount(
  prepare?: (recordings: ReturnType<typeof storage>) => void,
) {
  vi.stubGlobal("IS_REACT_ACT_ENVIRONMENT", true);
  window.matchMedia = vi.fn(() => ({
    matches: false,
    media: "",
    onchange: null,
    addListener() {},
    removeListener() {},
    addEventListener() {},
    removeEventListener() {},
    dispatchEvent: () => true,
  }));
  vi.spyOn(URL, "createObjectURL").mockReturnValue("blob:recording");
  vi.spyOn(URL, "revokeObjectURL").mockImplementation(() => {});
  fixture.recordings.current = storage();
  prepare?.(fixture.recordings.current);
  fixture.capture.mockImplementation(async () => ({
    sampleRate: 16000,
    stop: fixture.stop,
  }));
  const host = document.createElement("div");
  document.body.append(host);
  const root = createRoot(host);
  const insert = vi.fn();
  const render = async (chatId: string) =>
    act(async () =>
      root.render(
        createElement(
          MantineProvider,
          null,
          createElement(Dictation, { chatId, onInsert: insert }),
        ),
      ),
    );
  await render("first");
  await act(async () =>
    host.querySelector<HTMLButtonElement>('[aria-label="Dictation"]')?.click(),
  );
  return {
    insert,
    render,
    cleanup: async () => {
      await act(async () => root.unmount());
      host.remove();
    },
  };
}
afterEach(() => {
  vi.restoreAllMocks();
  vi.clearAllMocks();
  vi.unstubAllGlobals();
});

it("keeps native and browser dictation state observable with one review/lock owner", () => {
  const state = new DictationStore();
  const eligibility: boolean[] = [];
  const dispose = autorun(() => eligibility.push(state.canInsert));
  try {
    state.update({ text: "reviewed", locale: "pl-PL", open: true });
    state.update({ active: true, status: "Listening", partial: "draft" });
    state.update({ active: false, partial: "", error: "connection lost" });
    expect(eligibility).toEqual([false, true, false, true]);
    expect(state.text).toBe("reviewed");
    expect(state.locale).toBe("pl-PL");
    expect(state.error).toBe("connection lost");
  } finally {
    dispose();
  }
});

it("requests permission within Record, stops capture on chat switch, and revokes playback URLs", async () => {
  const app = await mount();
  try {
    expect(fixture.permission).not.toHaveBeenCalled();
    const record = button("Record");
    await act(async () => {
      record.click();
      expect(fixture.permission).toHaveBeenCalledOnce();
      expect(fixture.capture).not.toHaveBeenCalled();
    });
    expect(button(/^Stop ·/).disabled).toBe(false);
    const accept = fixture.capture.mock.calls[0]?.[0];
    if (!accept) throw Error("missing capture callback");
    await act(async () => accept(new Int16Array([1, 2])));
    await app.render("second");
    expect(fixture.stop).toHaveBeenCalledOnce();
    expect(app.insert).not.toHaveBeenCalled();
    const rows = await fixture.recordings.current?.listRecordings("first");
    expect(rows?.[0]?.samples).toBe(2);
    await app.render("first");
    expect(URL.createObjectURL).toHaveBeenCalledOnce();
    await app.render("second");
    expect(URL.revokeObjectURL).toHaveBeenCalledWith("blob:recording");
  } finally {
    await app.cleanup();
  }
});

it("preserves pending transcription identity, cancellation, review, delete and undo through the caller", async () => {
  const app = await mount();
  const pending = Promise.withResolvers<{ text: string }>();
  fixture.transcribe.mockImplementationOnce(() => pending.promise);
  try {
    await act(async () => button("Record").click());
    const accept = fixture.capture.mock.calls[0]?.[0];
    if (!accept) throw Error("missing capture callback");
    await act(async () => accept(new Int16Array([1, 2])));
    await act(async () => button(/^Stop ·/).click());
    const attempt = fixture.transcribe.mock.calls[0]?.[0];
    expect(attempt).toBeDefined();
    expect(button("Record").disabled).toBe(true);
    expect(app.insert).not.toHaveBeenCalled();
    await act(async () => button("Cancel transcription").click());
    expect(fixture.cancel).toHaveBeenCalledOnce();
    await act(async () => pending.resolve({ text: "late result" }));
    expect(document.body.textContent).toContain("Transcription cancelled");
    expect(document.querySelector("audio")).not.toBeNull();
    await act(async () => button("Retry transcription").click());
    expect(fixture.transcribe).toHaveBeenCalledTimes(2);
    expect(
      document.querySelector<HTMLTextAreaElement>(
        '[aria-label="Review dictated text"]',
      )?.value,
    ).toBe("spoken instruction");
    await act(async () =>
      document
        .querySelector<HTMLButtonElement>('[aria-label="Delete recording"]')
        ?.click(),
    );
    expect(document.querySelector("audio")).toBeNull();
    await act(async () => button("Undo delete").click());
    expect(document.querySelector("audio")).not.toBeNull();
    expect(app.insert).not.toHaveBeenCalled();
  } finally {
    pending.resolve({ text: "late result" });
    await app.cleanup();
  }
});

it("discards a permission result delivered after leaving the capture scope", async () => {
  const permission = Promise.withResolvers<boolean>();
  fixture.permission.mockImplementationOnce(() => permission.promise);
  const app = await mount();
  try {
    await act(async () => button("Record").click());
    await app.cleanup();
    await act(async () => permission.resolve(true));
    expect(fixture.capture).not.toHaveBeenCalled();
    expect(await fixture.recordings.current?.listRecordings("first")).toEqual(
      [],
    );
  } finally {
    permission.resolve(true);
  }
});

it("shows storage failure while retaining durable audio and stopping capture", async () => {
  const app = await mount((recordings) => {
    const saveChunk = recordings.saveChunk;
    vi.spyOn(recordings, "saveChunk")
      .mockImplementationOnce(saveChunk)
      .mockRejectedValueOnce(Error("storage unavailable"));
  });
  try {
    await act(async () => button("Record").click());
    const accept = fixture.capture.mock.calls[0]?.[0];
    if (!accept || !fixture.recordings.current) throw Error("missing capture");
    await act(async () => accept(new Int16Array([1, 2])));
    await act(async () => accept(new Int16Array([3, 4])));
    expect(fixture.stop).toHaveBeenCalledOnce();
    expect(document.body.textContent).toContain(
      "Saved audio remains available",
    );
    expect(
      (await fixture.recordings.current.listRecordings("first"))[0]?.samples,
    ).toBe(2);
    expect(document.querySelector("audio")).not.toBeNull();
    expect(app.insert).not.toHaveBeenCalled();
  } finally {
    await app.cleanup();
  }
});
