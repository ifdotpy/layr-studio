import { TerminalState as GeneratedApiTerminalState } from "../../../generated/api.ts";
import { useServerAreas } from "../../../app/ServerAreasProvider.tsx";
import { useScreenEnvironment } from "../../../servers/ScreenContext.tsx";
import {
  useServerApi,
  useServerStore,
} from "../../servers/ServerServicesProvider/ServerServicesProvider.tsx";
import { Button, TextInput } from "@mantine/core";
import {
  ChevronDown,
  ChevronUp,
  Copy,
  Download,
  Plus,
  Search,
  Square,
  Terminal as TerminalIcon,
} from "lucide-react";
import { useEffect, useMemo, useRef, useState, useLayoutEffect } from "react";
import { running, createShellTerminalStore } from "./ShellTerminalStore.ts";
import { errorText } from "../../../api.ts";
import { observer } from "mobx-react-lite";
import { resourceData, type ResourceSlot } from "../../../store/runtime.ts";
import {
  terminalId,
  type TerminalOutput,
  type TerminalRecord,
} from "../../../generated/observableResources.ts";
import { createArea } from "../../../app/area.ts";
import { copyText } from "../../../clipboard/clipboard.ts";

import { layoutValues } from "../../../layout/layout-contract.generated.ts";
import { resolveLayoutLength } from "../../../layout/deriveWorkspaceLayout.ts";
import { useTerminalHeight } from "./useTerminalHeight.ts";
import { TerminalResizeGesture } from "./terminalResize.ts";

const terminalBounds = (rootFont: number) => ({
  preferred: resolveLayoutLength("terminal.preferred", rootFont),
  min: resolveLayoutLength("terminal.min", rootFont),
  closedBar: resolveLayoutLength("terminal.closedBar", rootFont),
  viewportMaxRatio: layoutValues.terminal.viewportMaxRatio,
});

export const [TerminalActionsProvider, useTerminalActions] = createArea<
  Pick<ReturnType<typeof useServerApi>, "get" | "post" | "syncPost"> & {
    notify(message: string): void;
  }
>("Terminal");
// In the combined window the shell draws the closed bar under its sidebar and makes room
// for the open panel across the whole window (docs: servers.css "Terminal under the sidebar").

export default observer(function TerminalDock() {
  const areas = useServerAreas();
  const ownerUi = areas.ui;
  const agent = areas.navigation.selectedAgent || areas.navigation.selectedLead;
  const notify = areas.notices.notify;
  const { hasShell, isServerView, isClassicServerView, serverViewId } =
    useScreenEnvironment();
  const shellTerminal = isServerView && !isClassicServerView && hasShell;
  const store = useServerStore();
  const terminalList = store.terminals().current;
  const shells = resourceData(terminalList) ?? [];
  const api = useServerApi();
  const actions = useMemo(() => ({ ...api, notify }), [api, notify]);

  const ui = ownerUi.terminal;
  const { opened, height, selected, creating } = ui;
  const dock = useRef<HTMLElement>(null);
  const resize = useRef(new TerminalResizeGesture());
  const [dragHeight, setDragHeight] = useState<number | null>(null);
  const geometry = useTerminalHeight(
    dock,
    dragHeight ?? height,
    opened,
    terminalBounds,
  );
  const scope = serverViewId ?? "local";
  const cancelResize = () => {
    resize.current.cancel();
    setDragHeight(null);
  };
  useEffect(() => {
    cancelResize();
    return () => resize.current.cancel();
  }, [scope, opened]);
  const commitHeight = (value: number) => {
    ui.resize(Math.min(geometry.max, Math.max(geometry.min, value)));
  };
  const { scrollTop } = ui;
  const setScrollTop = ui.scroll;
  const error = "error" in terminalList ? errorText(terminalList.error) : "";
  const list = useRef<HTMLDivElement>(null);
  // Page layout rules read this flag instead of #root:has(.terminal-dock).
  useLayoutEffect(() => {
    ui.measure(geometry.height);
    const root = document.documentElement;
    root.dataset.terminalDock = opened ? "open" : "closed";
    if (shellTerminal) root.dataset.terminalShell = "true";
    root.style.setProperty("--terminal-dock-height", `${geometry.height}px`);
    return () => {
      delete root.dataset.terminalDock;
      delete root.dataset.terminalShell;
      root.style.removeProperty("--terminal-dock-height");
    };
  }, [ui, opened, shellTerminal, geometry.height]);
  useLayoutEffect(() => {
    if (list.current && scrollTop === 0) list.current.scrollTop = 0;
  }, [scrollTop, selected]);
  useEffect(() => {
    const key = (event: KeyboardEvent) => {
      if ((event.ctrlKey || event.metaKey) && event.code === "Backquote") {
        event.preventDefault();
        ui.toggle();
      }
    };
    window.addEventListener("keydown", key);
    return () => window.removeEventListener("keydown", key);
  }, [ui]);
  const entries = shells.filter(
    (shell) => shell.status !== GeneratedApiTerminalState.closed,
  );
  const current = entries.find((entry) => `shell:${entry.id}` === selected);
  const owner = (id: string) =>
    resourceData(store.state().current)?.agents.get(id)?.name || "Agent";
  const filtered = entries.filter((entry) =>
    `${entry.title} ${entry.cwd || ""} ${owner(entry.agent)} ${entry.status}`
      .toLowerCase()
      .includes(ui.query.toLowerCase()),
  );
  const rowHeight = 58;
  const start = Math.max(
    0,
    Math.min(
      Math.max(0, filtered.length - 1),
      Math.floor(scrollTop / rowHeight) - 3,
    ),
  );
  const visible = filtered.slice(
    start,
    start + Math.ceil(geometry.height / rowHeight) + 6,
  );
  const create = ownerUi.createTerminal;
  const activeCount = entries.filter((entry) => running(entry.status)).length;
  const heightStep = resolveLayoutLength(
    "resize.step",
    Number.parseFloat(getComputedStyle(document.documentElement).fontSize),
  );
  return (
    <TerminalActionsProvider value={actions}>
      <section
        ref={dock}
        className={`terminal-dock ${opened ? "is-open" : ""}`}
        data-shell-terminal={shellTerminal || undefined}
        aria-label="Terminals"
        style={{ height: geometry.height }}
      >
        {opened && (
          <div
            className="terminal-dock-resizer"
            role="separator"
            aria-label="Terminal panel height"
            aria-orientation="horizontal"
            tabIndex={0}
            aria-valuemin={geometry.min}
            aria-valuemax={geometry.max}
            aria-valuenow={geometry.height}
            onKeyDown={(event) => {
              if (event.key === "Escape") cancelResize();
              if (event.key === "ArrowUp" || event.key === "ArrowDown") {
                event.preventDefault();
                cancelResize();
                commitHeight(
                  geometry.height +
                    (event.key === "ArrowUp" ? heightStep : -heightStep),
                );
              }
            }}
            onPointerDown={(event) => {
              if (event.button !== 0) return;
              event.preventDefault();
              resize.current.start(
                scope,
                event.pointerId,
                event.clientY,
                geometry.height,
              );
              event.currentTarget.setPointerCapture(event.pointerId);
            }}
            onPointerMove={(event) => {
              const next = resize.current.move(
                scope,
                event.pointerId,
                event.clientY,
                geometry.min,
                geometry.max,
              );
              if (next !== null) setDragHeight(next);
            }}
            onPointerUp={(event) => {
              const next = resize.current.finish(scope, event.pointerId);
              if (next !== null) commitHeight(next);
              setDragHeight(null);
              if (event.currentTarget.hasPointerCapture(event.pointerId))
                event.currentTarget.releasePointerCapture(event.pointerId);
            }}
            onPointerCancel={cancelResize}
            onLostPointerCapture={cancelResize}
          />
        )}
        <header className="terminal-dock-bar">
          <button
            type="button"
            className="terminal-dock-toggle"
            onClick={() => ui.open(!opened)}
            aria-expanded={opened}
            aria-label={opened ? "Hide terminals" : "Show terminals"}
          >
            <TerminalIcon size={15} />
            <strong>Terminals</strong>
            {entries.length > 0 && (
              <span className="terminal-dock-count">{entries.length}</span>
            )}
            {activeCount > 0 && (
              <span className="terminal-dock-running">
                {activeCount} active
              </span>
            )}
            {opened ? <ChevronDown size={14} /> : <ChevronUp size={14} />}
          </button>
          {!opened && current && (
            <span className="terminal-dock-current">{current.title}</span>
          )}
          <span className="terminal-dock-spacer" />
          {opened && (
            <Button
              size="compact-xs"
              variant="subtle"
              onClick={() => ui.open(false)}
            >
              Hide panel
            </Button>
          )}
          <Button
            variant="subtle"
            size="compact-xs"
            aria-label="New terminal"
            title="New terminal"
            leftSection={<Plus size={13} />}
            disabled={!agent}
            loading={creating}
            onClick={() => void create()}
          >
            <span className="terminal-new-label">New terminal</span>
          </Button>
        </header>
        {opened && (
          <div className="terminal-dock-body">
            <aside
              className="terminal-dock-sessions"
              aria-label="Terminal sessions"
            >
              <TextInput
                aria-label="Find terminal"
                placeholder="Find a terminal"
                leftSection={<Search size={13} />}
                size="xs"
                value={ui.query}
                onChange={(event) => {
                  ui.search(event.currentTarget.value);
                  setScrollTop(0);
                  if (list.current) list.current.scrollTop = 0;
                }}
              />
              <div
                className="terminal-dock-list"
                ref={list}
                onScroll={(event) =>
                  setScrollTop(event.currentTarget.scrollTop)
                }
              >
                <div
                  style={{
                    height: filtered.length * rowHeight,
                    position: "relative",
                  }}
                >
                  {visible.map((entry, index) => (
                    <button
                      type="button"
                      className={`terminal-session ${current?.id === entry.id ? "is-selected" : ""}`}
                      key={entry.id}
                      onClick={() => ui.choose(`shell:${entry.id}`)}
                      aria-pressed={current?.id === entry.id}
                      aria-label={`${entry.title} Your terminal · ${owner(entry.agent)}`}
                      style={{
                        position: "absolute",
                        top: (start + index) * rowHeight,
                        height: rowHeight,
                        width: "100%",
                      }}
                      title={`${entry.title}\n${entry.cwd || ""}`}
                    >
                      <span
                        className={`terminal-session-dot ${running(entry.status) ? "is-running" : ""}`}
                      />
                      <span className="terminal-session-description">
                        <strong>{entry.title}</strong>
                        <small>
                          {owner(entry.agent)} · {entry.status}
                        </small>
                      </span>
                    </button>
                  ))}
                </div>
                {!filtered.length && (
                  <p className="terminal-dock-empty">
                    {ui.query
                      ? "No matching sessions"
                      : "Your terminals appear here."}
                  </p>
                )}
              </div>
            </aside>
            <div className="terminal-dock-detail">
              {error && (
                <p className="terminal-dock-error" role="status">
                  {error}
                </p>
              )}
              {current ? (
                <>
                  <div className="terminal-detail-heading">
                    {ui.renameTitle !== null ? (
                      <form
                        onSubmit={(event) => {
                          event.preventDefault();
                          void ui.action("/api/terminals/rename", {
                            id: current.id,
                            title: ui.renameTitle ?? "",
                          });
                        }}
                      >
                        <TextInput
                          aria-label="Terminal name"
                          value={ui.renameTitle}
                          autoFocus
                          maxLength={120}
                          onChange={(event) =>
                            ui.rename(event.currentTarget.value)
                          }
                          onKeyDown={(event) => {
                            if (event.key === "Escape") ui.rename(null);
                          }}
                        />
                        <Button
                          size="compact-xs"
                          type="submit"
                          disabled={ui.pending || !ui.renameTitle.trim()}
                        >
                          Save
                        </Button>
                      </form>
                    ) : (
                      <button
                        className="terminal-detail-title"
                        title="Rename terminal"
                        onClick={() => ui.rename(current.title)}
                      >
                        {current.title}
                      </button>
                    )}
                    <span className="terminal-detail-status">
                      {current.status}
                      {current.exitCode != null
                        ? ` · Exit ${current.exitCode}`
                        : ""}
                    </span>
                    <span className="terminal-dock-spacer" />
                    <Button
                      size="compact-xs"
                      color="red"
                      variant="subtle"
                      disabled={ui.pending}
                      onClick={() => {
                        if (running(current.status)) ui.confirmEnd(current.id);
                        else
                          void ui.action("/api/terminals/close", {
                            id: current.id,
                          });
                      }}
                    >
                      End session
                    </Button>
                  </div>
                  {ui.endTarget === current.id && (
                    <div
                      className="terminal-end-confirm"
                      role="alertdialog"
                      aria-label="End terminal session"
                    >
                      <p>
                        End this session? This stops the shell and its active
                        processes.
                      </p>
                      <Button
                        size="compact-xs"
                        disabled={ui.pending}
                        onClick={() => ui.confirmEnd(null)}
                      >
                        Keep session
                      </Button>
                      <Button
                        variant="filled"
                        size="compact-xs"
                        color="red"
                        loading={ui.pending}
                        onClick={() => {
                          const id = current.id;
                          ui.confirmEnd(null);
                          void ui.action("/api/terminals/close", { id });
                        }}
                      >
                        End session and stop processes
                      </Button>
                    </div>
                  )}
                  <div className="terminal-detail-path" title={current.cwd}>
                    {current.cwd || owner(current.agent)}
                  </div>
                  <ShellView
                    key={current.id}
                    shell={current}
                    outputSlot={store.terminalOutput(terminalId(current.id))}
                  />
                </>
              ) : (
                <div className="terminal-dock-welcome">
                  <TerminalIcon size={26} />
                  <strong>Your terminals, always here</strong>
                  <p>
                    Start a shell in{" "}
                    {agent?.cwd || "the selected agent’s folder"}.
                  </p>
                  <Button
                    size="xs"
                    leftSection={<Plus size={14} />}
                    disabled={!agent}
                    loading={creating}
                    onClick={() => void create()}
                  >
                    Open terminal
                  </Button>
                  <small>⌘ / Ctrl + ` to show or hide</small>
                </div>
              )}
            </div>
          </div>
        )}
      </section>
    </TerminalActionsProvider>
  );
});

export const ShellView = observer(function ShellView({
  shell,
  outputSlot,
}: {
  shell: TerminalRecord;
  outputSlot: ResourceSlot<TerminalOutput>;
}) {
  const actions = useTerminalActions();
  const [state] = useState(() =>
    createShellTerminalStore(shell, outputSlot, actions),
  );
  return (
    <div className="terminal-shell-view">
      {state.truncated && (
        <div className="terminal-output-notice">
          Earlier output exceeded the retained buffer. The latest output is
          shown.
        </div>
      )}
      {(state.failure || state.inputFailure) && (
        <div className="terminal-dock-error" role="status">
          {state.inputFailure || state.failure}{" "}
          <button onClick={() => void state.reconnect()}>Reconnect</button>
        </div>
      )}
      {state.inputWarning && (
        <p className="terminal-output-notice" role="status">
          {state.inputWarning}
        </p>
      )}
      <div
        ref={state.mount}
        className="terminal-xterm"
        aria-label="Interactive terminal"
      />
      {state.downloadStatus && (
        <p className="terminal-output-notice" role="status">
          {state.downloadStatus}
        </p>
      )}
      <footer className="terminal-output-actions">
        <span>
          {running(state.status) ? "Direct shell input" : "Session ended"}
        </span>
        <span className="terminal-dock-spacer" />
        <Button
          size="compact-xs"
          variant="subtle"
          leftSection={<Download size={12} />}
          onClick={() => {
            if (state.downloading) state.cancelDownload();
            else void state.download();
          }}
        >
          {state.downloading ? "Cancel download" : "Download saved output"}
        </Button>
        <Button
          size="compact-xs"
          variant="subtle"
          leftSection={<Copy size={12} />}
          onClick={() => {
            const text = state.terminal?.getSelection();
            if (!text) return actions.notify("Select terminal text first.");
            void copyText(text).catch((e) => actions.notify(errorText(e)));
          }}
        >
          Copy selection
        </Button>
        <Button
          size="compact-xs"
          variant="subtle"
          disabled={!running(state.status) || !!state.inputFailure}
          leftSection={<Square size={11} />}
          onClick={() => state.send("\u0003")}
        >
          Ctrl+C
        </Button>
      </footer>
    </div>
  );
});

export { TerminalDockStore } from "./TerminalDockStore.ts";

export { running } from "./ShellTerminalStore.ts";
