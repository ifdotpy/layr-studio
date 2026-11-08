import { randomUUID } from "../uuid";

const REQUEST = "studio-native-frame-request";
const RESPONSE = "studio-native-frame-response";
const SHELL_ORIGIN = "tauri://localhost";

export function isNativeFrame() {
  return globalThis.location?.protocol === "studio-view:";
}
// TODO(attar-sync): Replace this postMessage request/reply hop with Attar's
// synchronous, owner-scoped native bridge for local commits. Preserve server
// isolation and the original request ID; do not make network calls synchronous.
export function frameRequest<T>(
  method: string,
  params: unknown,
  id: string = randomUUID(),
): Promise<T> {
  return new Promise((resolve, reject) => {
    const cleanup = () => {
      clearTimeout(timeout);
      window.removeEventListener("message", receive);
      window.removeEventListener("pagehide", closed);
    };
    const closed = () => {
      cleanup();
      reject(
        new Error(
          "The server view closed. Pending command outcomes may be unknown.",
        ),
      );
    };
    const receive = (event: MessageEvent) => {
      if (
        event.source !== window.parent ||
        event.origin !== SHELL_ORIGIN ||
        event.data?.kind !== RESPONSE ||
        event.data.id !== id
      )
        return;
      cleanup();
      if (event.data.error) reject(new Error(event.data.error));
      else resolve(event.data.result as T);
    };
    const timeout = setTimeout(
      () => {
        cleanup();
        reject(
          new Error(
            "The native service did not respond. The request was not repeated.",
          ),
        );
      },
      method === "speech.transcribe"
        ? 76 * 60_000
        : method === "desktop.requestMicrophone"
          ? 5 * 60_000
          : 30_000,
    );
    window.addEventListener("message", receive);
    window.addEventListener("pagehide", closed, { once: true });
    window.parent.postMessage(
      { kind: REQUEST, id, method, params },
      SHELL_ORIGIN,
    );
  });
}
