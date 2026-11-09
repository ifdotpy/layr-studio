declare global {
  interface Window {
    stopTranscript: () => void;
    cursor: () => Promise<number | undefined>;
    cacheRecord: () => Promise<unknown>;
  }
}
export const initializeTranscript = async () => {
  const { createServerServices } =
    await import("/src/servers/serverServices.ts");
  const { indexedDb } = await import("/src/indexedDb.ts");
  const { subscriptionKey } =
    await import("/src/sync/subscriptions/contracts.ts");
  const services = createServerServices({
    id: "local",
    label: "Fixture",
    origin: location.origin,
  });
  const resource = { kind: "transcript", agentId: "open" } as const;
  const stop = services.sync.subscribeTranscriptProjection(
    "transcript:open",
    (payload) => {
      document.querySelector("main")!.textContent =
        payload?.items.map((item) => item.text).join("\n") || "";
    },
    (error) => {
      if (error) throw error;
    },
  );
  window.stopTranscript = () => {
    stop();
    services.close();
  };
  window.cursor = async () =>
    (await services.sync.readSubscriptionCursor(resource))?.seq;
  const storage = indexedDb(
    "studio-subscription-cache:local",
    "subscriptions",
    1,
  );
  window.cacheRecord = async () => {
    const { workspaceId } = await services.sync.syncDatabase();
    return storage.get(
      JSON.stringify([workspaceId, subscriptionKey(resource)]),
    );
  };
};
