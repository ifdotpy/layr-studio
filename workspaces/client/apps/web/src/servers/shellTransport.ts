import { requestId as randomUUID } from "studio-claude-bridge/request-id.ts";
import type { CredentialBridge } from "../../../desktop/api.ts";
import type { StreamChunk } from "./net/desktopCredentials.ts";
import { serverParentOrigin, serverViewId } from "./environment.ts";

export function shellCredentialBridge(): CredentialBridge {
  const pending = new Map<string, (event: MessageEvent) => void>();
  const streams = new Set<(value: StreamChunk) => void>();
  window.addEventListener("message", (event) => {
    if (event.source !== window.parent || event.origin !== serverParentOrigin)
      return;
    if (
      event.data?.kind === "studio-server-stream" &&
      event.data.value?.serverId === serverViewId
    )
      for (const listener of streams) listener(event.data.value);
    if (event.data?.kind === "studio-server-transport-result") {
      const handler = pending.get(event.data.correlation);
      if (!handler) return;
      pending.delete(event.data.correlation);
      handler(event);
    }
  });
  return {
    serverCredentialAction: (value) =>
      new Promise((resolve, reject) => {
        const correlation = randomUUID();
        // Parent origin, source and correlation were checked before delivery.
        pending.set(correlation, (event) => {
          if (event.data.error) reject(new Error(event.data.error));
          else resolve(event.data.result);
        });
        window.parent.postMessage(
          { kind: "studio-server-transport", correlation, value },
          serverParentOrigin,
        );
      }),
    onServerStream: (callback) => {
      streams.add(callback);
      return () => {
        streams.delete(callback);
      };
    },
  };
}
