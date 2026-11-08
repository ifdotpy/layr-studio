declare global {
  interface Window {
    syncErrors: string[];
    stopConnection: () => void;
    stopProjection: () => void;
  }
}
export const initializeDelivery = async () => {
  const client = await import("/src/sync/client.ts");
  window.nativeState = null;
  window.syncErrors = [];
  window.connection = "closed";
  window.stopConnection = client.watchResourceConnection(
    (state) => (window.connection = state),
  );
  const { observeGeneratedState } =
    await import("/src/sync/generatedState.testHelper.ts");
  window.stopProjection = observeGeneratedState(
    client,
    (state) => {
      window.nativeState = state;
    },
    (error) => {
      if (error) window.syncErrors.push(String(error));
    },
  ).stop;
  window.checkpoint = async () => {
    const { db } = await client.syncDatabase();
    return (await db.projections.findOne("state:entities:checkpoint").exec())
      ?.seq;
  };
};
