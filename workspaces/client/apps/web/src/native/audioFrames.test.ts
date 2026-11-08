import { expect, it, vi } from "vitest";
import { bindAudioFrames } from "./audioFrames";
async function caller(audio: ReturnType<typeof bindAudioFrames>) {
  const client = audio.createClient();
  const received = vi.fn();
  await client.listen(received);
  return { ...client, received };
}
it("binds speech permits, uploads, progress and cancellation to the initiating owner", async () => {
  let events: (value: unknown) => void = () => {};
  let finish!: (value: unknown) => void;
  const invoke = vi.fn(async (method: string) => {
    if (method === "desktop.prepareTranscription") return "permit";
    if (method === "speech.uploadBegin") return { transfer: "upload" };
    if (method === "speech.transcribe")
      return new Promise((resolve) => {
        finish = resolve;
      });
    return true;
  });
  const audio = bindAudioFrames(invoke, async (callback) => {
    events = callback;
    return vi.fn<() => void>();
  });
  const a = await caller(audio),
    b = await caller(audio);
  await a.request("desktop.prepareTranscription", {}, "prepare");
  await expect(
    b.request("speech.uploadBegin", { permit: "permit", size: 4 }, "bad"),
  ).rejects.toThrow("another view");
  await a.request("speech.uploadBegin", { permit: "permit", size: 4 }, "begin");
  await expect(
    b.request("speech.uploadChunk", { transfer: "upload" }, "bad"),
  ).rejects.toThrow("another view");
  await a.request(
    "view.subscribe",
    { topic: "transcription", subscription: "progress" },
    "sub",
  );
  const subscription = (
    invoke.mock.calls.find(
      (call) => call[0] === "view.subscribe",
    ) as unknown as [string, { subscription: string }]
  )[1].subscription;
  const task = a.request(
    "speech.transcribe",
    {
      transfer: "upload",
      permit: "permit",
      id: "attempt",
      locale: "en",
      recording: "private-record",
    },
    "run",
  );
  expect(invoke).toHaveBeenLastCalledWith(
    "speech.transcribe",
    { transfer: "upload", permit: "permit", id: "attempt", locale: "en" },
    "run",
  );
  events({ event: subscription, value: { id: "other", completed: 1 } });
  expect(a.received).not.toHaveBeenCalled();
  events({ event: subscription, value: { id: "attempt", completed: 1 } });
  expect(a.received).toHaveBeenCalledOnce();
  await expect(
    b.request("desktop.cancelTranscription", { value: "attempt" }, "bad"),
  ).rejects.toThrow("another view");
  await a.request(
    "desktop.cancelTranscription",
    { value: "attempt" },
    "cancel",
  );
  finish({ text: "done" });
  await task;
  await expect(
    a.request(
      "speech.transcribe",
      { transfer: "upload", permit: "permit", id: "retry" },
      "retry",
    ),
  ).rejects.toThrow();
  audio.dispose();
});
it("rewrites microphone identities and stops a detached owner without affecting other owners", async () => {
  let events: (value: unknown) => void = () => {};
  const invoke = vi.fn(
    async (_method: string, _params: unknown, _id?: string) => true,
  );
  const audio = bindAudioFrames(invoke, async (callback) => {
    events = callback;
    return vi.fn<() => void>();
  });
  const a = await caller(audio),
    b = await caller(audio);
  await a.request("desktop.requestMicrophone", {}, "mic");
  await expect(
    b.request(
      "view.subscribe",
      { topic: "microphone", subscription: "spy", capture: "c" },
      "spy",
    ),
  ).rejects.toThrow();
  await a.request(
    "view.subscribe",
    { topic: "microphone", subscription: "audio", capture: "c" },
    "sub",
  );
  const native = (
    invoke.mock.calls.find((call) => call[0] === "view.subscribe")![1] as {
      subscription: string;
    }
  ).subscription;
  expect(native).not.toBe("audio");
  await a.request("media.captureStart", { capture: "c" }, "start");
  await expect(
    b.request("media.captureStop", { capture: "c" }, "stop"),
  ).rejects.toThrow();
  events({ event: native, value: { pcm: "AIAAAP9/" } });
  expect(a.received).toHaveBeenCalledWith({
    event: "audio",
    value: { pcm: "AIAAAP9/" },
  });
  expect(b.received).not.toHaveBeenCalled();
  a.close();
  await b.request("desktop.requestMicrophone", {}, "new-owner");
  expect(invoke).toHaveBeenCalledWith("media.captureStop", { capture: native });
  audio.dispose();
});

it("keeps the Rust voice peer and its events private to its owning server", async () => {
  let events: (value: unknown) => void = () => {};
  const invoke = vi.fn(
    async (_method: string, _params: unknown, _id?: string) => true,
  );
  const audio = bindAudioFrames(invoke, async (callback) => {
    events = callback;
    return () => {};
  });
  const a = await caller(audio),
    b = await caller(audio);
  await a.request("desktop.requestMicrophone", {}, "mic");
  await a.request(
    "view.subscribe",
    { topic: "voice", subscription: "events", session: "logical" },
    "sub",
  );
  const params = invoke.mock.calls.find(
    (call) => call[0] === "view.subscribe",
  )![1] as { session: string; subscription: string };
  expect(params.session).toBe(params.subscription);
  await a.request("voice.create", { session: "logical" }, "start");
  await expect(
    b.request("voice.answer", { session: "logical", sdp: "foreign" }, "bad"),
  ).rejects.toThrow();
  await expect(
    b.request("voice.close", { session: "logical" }, "bad-close"),
  ).rejects.toThrow();
  events({
    event: params.subscription,
    value: { type: "state", state: "connected" },
  });
  expect(a.received).toHaveBeenCalledOnce();
  expect(b.received).not.toHaveBeenCalled();
  audio.dispose();
  expect(invoke).toHaveBeenCalledWith("voice.close", {
    session: params.session,
  });
});
it("isolates direct server owners and releases capture when an owner closes", async () => {
  let events: (value: unknown) => void = () => {};
  const invoke = vi.fn(
    async (_method: string, _params: unknown, _id?: string) => true,
  );
  const audio = bindAudioFrames(invoke, async (callback) => {
    events = callback;
    return () => {};
  });
  const a = audio.createClient();
  const b = audio.createClient();
  const receivedA = vi.fn();
  const receivedB = vi.fn();
  await a.listen(receivedA);
  await b.listen(receivedB);
  await a.request("desktop.requestMicrophone", {}, "gesture-a");
  await expect(b.request("desktop.requestMicrophone", {})).rejects.toThrow(
    "Another server",
  );
  await a.request("view.subscribe", {
    topic: "microphone",
    subscription: "same",
    capture: "same",
  });
  const nativeA = (
    invoke.mock.calls.find((call) => call[0] === "view.subscribe")![1] as {
      subscription: string;
    }
  ).subscription;
  await a.request("media.captureStart", { capture: "same" });
  events({ event: nativeA, value: { pcm: "AAAA" } });
  expect(receivedA).toHaveBeenCalledWith({
    event: "same",
    value: { pcm: "AAAA" },
  });
  expect(receivedB).not.toHaveBeenCalled();
  await expect(
    b.request("media.captureStop", { capture: "same" }),
  ).rejects.toThrow();
  a.close();
  expect(invoke).toHaveBeenCalledWith("media.captureStop", {
    capture: nativeA,
  });
  expect(invoke).toHaveBeenCalledWith("view.unsubscribe", {
    subscription: nativeA,
  });
  await expect(a.request("desktop.requestMicrophone", {})).rejects.toThrow(
    "closed",
  );
  await b.request("desktop.requestMicrophone", {}, "gesture-b");
  await b.request("view.subscribe", {
    topic: "microphone",
    subscription: "same",
    capture: "same",
  });
  const nativeB = (
    invoke.mock.calls
      .filter((call) => call[0] === "view.subscribe")
      .at(-1)![1] as { subscription: string }
  ).subscription;
  expect(nativeB).not.toBe(nativeA);
  events({ event: nativeA, value: { pcm: "AAAA" } });
  expect(receivedA).toHaveBeenCalledTimes(1);
  expect(receivedB).not.toHaveBeenCalled();
  events({ event: nativeB, value: { pcm: "BBBB" } });
  expect(receivedB).toHaveBeenCalledOnce();
  await b.request(
    "view.unsubscribe",
    { subscription: "same" },
    "unsubscribe-b",
  );
  expect(invoke).toHaveBeenLastCalledWith(
    "view.unsubscribe",
    { subscription: nativeB },
    "unsubscribe-b",
  );
  b.close();
  audio.dispose();
});
