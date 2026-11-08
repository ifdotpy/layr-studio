import "./browser-reset.css";
import "@mantine/core/styles.css";
import {
  ServerAreas,
  ServerAreasProvider,
} from "./app/ServerAreasProvider.tsx";
import {
  createServerViewServices,
  ServerServicesProvider,
} from "./servers/ServerServicesContext.tsx";
import { BottomInsetBand } from "./hooks/BottomInsetBand";
import { NativeServerCredentials } from "./native/serverCredentials";
import { hasNativeClient } from "./native/client";
import { isNativeFrame } from "./native/frame";
import {
  DesktopServerCredentials,
  nativeCredentialBridge,
} from "./servers/desktopCredentials";
import { BrowserServerCredentials } from "./servers/browserCredentials";
import { setServerCredentialAdapter } from "./servers/transport";
import { createRoot } from "react-dom/client";
import { MantineProvider } from "@mantine/core";
import App from "./App";
import { SidebarWarningsProvider } from "./components/SidebarWarnings";
import MultiServerApp from "./servers/MultiServerApp";
import { isServerView } from "./servers/environment";
import { isRemoteServerView } from "./servers/environment";
import { isClassicServerView } from "./servers/environment";
import { shellCredentialBridge } from "./servers/shellTransport";
import "./servers/servers.css";
import UIErrorBoundary from "./components/UIErrorBoundary";
import { theme } from "./theme";
import "./style.css";
import "./workspace-layout.css";
import "./appearance.css";
import "./studio-preferences.css";
import "./visual-activity.css";
import "./design/state.css";
import { initializeNativeClose } from "./native/close";

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
    createRoot(container).render(
      <UIErrorBoundary label="Studio" fullPage>
        <MantineProvider theme={theme} defaultColorScheme="auto">
          <BottomInsetBand>
            <SidebarWarningsProvider>
              {services ? (
                <ServerServicesProvider services={services}>
                  <ServerAreasProvider areas={services.areas}>
                    <App />
                  </ServerAreasProvider>
                </ServerServicesProvider>
              ) : (
                <MultiServerApp />
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
