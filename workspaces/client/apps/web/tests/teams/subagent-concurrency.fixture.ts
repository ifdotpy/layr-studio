declare global {
  interface Window {
    concurrencyStorageFixture: (leadId: string) => string;
  }
}

/** Installed with the same scope that own the current renderer receipt. */
export function installConcurrencyStorageFixture({
  stateDir,
  workspaceId,
}: {
  stateDir: string;
  workspaceId: string;
}) {
  window.concurrencyStorageFixture = (leadId) =>
    `studio-agent-mode:${JSON.stringify([stateDir, workspaceId, leadId])}`;
}
