import { beforeEach, afterEach } from "vitest";
// @vitest-environment happy-dom
import { act, createElement } from "react";
import { createRoot } from "react-dom/client";
import { MantineProvider } from "@mantine/core";
import { observable, action } from "mobx";
import { NetStatus } from "../../../servers/net/status.ts";
import { expect, it, vi } from "vitest";
import type { ResourceSlot, ResourceStatus } from "../../../store/runtime.ts";
import type { VoiceData } from "../../../generated/observableResources.ts";
import { VoiceSessionState } from "../../../generated/api.ts";

const fixture = vi.hoisted(() => ({
  slot: undefined as ResourceSlot<VoiceData> | undefined,
  connection: undefined as NetStatus | undefined,
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
    useServerNet: () => ({ status: fixture.connection }),
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
import RealtimeVoice, { createRealtimeVoice } from "./RealtimeVoice.tsx";
import {
  useServerStore as fixtureStore,
  useServerApi as fixtureApi,
  useServerAudio as fixtureAudio,
} from "../../servers/ServerServicesProvider/ServerServicesProvider.tsx";

it.each(["stale", "connecting", "degraded", "offline", "unmount"] as const)(
  "stops established capture on %s without repeating voice/end",
  async (reason) => {
    fixture.connection = new NetStatus();
    fixture.connection.receive("live");
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
    const voiceStore = createRealtimeVoice({
      agentId: "chat",
      notify: vi.fn(),
      store: fixtureStore(),
      api: fixtureApi(),
      audio: fixtureAudio(),
      connection: fixture.connection,
    });
    const host = document.createElement("div");
    document.body.append(host);
    const mounted = createRoot(host);
    try {
      await act(async () =>
        mounted.render(
          createElement(
            MantineProvider,
            null,
            createElement(RealtimeVoice, {
              voiceStore,
            }),
          ),
        ),
      );
      await act(async () =>
        host
          .querySelector<HTMLButtonElement>(
            '[aria-label="Voice conversation"]',
          )!
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
      if (reason === "unmount") await act(async () => mounted.unmount());
      else if (reason !== "stale")
        await act(async () => fixture.connection?.receive(reason));
      else {
        await act(async () =>
          action(() =>
            snapshot.set({
              status: "stale",
              data: cached,
              error: new Error("gap read rejected"),
            }),
          )(),
        );
      }
      expect(fixture.close).toHaveBeenCalledOnce();
      expect(voiceStore.active).toBe(false);
      expect(
        fixture.post.mock.calls.filter(([path]) => path === "/api/voice/end"),
      ).toHaveLength(1);
      if (reason !== "unmount")
        expect(document.body.textContent).toContain("The microphone is off.");
      if (reason === "stale")
        expect(document.body.textContent).toContain("gap read rejected");
    } finally {
      await act(async () => mounted.unmount());
      host.remove();
      vi.unstubAllGlobals();
      vi.clearAllMocks();
    }
  },
);

let renderErrors: ReturnType<typeof vi.spyOn>;
beforeEach(() => {
  renderErrors = vi.spyOn(console, "error");
});
afterEach(() => {
  const warnings = renderErrors.mock.calls.filter((args: unknown[]) =>
    args.some((arg: unknown) =>
      String(arg).includes("Cannot update a component"),
    ),
  );
  renderErrors.mockRestore();
  expect(warnings).toEqual([]);
});
