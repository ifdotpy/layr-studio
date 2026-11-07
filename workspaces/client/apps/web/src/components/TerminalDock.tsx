import { TerminalState as GeneratedApiTerminalState } from "../generated/api.ts";
import { randomUUID } from "../uuid";
import { useScreenEnvironment } from "../servers/ScreenContext";
import { useServerApi, useServerStore } from "../servers/ServerServicesContext";
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
import { useEffect, useRef, useState, useLayoutEffect } from "react";
import type { CSSProperties } from "react";
import type { Terminal } from "@xterm/xterm";
import { errorText, type GetResult, type PostBody } from "../api";
import { observer } from "mobx-react-lite";
import { resourceData, type ResourceSlot } from "../store/runtime.ts";
import {
  terminalId,
  type TerminalOutput,
  type TerminalRecord,
} from "../generated/observableResources.ts";
import { createArea } from "./area.ts";
import "./terminal-dock.css";
import { copyText } from "../clipboard/clipboard";
import { serverParentOrigin } from "../servers/environment";

import { layoutValues } from "../layout/layout-contract.generated";
import { resolveLayoutLength } from "../layout/deriveWorkspaceLayout";
import { useTerminalHeight } from "./useTerminalHeight";
import { TerminalResizeGesture } from "./terminalResize";

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
const running = (status: string) =>
  ["running", "starting", "approval"].includes(status);
// In the combined window the shell draws the closed bar under its sidebar and makes room
// for the open panel across the whole window (docs: servers.css "Terminal under the sidebar").
const inputUncertaintyNotice =
  "Previous input delivery is uncertain. Check the terminal output before you enter it again. Buffered input was not sent.";

/** xterm paints on a canvas, so it reads the CSS owner in studio-theme.css. */
function terminalTheme() {
  const style = getComputedStyle(document.documentElement);
  const token = (name: string) => style.getPropertyValue(name).trim();
  return {
    background: token("--studio-terminal"),
    foreground: token("--studio-terminal-text"),
    cursor: token("--studio-terminal-text"),
    cursorAccent: token("--studio-terminal"),
    selectionBackground: token("--studio-terminal-selection"),
  };
}

export default observer(function TerminalDock({
  agentId,
  notify,
}: {
  agentId?: string | undefined;
  notify: (message: string) => void;
}) {
  const {
    screen,
    postToShell,
    hasShell,
    isServerView,
    isClassicServerView,
    serverViewId,
  } = useScreenEnvironment();
  const shellTerminal = isServerView && !isClassicServerView && hasShell;
  const store = useServerStore();
  const agents = resourceData(store.state().current)?.agents;
  const agent = agentId ? agents?.get(agentId) : undefined;
  const terminalList = store.terminals().current;
  const shells = resourceData(terminalList) ?? [];
  const api = useServerApi();
  const { post, save, saved } = api;

  const [opened, setOpened] = useState(() =>
    saved("codex.terminal.open", false),
  );
  const [height, setHeight] = useState(() =>
    saved(
      "codex.terminal.height",
      terminalBounds(
        Number.parseFloat(getComputedStyle(document.documentElement).fontSize),
      ).preferred,
    ),
  );
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
  const commitHeight = (next: number) => {
    setHeight(next);
    save("codex.terminal.height", next);
  };
  const [selected, setSelected] = useState(() =>
    saved("codex.terminal.selected", ""),
  );
  const [query, setQuery] = useState("");
  const [scrollTop, setScrollTop] = useState(0);
  const error = "error" in terminalList ? errorText(terminalList.error) : "";
  const [creating, setCreating] = useState(false);
  const [renaming, setRenaming] = useState(false);
  const [title, setTitle] = useState("");
  const [pending, setPending] = useState(false);
  const [endTarget, setEndTarget] = useState<string | null>(null);
  const list = useRef<HTMLDivElement>(null);
  const notifyRef = useRef(notify);
  notifyRef.current = notify;
  // Page layout rules read this flag instead of #root:has(.terminal-dock).
  useLayoutEffect(() => {
    const root = document.documentElement;
    root.dataset.terminalDock = opened ? "open" : "closed";
    if (shellTerminal) root.dataset.terminalShell = "true";
    return () => {
      delete root.dataset.terminalDock;
      delete root.dataset.terminalShell;
    };
  }, [opened]);
  useEffect(() => {
    if (!opened) setEndTarget(null);
    save("codex.terminal.open", opened);
  }, [opened]);
  useLayoutEffect(() => {
    document.documentElement.style.setProperty(
      "--terminal-dock-height",
      `${geometry.height}px`,
    );
    return () => {
      document.documentElement.style.removeProperty("--terminal-dock-height");
    };
  }, [geometry.height]);
  useEffect(() => {
    const key = (event: KeyboardEvent) => {
      if ((event.ctrlKey || event.metaKey) && event.code === "Backquote") {
        event.preventDefault();
        setOpened((value: boolean) => !value);
      }
    };
    window.addEventListener("keydown", key);
    return () => window.removeEventListener("keydown", key);
  }, []);
  useEffect(() => {
    save("codex.terminal.selected", selected);
    setRenaming(false);
    setEndTarget(null);
  }, [selected]);
  const entries = shells.filter(
    (shell) => shell.status !== GeneratedApiTerminalState.closed,
  );
  const current = entries.find((entry) => `shell:${entry.id}` === selected);
  const owner = (id: string) => agents?.get(id)?.name || "Agent";
  const filtered = entries.filter((entry) =>
    `${entry.title} ${entry.cwd || ""} ${owner(entry.agent)} ${entry.status}`
      .toLowerCase()
      .includes(query.toLowerCase()),
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
  const create = async () => {
    if (!agent || creating) return;
    setCreating(true);
    setOpened(true);
    try {
      const shell = await post("/api/terminals/create", {
        id: randomUUID(),
        agent: agent.id,
        cols: 100,
        rows: 24,
      });
      setSelected(`shell:${shell.id}`);
      setQuery("");
      setScrollTop(0);
      if (list.current) list.current.scrollTop = 0;
    } catch (e) {
      notifyRef.current(errorText(e));
    } finally {
      setCreating(false);
    }
  };
  const createRef = useRef(create);
  createRef.current = create;
  const activeCount = entries.filter((entry) => running(entry.status)).length;
  const shownHeight = geometry.height;
  const heightStep = resolveLayoutLength(
    "resize.step",
    Number.parseFloat(getComputedStyle(document.documentElement).fontSize),
  );
  useEffect(() => {
    if (!shellTerminal) return;
    postToShell(
      {
        kind: "studio-server-terminal",
        serverId: serverViewId,
        opened,
        height: shownHeight,
        count: entries.length,
        active: activeCount,
        canCreate: !!agent,
      },
      serverParentOrigin,
    );
  }, [
    opened,
    shownHeight,
    entries.length,
    activeCount,
    !!agent,
    shellTerminal,
    postToShell,
    serverViewId,
  ]);
  useEffect(() => {
    if (screen)
      return screen.bindTerminal((operation) => {
        if (operation === "toggle") setOpened((value) => !value);
        else void createRef.current();
      });
    const command = (event: Event) => {
      const operation = (event as CustomEvent<string>).detail;
      if (operation === "toggle") setOpened((value: boolean) => !value);
      else if (operation === "new") void createRef.current();
    };
    window.addEventListener("studio-terminal-command", command);
    return () => window.removeEventListener("studio-terminal-command", command);
  }, [screen, shellTerminal]);
  const action = async <
    Path extends "/api/terminals/rename" | "/api/terminals/close",
  >(
    path: Path,
    body: PostBody<Path>,
  ) => {
    setPending(true);
    try {
      await post(path, body);
      return true;
    } catch (e) {
      notifyRef.current(errorText(e));
      return false;
    } finally {
      setPending(false);
    }
  };
  return (
    <TerminalActionsProvider value={{ ...api, notify }}>
      <section
        ref={dock}
        className={`terminal-dock ${opened ? "is-open" : ""}`}
        data-shell-terminal={shellTerminal || undefined}
        aria-label="Terminals"
        style={
          {
            height: geometry.height,
            "--terminal-dock-height": `${geometry.height}px`,
          } as CSSProperties
        }
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
                  Math.min(
                    geometry.max,
                    Math.max(
                      geometry.min,
                      geometry.height +
                        (event.key === "ArrowUp" ? heightStep : -heightStep),
                    ),
                  ),
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
              if (next !== null)
                commitHeight(
                  Math.min(geometry.max, Math.max(geometry.min, next)),
                );
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
            onClick={() => setOpened(!opened)}
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
              onClick={() => setOpened(false)}
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
                value={query}
                onChange={(event) => {
                  setQuery(event.currentTarget.value);
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
                      onClick={() => setSelected(`shell:${entry.id}`)}
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
                    {query
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
                    {renaming ? (
                      <form
                        onSubmit={(event) => {
                          event.preventDefault();
                          void action("/api/terminals/rename", {
                            id: current.id,
                            title,
                          }).then((ok) => {
                            if (ok) setRenaming(false);
                          });
                        }}
                      >
                        <TextInput
                          aria-label="Terminal name"
                          value={title}
                          autoFocus
                          maxLength={120}
                          onChange={(event) =>
                            setTitle(event.currentTarget.value)
                          }
                          onKeyDown={(event) => {
                            if (event.key === "Escape") setRenaming(false);
                          }}
                        />
                        <Button
                          size="compact-xs"
                          type="submit"
                          disabled={pending || !title.trim()}
                        >
                          Save
                        </Button>
                      </form>
                    ) : (
                      <button
                        className="terminal-detail-title"
                        title="Rename terminal"
                        onClick={() => {
                          setTitle(current.title);
                          setRenaming(true);
                        }}
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
                      disabled={pending}
                      onClick={() => {
                        if (running(current.status)) setEndTarget(current.id);
                        else
                          void action("/api/terminals/close", {
                            id: current.id,
                          });
                      }}
                    >
                      End session
                    </Button>
                  </div>
                  {endTarget === current.id && (
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
                        disabled={pending}
                        onClick={() => setEndTarget(null)}
                      >
                        Keep session
                      </Button>
                      <Button
                        variant="filled"
                        size="compact-xs"
                        color="red"
                        loading={pending}
                        onClick={() => {
                          const id = current.id;
                          setEndTarget(null);
                          void action("/api/terminals/close", { id });
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
  const output = outputSlot.current;
  const data = resourceData(output);
  const outputError = "error" in output ? output.error : undefined;
  const [manualOutput, setManualOutput] =
    useState<GetResult<"/api/terminals/output">>();
  useEffect(
    () => setManualOutput(undefined),
    [data?.text, data?.offset, data?.status],
  );
  const outputRef = useRef(data);
  outputRef.current = manualOutput ?? data;
  const offsetRef = useRef(0);
  const applyOutputRef = useRef<() => void>(() => {});
  const { get, post, syncPost, notify } = useTerminalActions();

  const host = useRef<HTMLDivElement>(null);
  const terminal = useRef<Terminal | null>(null);
  const [failure, setFailure] = useState("");
  const [inputFailure, setInputFailure] = useState("");
  const [inputWarning, setInputWarning] = useState("");
  const inputBlocked = useRef(false);
  const inputGeneration = useRef(0);
  const inputUncertain = useRef(false);
  const pendingInput = useRef<object | null>(null);
  const [truncated, setTruncated] = useState(false);
  const status = (manualOutput ?? data)?.status ?? shell.status;
  const [retry, setRetry] = useState(0);
  const inputChain = useRef(Promise.resolve());
  const downloadAttempt = useRef<symbol | null>(null);
  const [downloading, setDownloading] = useState(false);
  const [downloadStatus, setDownloadStatus] = useState("");
  const fenceInput = () => {
    if (pendingInput.current) inputUncertain.current = true;
    inputGeneration.current++;
    pendingInput.current = null;
    inputChain.current = Promise.resolve();
    inputBlocked.current = true;
    if (terminal.current) terminal.current.options.disableStdin = true;
  };
  useEffect(
    () => () => {
      downloadAttempt.current = null;
    },
    [],
  );
  const download = async () => {
    if (downloadAttempt.current) return;
    const attempt = Symbol();
    downloadAttempt.current = attempt;
    setDownloading(true);
    setDownloadStatus("Preparing saved output…");
    try {
      const parts: string[] = [];
      let offset = 0;
      let end: number | undefined;
      let incomplete = false;
      for (let page = 0; page < 512; page++) {
        const value = await get("/api/terminals/output", {
          query: {
            id: shell.id,
            history: "1",
            offset,
            limit: Math.min(65536, end === undefined ? 65536 : end - offset),
          },
        });
        if (downloadAttempt.current !== attempt) return;
        if (
          !value ||
          typeof value.offset !== "number" ||
          !Number.isSafeInteger(value.offset) ||
          typeof value.availableOffset !== "number" ||
          !Number.isSafeInteger(value.availableOffset) ||
          typeof value.historyStart !== "number" ||
          !Number.isSafeInteger(value.historyStart) ||
          typeof value.text !== "string" ||
          typeof value.hasMore !== "boolean"
        )
          throw new Error("The server did not return saved terminal output.");
        const endOffset = end ?? value.availableOffset;
        end = endOffset;
        if (value.offset < offset || (value.hasMore && value.offset === offset))
          throw new Error(
            "The saved output did not advance. Try the download again.",
          );
        parts.push(value.text);
        incomplete ||= value.truncated;
        offset = value.offset;
        if (!value.hasMore || offset >= endOffset) {
          const url = URL.createObjectURL(
            new Blob(parts, { type: "text/plain;charset=utf-8" }),
          );
          const link = document.createElement("a");
          link.href = url;
          link.download = `terminal-${shell.id}.txt`;
          link.click();
          setTimeout(() => URL.revokeObjectURL(url), 60000);
          setDownloadStatus(
            incomplete
              ? "Saved output downloaded. Earlier output is unavailable."
              : "Saved output downloaded.",
          );
          return;
        }
        setDownloadStatus(
          `Preparing saved output: ${offset.toLocaleString()} of ${endOffset.toLocaleString()} characters`,
        );
      }
      throw new Error(
        "This saved output exceeds the browser download limit (32 million characters).",
      );
    } catch (error) {
      if (downloadAttempt.current === attempt)
        setDownloadStatus(errorText(error));
    } finally {
      if (downloadAttempt.current === attempt) {
        downloadAttempt.current = null;
        setDownloading(false);
      }
    }
  };
  const send = (text: string) => {
    if (inputBlocked.current) return;
    const generation = inputGeneration.current;
    const request_id = randomUUID();
    inputChain.current = inputChain.current
      .then(async () => {
        if (generation !== inputGeneration.current || inputBlocked.current)
          return;
        const pending = {};
        pendingInput.current = pending;
        try {
          const result = await syncPost("/api/terminals/input", {
            id: shell.id,
            text,
            request_id,
          });
          if (generation !== inputGeneration.current) return;
          if (result.ok === false)
            throw new Error(
              result.error || "Terminal input delivery is uncertain.",
            );
        } finally {
          if (pendingInput.current === pending) pendingInput.current = null;
        }
      })
      .catch((e) => {
        if (generation !== inputGeneration.current) return;
        inputBlocked.current = true;
        inputUncertain.current = true;
        setInputWarning(inputUncertaintyNotice);
        if (terminal.current) terminal.current.options.disableStdin = true;
        setInputFailure(
          errorText(e) + " Input paused. Reconnect before you continue.",
        );
      });
  };
  const sendRef = useRef(send);
  sendRef.current = send;
  useEffect(() => {
    const generation = ++inputGeneration.current;
    let disposed = false;
    let observer: ResizeObserver | undefined;
    let resizeTimer: ReturnType<typeof setTimeout>;
    offsetRef.current = 0;
    inputChain.current = Promise.resolve();
    inputBlocked.current = false;
    setInputFailure("");
    if (inputUncertain.current) setInputWarning(inputUncertaintyNotice);
    let instance: Terminal | undefined;
    void Promise.all([
      import("@xterm/xterm"),
      import("@xterm/addon-fit"),
      import("@xterm/xterm/css/xterm.css"),
      // xterm measures glyphs once at open, so the bundled face must be ready.
      document.fonts.load('12px "Geist Mono"').catch(() => []),
    ])
      .then(([{ Terminal }, { FitAddon }]) => {
        if (disposed || !host.current) return;
        instance = new Terminal({
          cursorBlink: true,
          screenReaderMode: true,
          fontSize: 12,
          fontFamily:
            '"Geist Mono", SFMono-Regular, Menlo, Consolas, monospace',
          scrollback: 5000,
          allowProposedApi: false,
          disableStdin: !running(shell.status),
          theme: terminalTheme(),
        });
        terminal.current = instance;
        const fit = new FitAddon();
        instance.loadAddon(fit);
        instance.open(host.current);
        instance.onData((text) => {
          if (!disposed && generation === inputGeneration.current)
            sendRef.current(text);
        });
        const resize = () => {
          if (
            disposed ||
            !host.current?.clientWidth ||
            !host.current.clientHeight
          )
            return;
          fit.fit();
          clearTimeout(resizeTimer);
          resizeTimer = setTimeout(() => {
            if (instance && !disposed && running(shell.status))
              void post("/api/terminals/resize", {
                id: shell.id,
                cols: instance.cols,
                rows: instance.rows,
              }).catch((e) => {
                if (!disposed) setFailure(errorText(e));
              });
          }, 150);
        };
        observer = new ResizeObserver(resize);
        observer.observe(host.current);
        resize();
        applyOutputRef.current = () => {
          const value = outputRef.current;
          if (!value || disposed) return;
          const chars = Array.from(value.text);
          const start = value.offset - chars.length;
          if (offsetRef.current < start || offsetRef.current > value.offset) {
            instance!.reset();
            offsetRef.current = start;
            setTruncated(start > 0);
          }
          const append = chars
            .slice(Math.max(0, offsetRef.current - start))
            .join("");
          if (append) instance!.write(append);
          offsetRef.current = value.offset;
          instance!.options.disableStdin =
            inputBlocked.current || !running(value.status);
          setFailure(value.error || "");
        };
        applyOutputRef.current();
      })
      .catch((e) => {
        if (!disposed) setFailure(errorText(e));
      });
    return () => {
      disposed = true;
      fenceInput();
      applyOutputRef.current = () => {};
      clearTimeout(resizeTimer);
      observer?.disconnect();
      instance?.dispose();
      terminal.current = null;
    };
  }, [shell.id, retry]);
  useEffect(() => {
    applyOutputRef.current();
    if (outputError) setFailure(errorText(outputError));
  }, [
    data?.text,
    data?.offset,
    data?.status,
    data?.error,
    outputError,
    manualOutput,
  ]);
  return (
    <div className="terminal-shell-view">
      {truncated && (
        <div className="terminal-output-notice">
          Earlier output exceeded the retained buffer. The latest output is
          shown.
        </div>
      )}
      {(failure || inputFailure) && (
        <div className="terminal-dock-error" role="status">
          {inputFailure || failure}{" "}
          <button
            onClick={() => {
              fenceInput();
              void get("/api/terminals/output", { query: { id: shell.id } })
                .then((value) => {
                  setManualOutput(value);
                  setFailure("");
                  setRetry((attempt) => attempt + 1);
                })
                .catch((error) => setFailure(errorText(error)));
            }}
          >
            Reconnect
          </button>
        </div>
      )}
      {inputWarning && (
        <p className="terminal-output-notice" role="status">
          {inputWarning}
        </p>
      )}
      <div
        ref={host}
        className="terminal-xterm"
        aria-label="Interactive terminal"
      />
      {downloadStatus && (
        <p className="terminal-output-notice" role="status">
          {downloadStatus}
        </p>
      )}
      <footer className="terminal-output-actions">
        <span>{running(status) ? "Direct shell input" : "Session ended"}</span>
        <span className="terminal-dock-spacer" />
        <Button
          size="compact-xs"
          variant="subtle"
          leftSection={<Download size={12} />}
          onClick={() => {
            if (downloading) {
              downloadAttempt.current = null;
              setDownloading(false);
              setDownloadStatus("Download cancelled.");
            } else void download();
          }}
        >
          {downloading ? "Cancel download" : "Download saved output"}
        </Button>
        <Button
          size="compact-xs"
          variant="subtle"
          leftSection={<Copy size={12} />}
          onClick={() => {
            const text = terminal.current?.getSelection();
            if (!text) {
              notify("Select terminal text first.");
              return;
            }
            void copyText(text).catch((e) => notify(errorText(e)));
          }}
        >
          Copy selection
        </Button>
        <Button
          size="compact-xs"
          variant="subtle"
          disabled={!running(status) || !!inputFailure}
          leftSection={<Square size={11} />}
          onClick={() => send("\u0003")}
        >
          Ctrl+C
        </Button>
      </footer>
    </div>
  );
});
