import {
  flowResult,
  makeAutoObservable,
  observableRef,
  onBecomeObserved,
  onBecomeUnobserved,
  reaction,
} from "mobx";
import type { Terminal } from "@xterm/xterm";
import { requestId } from "studio-claude-bridge/request-id.ts";
import { errorText, type GetResult } from "../../../api.ts";
import type * as Api from "../../../servers/net/api.ts";
import { resourceData, type ResourceSlot } from "../../../store/runtime.ts";
import type {
  TerminalOutput,
  TerminalRecord,
} from "../../../generated/observableResources.ts";

export const running = (status: string) =>
  ["running", "starting", "approval"].includes(status);
const uncertainty =
  "Previous input delivery is uncertain. Check the terminal output before you enter it again. Buffered input was not sent.";
export function* wait<T>(promise: Promise<T>): Generator<Promise<T>, T, T> {
  return yield promise;
}
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

/** Observed shell state owns flows; the ref owns only xterm's DOM bindings. */
export function createShellTerminalStore(
  shell: TerminalRecord,
  slot: ResourceSlot<TerminalOutput>,
  api: Pick<typeof Api, "get" | "post" | "syncPost">,
) {
  const io = {
    terminal: null as Terminal | null,
    host: null as HTMLDivElement | null,
    offset: 0,
    generation: 0,
    blocked: false,
    uncertain: false,
    pending: false,
    chain: Promise.resolve(),
    attempt: null as symbol | null,
    stop: undefined as (() => void) | undefined,
    observer: undefined as ResizeObserver | undefined,
    timer: undefined as ReturnType<typeof setTimeout> | undefined,
  };
  const state = {
    binding: { shell, slot, api },
    bind(
      shell: TerminalRecord,
      slot: ResourceSlot<TerminalOutput>,
      api: Pick<typeof Api, "get" | "post" | "syncPost">,
    ) {
      const current = this.binding;
      if (
        current.shell === shell &&
        current.slot === slot &&
        current.api.get === api.get &&
        current.api.post === api.post &&
        current.api.syncPost === api.syncPost
      )
        return;
      const host = io.host;
      this.unmount();
      if (io.attempt) this.cancelDownload();
      this.binding = { shell, slot, api };
      this.manual = undefined;
      this.baseline = [];
      if (host) this.mount(host);
    },
    failure: "",
    inputFailure: "",
    inputWarning: "",
    truncated: false,
    downloading: false,
    downloadStatus: "",
    manual: undefined as GetResult<"/api/terminals/output"> | undefined,
    baseline: [] as readonly unknown[],

    get source() {
      return resourceData(this.binding.slot.current);
    },
    get version(): readonly (string | number | undefined)[] {
      const d = this.source;
      return [d?.text, d?.offset, d?.status];
    },
    get output() {
      return this.manual && this.version.every((v, i) => v === this.baseline[i])
        ? this.manual
        : this.source;
    },
    get status() {
      return this.output?.status ?? this.binding.shell.status;
    },
    start() {
      io.stop ??= reaction(
        () => [
          this.output?.text,
          this.output?.offset,
          this.output?.status,
          this.output?.error,
          "error" in this.binding.slot.current
            ? this.binding.slot.current.error
            : undefined,
        ],
        this.paint,
      );
    },
    dispose() {
      io.stop?.();
      io.stop = undefined;
      io.attempt = null;
      this.fence();
    },
    fence() {
      if (io.pending) io.uncertain = true;
      io.generation++;
      io.pending = false;
      io.chain = Promise.resolve();
      io.blocked = true;
      if (io.terminal) io.terminal.options.disableStdin = true;
    },
    paint() {
      const value = this.output,
        terminal = io.terminal;
      if (terminal && value) {
        const chars = Array.from(value.text),
          start = value.offset - chars.length;
        if (io.offset < start || io.offset > value.offset) {
          terminal.reset();
          io.offset = start;
          this.truncated = start > 0;
        }
        const append = chars.slice(Math.max(0, io.offset - start)).join("");
        if (append) terminal.write(append);
        io.offset = value.offset;
        terminal.options.disableStdin = io.blocked || !running(value.status);
      }
      const error =
        "error" in this.binding.slot.current
          ? this.binding.slot.current.error
          : undefined;
      this.failure = error ? errorText(error) : value?.error || "";
    },
    send(text: string) {
      if (io.blocked) return;
      const previous = io.chain,
        generation = io.generation;
      io.chain = flowResult(this.input(previous, generation, text));
    },
    *input(previous: Promise<void>, generation: number, text: string) {
      yield* wait(previous);
      if (generation !== io.generation || io.blocked) return;
      io.pending = true;
      try {
        const result = yield* wait(
          this.binding.api.syncPost("/api/terminals/input", {
            id: this.binding.shell.id,
            text,
            request_id: requestId(),
          }),
        );
        if (generation !== io.generation) return;
        if (result.ok === false)
          throw new Error(
            result.error || "Terminal input delivery is uncertain.",
          );
      } catch (error) {
        if (generation !== io.generation) return;
        io.blocked = io.uncertain = true;
        this.inputWarning = uncertainty;
        if (io.terminal) io.terminal.options.disableStdin = true;
        this.inputFailure =
          errorText(error) + " Input paused. Reconnect before you continue.";
      } finally {
        if (generation === io.generation) io.pending = false;
      }
    },
    *reconnect() {
      this.fence();
      const generation = io.generation;
      try {
        const value = yield* wait(
          this.binding.api.get("/api/terminals/output", {
            query: { id: this.binding.shell.id },
          }),
        );
        if (generation !== io.generation) return;
        this.baseline = this.version;
        this.manual = value;
        this.failure = "";
        const host = io.host;
        this.unmount();
        if (host) this.mount(host);
      } catch (error) {
        if (generation === io.generation) this.failure = errorText(error);
      }
    },
    cancelDownload() {
      io.attempt = null;
      this.downloading = false;
      this.downloadStatus = "Download cancelled.";
    },
    *download() {
      if (io.attempt) return;
      const attempt = Symbol();
      io.attempt = attempt;
      this.downloading = true;
      this.downloadStatus = "Preparing saved output…";
      try {
        const parts: string[] = [];
        let offset = 0,
          end: number | undefined;
        let incomplete = false;
        for (let page = 0; page < 512; page++) {
          const value = yield* wait(
            this.binding.api.get("/api/terminals/output", {
              query: {
                id: this.binding.shell.id,
                history: "1",
                offset,
                limit: Math.min(
                  65536,
                  end === undefined ? 65536 : end - offset,
                ),
              },
            }),
          );
          if (io.attempt !== attempt) return;
          if (
            value.availableOffset == null ||
            value.historyStart == null ||
            value.hasMore == null
          )
            throw new Error("The server did not return saved terminal output.");
          end ??= value.availableOffset;
          if (
            value.offset < offset ||
            (value.hasMore && value.offset === offset)
          )
            throw new Error(
              "The saved output did not advance. Try the download again.",
            );
          parts.push(value.text);
          incomplete ||= value.truncated;
          offset = value.offset;
          if (!value.hasMore || offset >= end) {
            const url = URL.createObjectURL(
              new Blob(parts, { type: "text/plain;charset=utf-8" }),
            );
            Object.assign(document.createElement("a"), {
              href: url,
              download: `terminal-${this.binding.shell.id}.txt`,
            }).click();
            setTimeout(() => URL.revokeObjectURL(url), 60000);
            this.downloadStatus = `Saved output downloaded.${incomplete ? " Earlier output is unavailable." : ""}`;
            return;
          }
          this.downloadStatus = `Preparing saved output: ${offset.toLocaleString()} of ${end.toLocaleString()} characters`;
        }
        throw new Error(
          "This saved output exceeds the browser download limit (32 million characters).",
        );
      } catch (error) {
        if (io.attempt === attempt) this.downloadStatus = errorText(error);
      } finally {
        if (io.attempt === attempt) {
          io.attempt = null;
          this.downloading = false;
        }
      }
    },
    get terminal() {
      return io.terminal;
    },
    // Only xterm and its DOM input/layout bindings follow the ref lifetime.
    mount(host: HTMLDivElement | null) {
      if (!host) return;
      io.host = host;
      io.offset = 0;
      io.blocked = false;
      this.inputFailure = "";
      if (io.uncertain) this.inputWarning = uncertainty;
      void this.open(host, ++io.generation);
      return this.unmount;
    },
    unmount() {
      this.fence();
      clearTimeout(io.timer);
      io.observer?.disconnect();
      io.terminal?.dispose();
      io.terminal = null;
      io.host = null;
    },
    *open(host: HTMLDivElement, generation: number) {
      try {
        const [{ Terminal }, { FitAddon }] = yield* wait(
          Promise.all([
            import("@xterm/xterm"),
            import("@xterm/addon-fit"),
            import("@xterm/xterm/css/xterm.css"),
            document.fonts.load('12px "Geist Mono"').catch(() => []),
          ]),
        );
        if (generation !== io.generation) return;
        const terminal = new Terminal({
          cursorBlink: true,
          screenReaderMode: true,
          fontSize: 12,
          fontFamily:
            '"Geist Mono", SFMono-Regular, Menlo, Consolas, monospace',
          scrollback: 5000,
          allowProposedApi: false,
          disableStdin: !running(this.status),
          theme: terminalTheme(),
        });
        io.terminal = terminal;
        const fit = new FitAddon();
        terminal.loadAddon(fit);
        terminal.open(host);
        terminal.onData((text) => {
          if (generation === io.generation) this.send(text);
        });
        const resize = () => {
          if (!host.clientWidth || !host.clientHeight) return;
          fit.fit();
          clearTimeout(io.timer);
          io.timer = setTimeout(() => {
            if (generation === io.generation) void this.resize();
          }, 150);
        };
        io.observer = new ResizeObserver(resize);
        io.observer.observe(host);
        resize();
        this.paint();
      } catch (error) {
        if (generation === io.generation) this.failure = errorText(error);
      }
    },
    *resize() {
      const terminal = io.terminal,
        generation = io.generation;
      if (!terminal || !running(this.status)) return;
      try {
        yield* wait(
          this.binding.api.post("/api/terminals/resize", {
            id: this.binding.shell.id,
            cols: terminal.cols,
            rows: terminal.rows,
          }),
        );
      } catch (error) {
        if (generation === io.generation) this.failure = errorText(error);
      }
    },
  };
  const store = makeAutoObservable(
    state,
    { binding: observableRef },
    { autoBind: true },
  );
  onBecomeObserved(store, "status", store.start);
  onBecomeUnobserved(store, "status", store.dispose);
  return store;
}
