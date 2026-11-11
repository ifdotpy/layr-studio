import "./rendererStyles.ts";

import { ServerAreas } from "./app/ServerAreasProvider.tsx";
import {
  createServerViewServices,
  ServerServicesProvider,
} from "./components/servers/ServerServicesProvider/ServerServicesProvider.tsx";
import { BottomInsetBand } from "./components/shell/BottomInsetBand/BottomInsetBand.tsx";
import { NativeServerCredentials } from "./native/serverCredentials.ts";
import { hasNativeClient } from "./native/client.ts";
import { isNativeFrame } from "./native/frame.ts";
import {
  DesktopServerCredentials,
  nativeCredentialBridge,
} from "./servers/net/desktopCredentials.ts";
import { BrowserServerCredentials } from "./servers/net/browserCredentials.ts";
import { setServerCredentialAdapter } from "./servers/net/transport.ts";
import { createRoot } from "react-dom/client";
import { MantineProvider } from "@mantine/core";
import App from "./components/shell/App/App.tsx";
import { SidebarWarningsProvider } from "./components/sidebar/SidebarWarnings/SidebarWarnings.tsx";
import MultiServerApp, {
  createShellState,
} from "./components/servers/MultiServerApp/MultiServerApp.tsx";
import { isServerView } from "./servers/environment.ts";
import { isRemoteServerView } from "./servers/environment.ts";
import { isClassicServerView } from "./servers/environment.ts";
import { shellCredentialBridge } from "./servers/shellTransport.ts";

import UIErrorBoundary from "./components/shell/UIErrorBoundary/UIErrorBoundary.tsx";
import { theme } from "./theme.ts";

import { initializeNativeClose } from "./native/close.ts";

async function mount() {
  await initializeNativeClose();
  const nativeCredentials = nativeCredentialBridge();
  setServerCredentialAdapter(
    nativeCredentials
      ? new DesktopServerCredentials(nativeCredentials)
      : hasNativeClient() && !isServerView
        ? new NativeServerCredentials()
        : isNativeFrame()
          ? new DesktopServerCredentials(shellCredentialBridge())
          : isRemoteServerView
            ? new BrowserServerCredentials()
            : isServerView && window.parent !== window
              ? new DesktopServerCredentials(shellCredentialBridge())
              : new BrowserServerCredentials(),
  );
  if (isServerView) document.documentElement.dataset.serverView = "true";
  if (isClassicServerView)
    document.documentElement.dataset.serverNavigation = "classic";

  const container = document.getElementById("root")!;
  if (container.dataset.studioMounted !== "true") {
    container.dataset.studioMounted = "true";
    const services = isServerView
      ? createServerViewServices((scope) => new ServerAreas(scope))
      : null;
    if (services) {
      services.areas.start();
      window.addEventListener("pagehide", (event) => {
        if (!event.persisted) services.close();
      });
    }
    const shell = services ? null : createShellState();
    createRoot(container).render(
      <UIErrorBoundary label="Studio" fullPage>
        <MantineProvider theme={theme} defaultColorScheme="auto">
          <BottomInsetBand>
            <SidebarWarningsProvider>
              {services ? (
                <ServerServicesProvider services={services}>
                  <App />
                </ServerServicesProvider>
              ) : (
                <MultiServerApp shell={shell!} />
              )}
            </SidebarWarningsProvider>
          </BottomInsetBand>
        </MantineProvider>
      </UIErrorBoundary>,
    );
  }
}
void mount().catch((error) => {
  const container = document.getElementById("root");
  if (container)
    container.textContent = `Studio could not start: ${String(error)}`;
});
