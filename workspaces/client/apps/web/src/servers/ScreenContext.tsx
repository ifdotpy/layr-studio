import { createContext, useContext } from "react";
import * as legacy from "./environment.ts";
import type { ScreenChannel, ScreenMessage } from "./screenChannel.ts";

export const ScreenContext = createContext<ScreenChannel | null>(null);
export const useScreenChannel = () => useContext(ScreenContext);

export function useScreenEnvironment() {
  const screen = useScreenChannel();
  const postToShell: (message: ScreenMessage, legacyOrigin?: string) => void =
    screen?.publish ?? postToLegacyShell;
  const owner = screen?.owner ?? legacy.serverViewId;
  return {
    screen,
    postToShell,
    hasShell: !!screen || window.parent !== window,
    serverViewId: owner,
    isServerView: !!owner,
    isClassicServerView: !screen && legacy.isClassicServerView,
    isRemoteServerView: !!owner && owner !== "local",
  };
}

function postToLegacyShell(message: ScreenMessage) {
  if (window.parent !== window)
    window.parent.postMessage(message, legacy.serverParentOrigin);
}
