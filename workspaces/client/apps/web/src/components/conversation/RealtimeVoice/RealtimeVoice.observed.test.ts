// @vitest-environment happy-dom
import { act, createElement } from "react";
import { createRoot } from "react-dom/client";
import { MantineProvider } from "@mantine/core";
import { observable, runInAction } from "mobx";
import { expect, it, vi } from "vitest";
import type { ResourceSlot, ResourceStatus } from "../../../store/runtime.ts";
import type { VoiceData } from "../../../generated/observableResources.ts";
import { VoiceSessionState } from "../../../generated/api.ts";

const fixture = vi.hoisted(() => ({
  slot: undefined as ResourceSlot<VoiceData> | undefined,
  post: vi.fn(),
  answer: vi.fn(async (_sdp: string) => {}),
  close: vi.fn(async () => {}),
}));
vi.mock("../../../native/client.ts", () => ({ hasNativeClient: () => true }));
vi.mock(
  "../../servers/ServerServicesProvider/ServerServicesProvider.tsx",
  () => ({
    useServerStore: () => ({ voice: () => fixture.slot }),
    useServerApi: () => ({ post: fixture.post }),
    useServerResources: () => ({
      watchResourceConnection: (accept: (status: "live") => void) => {
        accept("live");
        return () => {};
      },
    }),
    useServerAudio: () => ({
      dictationBridge: () => ({ requestMicrophone: async () => {} }),
      nativeVoice: () => ({
        offer: async () => "offer",
        answer: fixture.answer,
        close: fixture.close,
        mute: async () => {},
      }),
    }),
  }),
);
import RealtimeVoice from "./RealtimeVoice.tsx";

it("stops an established capture when gap recovery leaves cached voice data stale with an error", async () => {
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
  const data: VoiceData = { after: 0, cursor: 1, records: [], session: null };
  const snapshot = observable.box<ResourceStatus<VoiceData>>(
    { status: "live", data },
    { deep: false },
  );
  fixture.slot = {
    get current() {
      return snapshot.get()!;
    },
  };
  let sessionId = "";
  fixture.post.mockImplementation(
    async (path: string, body: { session_id?: string }) => {
      if (path === "/api/voice/start") {
        sessionId = body.session_id!;
        return {
          session_id: sessionId,
          sdp: "answer",
          ended: null,
          error: null,
          state: VoiceSessionState.ready,
        };
      }
      return {};
    },
  );
  const host = document.createElement("div");
  document.body.append(host);
  const mounted = createRoot(host);
  try {
    await act(async () =>
      mounted.render(
        createElement(
          MantineProvider,
          null,
          createElement(RealtimeVoice, { agentId: "chat", notify: vi.fn() }),
        ),
      ),
    );
    await act(async () =>
      host
        .querySelector<HTMLButtonElement>('[aria-label="Voice conversation"]')!
        .click(),
    );
    const start = [
      ...document.querySelectorAll<HTMLButtonElement>("button"),
    ].find((button) => button.textContent === "Start voice")!;
    await act(async () => start.click());
    expect(fixture.answer).toHaveBeenCalledExactlyOnceWith("answer");
    expect(sessionId).not.toBe("");
    expect(fixture.close).not.toHaveBeenCalled();
    const cached = {
      ...data,
      session: {
        session_id: sessionId,
        sdp: "cached",
        ended: null,
        error: null,
        state: VoiceSessionState.ready,
      },
    };
    await act(async () =>
      runInAction(() =>
        snapshot.set({
          status: "stale",
          data: cached,
          error: new Error("gap read rejected"),
        }),
      ),
    );
    expect(fixture.close).toHaveBeenCalledOnce();
    expect(
      fixture.post.mock.calls.filter(([path]) => path === "/api/voice/end"),
    ).toHaveLength(1);
    expect(document.body.textContent).toContain("The microphone is off.");
    expect(document.body.textContent).toContain("gap read rejected");
  } finally {
    await act(async () => mounted.unmount());
    host.remove();
    vi.unstubAllGlobals();
    vi.clearAllMocks();
  }
});
