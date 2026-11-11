import { ActionIcon, Button, useMantineColorScheme } from "@mantine/core";
import {
  ChevronUp,
  Plus,
  Settings,
  Terminal as TerminalIcon,
} from "lucide-react";
import { observer } from "mobx-react-lite";
import { useEffect, useRef, type CSSProperties } from "react";
import { layoutQueries } from "../../../layout/layout-contract.generated.ts";
import { ScreenContext } from "../../../servers/ScreenContext.tsx";
import {
  ServerManagementPanelContext,
  ServerSettingsContext,
} from "../../../servers/ServerSettingsContext.tsx";
import App from "../../shell/App/App.tsx";
import { ServerAreasProvider } from "../../shell/ServerAreasProvider/ServerAreasProvider.tsx";
import Sidebar from "../../sidebar/Sidebar/Sidebar.tsx";
import { ServerServicesProvider } from "../ServerServicesProvider/ServerServicesProvider.tsx";
import { ShellNewChat } from "../ShellNewChat/ShellNewChat.tsx";
import { ShellSearch } from "../ShellSearch/ShellSearch.tsx";
import { ShellSettings } from "../ShellSettings/ShellSettings.tsx";
import { ShellManagement } from "../ShellManagement/ShellManagement.tsx";

import {
  fontFamilies,
  parseSidebarShortcut,
} from "../../../studioPreferences.ts";

import { ShellState } from "../../../app/shell/ShellState.ts";
export { createShellState, ShellState } from "../../../app/shell/ShellState.ts";
export default function MultiServerApp({ shell }: { shell: ShellState }) {
  return (
    <ServerServicesProvider services={shell.localServices}>
      <MultiServerShell shell={shell} />
    </ServerServicesProvider>
  );
}
const MultiServerShell = observer(function MultiServerShell({
  shell,
}: {
  shell: ShellState;
}) {
  const localServices = shell.localServices;
  const shellRef = useRef<HTMLDivElement>(null);
  const { setColorScheme } = useMantineColorScheme();
  const services = shell.serviceInstances;
  const directScreen = shell.directScreen;
  const combinedSidebar = shell.combinedSidebar;
  const mergedSidebar = shell.sidebar.model;
  useEffect(() => {
    const key = (event: KeyboardEvent) => {
      if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === "k") {
        event.preventDefault();
        shell.set({ searchOpen: true });
      }
      const shortcut = parseSidebarShortcut(
        shell.sidebar.value.sidebarShortcut,
      );
      if (
        shortcut &&
        event.key.toLowerCase() === shortcut.key &&
        event.metaKey === shortcut.meta &&
        event.ctrlKey === shortcut.ctrl &&
        event.altKey === shortcut.alt &&
        event.shiftKey === shortcut.shift &&
        !(event.target as HTMLElement)?.closest?.(
          "input,textarea,select,[contenteditable]",
        )
      ) {
        event.preventDefault();
        shell.toggleSidebar();
      }
    };
    window.addEventListener("keydown", key);
    return () => {
      window.removeEventListener("keydown", key);
    };
  }, [shell.sidebar.value.sidebarShortcut, shell.toggleSidebar]);
  useEffect(() => {
    setColorScheme(shell.sidebar.value.theme);
    document.documentElement.style.setProperty(
      "--studio-font-family",
      fontFamilies[shell.sidebar.value.fontFamily].css,
    );
    document.documentElement.style.setProperty(
      "--studio-sidebar-font-size",
      shell.sidebar.value.sidebarFontSize + "px",
    );
  }, [shell.sidebar.value, setColorScheme]);

  useEffect(() => {
    const narrow = window.matchMedia(layoutQueries.navigationCompact);
    let observed: Element | null = null;
    // A resize reports the new width; reading it again would force layout.
    const measure = (reported?: number) => {
      const sidebar = document.querySelector(".multi-server-shell > #sidebar");
      if (sidebar !== observed) {
        if (observed) observer.unobserve(observed);
        if (sidebar) observer.observe(sidebar);
        observed = sidebar;
      }
      const width =
        !shell.sidebarVisible || narrow.matches || !sidebar
          ? 0
          : Math.round(reported ?? sidebar.getBoundingClientRect().width);
      shellRef.current?.style.setProperty(
        "--shell-sidebar-width",
        `${width}px`,
      );
      shell.set({ shellSidebar: width > 0 });
      const root = document.documentElement;
      if (width > 0 && root.dataset.shellSidebar !== "true")
        root.dataset.shellSidebar = "true";
      else if (width === 0 && "shellSidebar" in root.dataset)
        delete root.dataset.shellSidebar;
    };
    const observer = new ResizeObserver((entries) =>
      measure(entries.at(-1)?.borderBoxSize[0]?.inlineSize),
    );
    const remeasure = () => measure();
    measure();
    narrow.addEventListener("change", remeasure);
    return () => {
      observer.disconnect();
      narrow.removeEventListener("change", remeasure);
    };
  }, [
    !shell.sidebarVisible,
    shell.sidebar.compact,
    mergedSidebar?.sources.length,
    shell.servers.length,
    combinedSidebar.list.frame.aside,
  ]);
  if (directScreen)
    return (
      <ServerSettingsContext.Provider value={shell}>
        <ServerManagementPanelContext.Provider
          value={<ShellManagement shell={shell} />}
        >
          <ServerAreasProvider areas={localServices!.areas}>
            <App />
          </ServerAreasProvider>
        </ServerManagementPanelContext.Provider>
      </ServerSettingsContext.Provider>
    );
  const terminal = shell.terminal;
  const terminalLayout =
    terminal && shell.shellSidebar
      ? terminal.opened
        ? "open"
        : "closed"
      : undefined;
  return (
    <div
      ref={shellRef}
      className={`multi-server-shell ${!shell.sidebarVisible ? "server-sidebar-hidden" : ""}`}
      data-navigation="combined"
      data-navigation-compact={shell.sidebar.compact}
      data-shell-terminal={terminalLayout}
      style={
        {
          "--shell-terminal-height": `${terminal?.height || 0}px`,
        } as CSSProperties
      }
    >
      <>
        <Button
          className="server-mobile-toggle"
          onClick={() => shell.sidebar.setMobileOpen(!shell.sidebar.mobileOpen)}
          aria-expanded={shell.sidebar.mobileOpen}
        >
          Servers and chats
        </Button>
        {mergedSidebar?.sources.length ? (
          <Sidebar owner={combinedSidebar.list} notice={shell.failure} />
        ) : (
          <aside className="server-sidebar" aria-label="Servers and projects">
            <header>
              <strong>LAYR Studio</strong>
              <ActionIcon
                aria-label="Studio settings"
                title="Studio settings"
                onClick={shell.openManager}
              >
                <Settings size={18} />
              </ActionIcon>
            </header>
            {shell.servers.length ? (
              <>
                <p role="status" className="sr-only">
                  Loading chats...
                </p>
                {/* Placeholder rows hold the sidebar's shape while chats load. */}
                <div className="server-sidebar-skeleton" aria-hidden="true">
                  {[62, 48, 74, 40, 56].map((width) => (
                    <span key={width} style={{ width: `${width}%` }} />
                  ))}
                </div>
              </>
            ) : (
              <p role="status">Pair a server to open its projects and chats.</p>
            )}
            {shell.failure && <p role="alert">{shell.failure}</p>}
          </aside>
        )}
      </>
      {terminalLayout === "closed" && terminal && (
        <div className="shell-terminal-bar">
          <button
            type="button"
            className="shell-terminal-toggle"
            aria-expanded={false}
            aria-label="Show terminals"
            onClick={() =>
              shell.send(shell.current, {
                action: "terminal",
                operation: "toggle",
              })
            }
          >
            <TerminalIcon size={15} />
            <strong>Terminals</strong>
            {terminal.count > 0 && (
              <span className="terminal-dock-count">{terminal.count}</span>
            )}
            <ChevronUp size={14} />
          </button>
          <ActionIcon
            variant="filled"
            size={28}
            aria-label="New terminal"
            title="New terminal"
            disabled={!terminal.canCreate}
            onClick={() =>
              shell.send(shell.current, {
                action: "terminal",
                operation: "new",
              })
            }
          >
            <Plus size={13} />
          </ActionIcon>
        </div>
      )}
      <main className="server-views">
        {!shell.servers.length && (
          <div className="startup">
            <p>No server is paired.</p>
            <Button onClick={shell.openManager}>Open Settings</Button>
          </div>
        )}
        {shell.servers.map((server) => {
          const service = services.get(server.id)!;
          const channel = shell.screens.get(server.id);
          if (!service || !channel) return null;
          return (
            <ServerServicesProvider
              key={JSON.stringify([
                server.id,
                service.owner.origin,
                service.owner.credentialId,
                shell.revivedSeen[server.id] || 0,
              ])}
              services={service}
            >
              <ScreenContext.Provider value={channel}>
                <ServerAreasProvider areas={service.areas}>
                  <App />
                </ServerAreasProvider>
              </ScreenContext.Provider>
            </ServerServicesProvider>
          );
        })}
      </main>
      <ShellSettings shell={shell} />
      <ShellNewChat shell={shell} />
      <ShellSearch shell={shell} />
    </div>
  );
});
