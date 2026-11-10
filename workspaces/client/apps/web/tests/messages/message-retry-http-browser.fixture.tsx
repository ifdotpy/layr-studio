import type {} from "../drafts/draft-fixture-helpers.ts";
const initialize = async () => {
  const send = await import("../../src/sync/send.ts");
  const { watchResourceChanges } =
    await import("../../src/sync/subscriptions/events.ts");
  const { waitForStreamSession } = await import("../../src/servers/net/api.ts");
  watchResourceChanges({ kind: "state" }, () => {});
  await waitForStreamSession();
  window.retrySendSend = send.durableSend;
  window.outbox = { entries: [], error: "" };
  const stop = send.observeOutbox(
    (entries) => {
      window.outbox.entries = entries;
    },
    (error) => {
      window.outbox.error = error;
    },
  );
  window.addEventListener("pagehide", stop, { once: true });
};
window.retrySendInitialize = initialize;

declare global {
  interface Window {
    retrySendInitialize: typeof initialize;
    retrySendSend: typeof import("../../src/sync/send.ts").durableSend;
    sendResult: ReturnType<typeof import("../../src/sync/send.ts").durableSend>;
  }
}
