import type { ConnectionHealth } from "../../../sync/connectionState.ts";
import { watchResourceConnection } from "../../../sync/client.ts";

export function watchVoiceTransport(
  onState: (state: ConnectionHealth) => void,
  stopVoice: () => void,
  watchConnection: typeof watchResourceConnection = watchResourceConnection,
): () => void {
  return watchConnection((state) => {
    onState(state);
    if (state !== "live") stopVoice();
  });
}
