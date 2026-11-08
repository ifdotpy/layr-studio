import { nativeHost } from "./client";
import { isNativeFrame } from "./frame";
import { prepareNativeClose } from "./closeState";
import { serverParentOrigin } from "../servers/environment";

const QUERY = "studio-native-close-query";
const REPLY = "studio-native-close-reply";
const RESUME = "studio-native-close-resume";

function freeze() {
  const previous = document.documentElement.inert;
  document.documentElement.inert = true;
  return () => {
    document.documentElement.inert = previous;
  };
}

/** Install before React mounts, so every editable native view joins close checks. */
export async function initializeNativeClose() {
  if (isNativeFrame()) {
    let active: { id: string; release: () => void } | undefined;
    window.addEventListener("message", async (event) => {
      if (event.source !== window.parent || event.origin !== serverParentOrigin)
        return;
      const { kind, id } = event.data || {};
      if (kind === RESUME && active && active.id === id) {
        active.release();
        active = undefined;
      }
      if (kind !== QUERY || typeof id !== "string" || active) return;
      active = { id, release: freeze() };
      try {
        await prepareNativeClose();
        window.parent.postMessage(
          { kind: REPLY, id, ready: true },
          serverParentOrigin,
        );
      } catch (error) {
        window.parent.postMessage(
          { kind: REPLY, id, ready: false, error: String(error) },
          serverParentOrigin,
        );
      }
    });
    return;
  }
  const host = nativeHost();
  if (!host?.event) return;
  await host.event.listen("studio-desktop-error", ({ payload }) => {
    const value = payload as { message?: unknown };
    if (typeof value.message !== "string") return;
    document.getElementById("studio-desktop-error")?.remove();
    const message = document.createElement("div");
    message.id = "studio-desktop-error";
    message.role = "alert";
    message.style.cssText =
      "position:fixed;inset:0 0 auto;z-index:2147483647;padding:16px;background:#7f1d1d;color:white";
    message.textContent = value.message;
    const dismiss = document.createElement("button");
    dismiss.textContent = "Dismiss";
    dismiss.onclick = () => message.remove();
    message.append(dismiss);
    document.body.append(message);
  });
  await host.event.listen("studio-close-request", async ({ payload }) => {
    const { id, action } = payload as {
      id: number;
      action?: "close" | "reload";
    };
    const release = freeze();
    document.getElementById("studio-close-error")?.remove();
    try {
      await prepareNativeClose();
      await host.core.invoke("close_response", { id, ready: true });
    } catch (error) {
      release();
      const message = document.createElement("div");
      message.id = "studio-close-error";
      message.role = "alert";
      message.style.cssText =
        "position:fixed;inset:0 0 auto;z-index:2147483647;padding:16px;background:#7f1d1d;color:white";
      message.textContent = `${action === "reload" ? "Reload was cancelled" : "The window is still open"} because changes may not be saved. ${String(error)}`;
      document.body.append(message);
      await host.core.invoke("close_response", { id, ready: false });
    }
  });
  await host.core.invoke("close_ready", {});
}
