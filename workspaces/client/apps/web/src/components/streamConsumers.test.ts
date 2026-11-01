import { ProcessSourceKind } from "../generated/api";
import {
  TaskStatus as GeneratedApiTaskStatus,
  TaskKind as GeneratedApiTaskKind,
  WorkStatus as GeneratedApiWorkStatus,
  TerminalState as GeneratedApiTerminalState,
} from "../generated/api.ts";
import type {
  ResourceRef,
  ResourceData,
} from "../sync/subscriptions/contracts";
import type { ComponentHookCell } from "./componentTestFixtures";
import {
  type EffectCallback,
  type PropsWithChildren,
  type ComponentProps,
} from "react";
import { backgroundDisplay, componentSnapshot } from "./componentTestFixtures";
import { afterEach, expect, it, vi } from "vitest";
import type { ReactElement, ReactNode } from "react";
const h = vi.hoisted(() => ({
  cursor: 0,
  cells: [] as ComponentHookCell[],
  effects: [] as Array<() => void>,
  data: new Map<string, unknown>(),
  statuses: new Map<string, string>(),
  watchConnection: vi.fn((accept: (status: string) => void) => {
    accept("live");
    return () => {};
  }),
  get: vi.fn(async () => ({})),
  storage: {},
  writes: [] as string[],
  nullRefs: 0,
  refs: [] as ResourceRef[],
  errors: new Map<string, Error>(),
}));
vi.mock("react", async (original) => {
  const react = await original<typeof import("react")>();
  const memo = (factory: () => unknown, deps: unknown[]) => {
    const index = h.cursor++;
    const old = h.cells[index];
    if (!old || deps.some((value, i) => value !== old.deps![i]))
      h.cells[index] = { deps, value: factory() };
    return required(h.cells[index]).value;
  };
  return {
    ...react,
    useState: (initial: unknown) => {
      const index = h.cursor++;
      h.cells[index] ??= {
        value: typeof initial === "function" ? initial() : initial,
      };
      return [
        required(h.cells[index]).value,
        (value: unknown) => {
          required(h.cells[index]).value =
            typeof value === "function"
              ? value(required(h.cells[index]).value)
              : value;
        },
      ];
    },
    useRef: (current: unknown) => {
      const index = h.cursor++;
      h.cells[index] ??= {
        current:
          current === null && h.nullRefs++ === 0 ? { clientWidth: 0 } : current,
      };
      return h.cells[index];
    },
    useMemo: memo,
    useCallback: (callback: unknown, deps: unknown[]) =>
      memo(() => callback, deps),
    useEffect: (effect: EffectCallback, deps: unknown[]) => {
      const index = h.cursor++;
      const old = h.cells[index];
      if (!old || !deps || deps.some((value, i) => value !== old.deps![i])) {
        old?.cleanup?.();
        h.cells[index] = { deps };
        h.effects.push(() => {
          required(h.cells[index]).cleanup =
            effect() as ComponentHookCell["cleanup"];
        });
      }
    },
    useLayoutEffect: () => {},
  };
});
vi.mock("../sync/subscriptions", () => ({
  useSubscription: (ref: ResourceRef | null) => {
    if (ref) h.refs.push(ref);
    return {
      data: ref ? h.data.get(ref.kind) : undefined,
      status: ref ? (h.statuses.get(ref.kind) ?? "live") : "stale",
      error: ref ? (h.errors.get(ref.kind) ?? null) : null,
    };
  },
}));
vi.mock("../sync/client", () => ({}));
vi.mock("../sync/transcriptCache", () => ({}));
vi.mock("../servers/ServerServicesContext", () => ({
  useServerApi: () => ({
    get: h.get,
    syncGet: h.get,
    saved: (_key: string, value: unknown) => value,
  }),
  useServerStorage: () => h.storage,
  useServerServices: () => ({ owner: { id: "local" } }),
  useServerSync: () => ({}),
  useServerResources: () => ({ watchResourceConnection: h.watchConnection }),
  useServerTranscripts: () => ({ peekTranscript: () => undefined }),
}));
vi.mock("@xterm/xterm", () => ({
  Terminal: class {
    options = {};
    write(value: string) {
      h.writes.push(value);
    }
    loadAddon() {}
    open() {}
    onData() {}
    reset() {}
    dispose() {}
  },
}));
vi.mock("@xterm/addon-fit", () => ({
  FitAddon: class {
    fit() {}
  },
}));
import BackgroundTasks from "./shell/BackgroundTasks";
import type { BackgroundTaskStrip, TaskDetail } from "./shell/BackgroundTasks";
import Tasks from "./shell/Tasks";
import { UserMessage } from "./shell/messages/UserMessages";
import { ShellView } from "./TerminalDock";
import { useMessages } from "../hooks";
const snapshot = () => componentSnapshot([{ id: "lead", epoch: 1 }]);
function render<T>(fn: () => T, offset = 0) {
  h.cursor = offset;
  const result = fn();
  for (const effect of h.effects.splice(0)) effect();
  return result;
}
afterEach(() => {
  for (const cell of h.cells) cell?.cleanup?.();
  h.cells = [];
  h.effects = [];
  h.data.clear();
  h.statuses.clear();
  h.watchConnection.mockClear();
  h.get.mockClear();
  h.writes = [];
  h.nullRefs = 0;
  h.refs = [];
  h.errors.clear();
  vi.unstubAllGlobals();
});
function elements(
  node: ReactNode,
): ReactElement<PropsWithChildren<Record<string, unknown>>>[] {
  if (Array.isArray(node)) return node.flatMap(elements);
  if (node && typeof node === "object" && "props" in node) {
    const element = node as ReactElement<
      PropsWithChildren<Record<string, unknown>>
    >;
    return [element, ...elements(element.props.children)];
  }
  return [];
}
const processProps = () => ({
  opened: true,
  close: vi.fn(),
  data: snapshot(),
  leadId: "lead",
  openAgent: vi.fn(),
  refresh: vi.fn(),
  notify: vi.fn(),
});
const commandProcess = () =>
  ({
    id: "command",
    name: "command",
    agent: "lead",
    created: 2,
    status: "running" as const,
    source: ProcessSourceKind.codex_task,
    key: "command-key",
    generation: "generation",
    accountKey: "default",
    epoch: 1,
    tail: "Streamed command output",
    bytes: 23,
    cancelRequested: false,
    outputTruncated: false,
    outputPatterns: [],
    display: backgroundDisplay({
      kindLabel: "backgroundCommand",
      title: "command",
      actions: ["readOutput", "stop", "input"],
      stdinState: "open",
    }),
  }) satisfies ResourceData<
    Extract<ResourceRef, { kind: "processes" }>
  >["processes"][number];
it("allows stopping a recovered server-verified response when the owner is not in flight", () => {
  h.nullRefs = 100;
  const props = processProps();
  props.data.threads[0] = { id: "lead", epoch: 1, inFlight: false };
  h.data.set("processes", {
    processes: [],
    activities: [],
    responseTarget: {
      agent: "lead",
      accountKey: "default",
      epoch: 1,
      connectionId: "connection",
      threadId: "thread",
      turnId: "turn",
    },
  });
  const button = elements(render(() => BackgroundTasks(props))).find(
    (node) => node.props.children === "Stop response",
  );
  expect(button?.props.disabled).toBe(false);
});
it("keeps monitor resize available after stdin closes", () => {
  h.nullRefs = 100;
  const process = {
    ...commandProcess(),
    source: ProcessSourceKind.monitor,
    display: backgroundDisplay({
      kind: "monitor",
      kindLabel: "monitor",
      stdinState: "closed",
      actions: ["resize", "readOutput"],
    }),
  };
  h.data.set("processes", {
    processes: [process],
    activities: [{ source: "process", value: process }],
  });
  const detail = elements(render(() => BackgroundTasks(processProps()))).find(
    (node) => node.props.canStop !== undefined,
  )!;
  const view = render(
    () =>
      (detail.type as typeof TaskDetail)(
        detail.props as ComponentProps<typeof TaskDetail>,
      ),
    100,
  );
  const input = elements(view).find(
    (node) => node.props.receiptOnly !== undefined,
  );
  expect(input?.props.receiptOnly).toBe(false);
});
it("an open process panel and its selected detail need only two distinct resource subscriptions", () => {
  h.nullRefs = 100;
  h.data.set(
    "tasks",
    [] satisfies ResourceData<Extract<ResourceRef, { kind: "tasks" }>>,
  );
  h.data.set("processes", {
    processes: [commandProcess()],
    activities: [{ source: "process", value: commandProcess() }],
  } satisfies ResourceData<Extract<ResourceRef, { kind: "processes" }>>);
  const view = render(() => BackgroundTasks(processProps()));
  const detail = elements(view).find(
    (node) => node.props.canStop !== undefined,
  )!;
  render(
    () =>
      (detail.type as typeof TaskDetail)(
        detail.props as ComponentProps<typeof TaskDetail>,
      ),
    100,
  );
  expect(h.refs).toEqual([{ kind: "processes", agentId: "lead" }]);
  expect(h.get).not.toHaveBeenCalled();
});
it("renders the server activity exactly once as its process starts and ends", () => {
  h.nullRefs = 100;
  const props = processProps();
  const process = commandProcess();
  const task = {
    id: "command",
    agent: "lead",
    created: 2,
    status: GeneratedApiTaskStatus.running,
    kind: GeneratedApiTaskKind.command,
    name: "command",
    turnId: "turn",
    tail: "Task output",
    display: backgroundDisplay(),
  };
  const rows = () =>
    elements(render(() => BackgroundTasks(props))).filter(
      (
        node,
      ): node is ReactElement<ComponentProps<typeof BackgroundTaskStrip>> =>
        !!node.props.select,
    );
  h.data.set("processes", {
    processes: [],
    activities: [{ source: "task", value: task }],
  });
  expect(rows().map((row) => row.props.task.id)).toEqual(["command"]);
  expect(rows()[0]?.props.task.tail).toBe("Task output");
  h.data.set("processes", {
    processes: [process],
    activities: [{ source: "process", value: process }],
  });
  expect(rows().map((row) => row.props.task.id)).toEqual(["command"]);
  expect(rows()[0]?.props.task.tail).toBe("Streamed command output");
  h.data.set("processes", {
    processes: [],
    activities: [{ source: "task", value: task }],
  });
  expect(rows().map((row) => row.props.task.id)).toEqual(["command"]);
  expect(rows()[0]?.props.task.tail).toBe("Task output");
  expect(h.get).not.toHaveBeenCalled();
});
it.each([false, true])(
  "a process panel without visible scope has no resource subscriptions (opened=%s)",
  (opened) => {
    h.nullRefs = 100;
    render(() =>
      BackgroundTasks({
        ...processProps(),
        opened,
        ...(opened ? { leadId: undefined } : {}),
      }),
    );
    expect(h.refs).toEqual([]);
    expect(h.get).not.toHaveBeenCalled();
  },
);
it("process stream failures remain visible", () => {
  h.nullRefs = 100;
  h.data.set("processes", { processes: [], activities: [] });
  h.statuses.set("processes", "error");
  h.errors.set("processes", new Error("Process stream unavailable"));
  const nodes = elements(render(() => BackgroundTasks(processProps())));
  expect(
    nodes
      .filter((node) => node.props.role === "alert")
      .flatMap((node) => node.props.children)
      .join(""),
  ).toContain("Process stream unavailable");
  expect(h.get).not.toHaveBeenCalled();
});
it("process owner/task events consume process records without GET /api/processes", () => {
  h.nullRefs = 100;
  h.data.set("processes", {
    processes: [],
    activities: [],
  } satisfies ResourceData<Extract<ResourceRef, { kind: "processes" }>>);
  const props = {
    opened: true,
    close: vi.fn(),
    data: snapshot(),
    leadId: "lead",
    openAgent: vi.fn(),
    refresh: vi.fn(),
    notify: vi.fn(),
  };
  render(() => BackgroundTasks(props));
  h.data.set("processes", {
    processes: [],
    activities: [],
  } satisfies ResourceData<Extract<ResourceRef, { kind: "processes" }>>);
  render(() =>
    BackgroundTasks({
      ...props,
      data: {
        ...snapshot(),
        threads: [{ id: "lead", epoch: 2 }],
        runtime: { ...snapshot().runtime, tasks: [] },
      },
    }),
  );
  expect(h.get).not.toHaveBeenCalled();
});
it("work changes consume the stream without GET /api/work", () => {
  h.nullRefs = 100;
  h.data.set("agentDetail", {
    id: "lead",
    taskDefaultGate: null,
  } satisfies ResourceData<Extract<ResourceRef, { kind: "agentDetail" }>>);
  const task = {
    id: "work",
    rootId: "lead",
    title: "Before",
    created: 1,
    updated: 1,
    version: 1,
    status: GeneratedApiWorkStatus.running,
    policy: {
      blockedBy: [],
      displayStatus: GeneratedApiWorkStatus.running,
      acceptAvailable: false,
      gateOverrideRequired: false,
    },
  } as ResourceData<Extract<ResourceRef, { kind: "work" }>>["items"][number];
  h.data.set("work", { items: [task], tasks: [] } satisfies ResourceData<
    Extract<ResourceRef, { kind: "work" }>
  >);
  const titles = () =>
    elements(render(() => Tasks({ agentId: "lead" })))
      .filter((node) => node.type === "strong")
      .map((node) => node.props.children);
  expect(titles()).toContain("Before");
  h.data.set("work", {
    items: [{ ...task, title: "After" } as typeof task],
    tasks: [],
  } satisfies ResourceData<Extract<ResourceRef, { kind: "work" }>>);
  expect(titles()).toContain("After");
  expect(h.get).not.toHaveBeenCalled();
});
it("complaint version changes consume streamed detail without GET /api/complaint", () => {
  h.nullRefs = 100;
  const props: ComponentProps<typeof UserMessage> = {
    message: { id: "message", version: 1 },
    data: snapshot(),
    focused: false,
    requests: new Map(),
    pendingKey: "replies",
    refresh: vi.fn(),
    notify: vi.fn(),
  };
  h.data.set("complaint", {
    id: "message",
    version: 1,
    text: "First",
    responses: [],
  } satisfies ResourceData<Extract<ResourceRef, { kind: "complaint" }>>);
  render(() => UserMessage(props));
  h.data.set("complaint", {
    id: "message",
    version: 2,
    text: "Updated",
    responses: [],
  } satisfies ResourceData<Extract<ResourceRef, { kind: "complaint" }>>);
  render(() =>
    UserMessage({ ...props, message: { ...props.message, version: 2 } }),
  );
  expect(h.get).not.toHaveBeenCalled();
});
it.each(["agent", "room"] as const)(
  "%s records update a nonmanaged conversation without a resource GET",
  (kind) => {
    h.nullRefs = 100;
    h.data.set(
      kind === "agent" ? "transcript" : "room",
      // Transcript omits truncated; room messages omit senderName, room and deliveries.
      kind === "agent"
        ? ({
            items: [{ id: "new", role: "assistant", text: "Streamed" }],
          } as ResourceData<Extract<ResourceRef, { kind: "transcript" }>>)
        : ({
            messages: [
              {
                id: "new",
                seq: 1,
                text: "Streamed",
                created: 1,
                sender: "lead",
              },
            ],
            room: { id: "team" },
          } as ResourceData<Extract<ResourceRef, { kind: "room" }>>),
    );
    const read = () =>
      useMessages("team", kind, false, "workspace", "workspace");
    render(read);
    expect(render(read).items.map((item) => item.text)).toEqual(["Streamed"]);
    expect(h.get).not.toHaveBeenCalled();
  },
);
it("keeps live agent transport after visiting a stale room in the same component", () => {
  h.nullRefs = 100;
  h.data.set("room", {
    messages: [],
    room: { id: "team" },
  } satisfies ResourceData<Extract<ResourceRef, { kind: "room" }>>);
  h.statuses.set("room", "stale");
  const agent = () =>
    useMessages("lead", "agent", true, "workspace", "workspace");
  const room = () =>
    useMessages("team", "room", true, "workspace", "workspace");
  render(agent);
  expect(render(agent).connection).toMatchObject({
    health: "live",
    message: "",
  });
  render(room);
  expect(render(room).connection).toBeNull();
  render(agent);
  expect(render(agent).connection).toMatchObject({
    health: "live",
    message: "",
  });
  expect(h.watchConnection).toHaveBeenCalledTimes(2);
  expect(h.get).not.toHaveBeenCalled();
});
it("terminal hints apply Unicode appends without GET /api/terminals/output", async () => {
  vi.stubGlobal(
    "ResizeObserver",
    class {
      observe() {}
      disconnect() {}
    },
  );
  const shell = {
    id: "shell",
    agent: "lead",
    title: "Shell",
    cwd: "/",
    status: "running",
    created: 1,
  } as const;
  h.data.set("terminal-output", {
    text: "a🙂",
    offset: 2,
    status: GeneratedApiTerminalState.running,
    truncated: false,
  } satisfies ResourceData<Extract<ResourceRef, { kind: "terminal-output" }>>);
  const read = () => ShellView({ shell, notify: vi.fn() });
  render(read);
  await vi.waitFor(() => expect(h.writes).toEqual(["a🙂"]));
  h.data.set("terminal-output", {
    text: "a🙂中",
    offset: 3,
    status: GeneratedApiTerminalState.running,
    truncated: false,
  } satisfies ResourceData<Extract<ResourceRef, { kind: "terminal-output" }>>);
  render(read);
  expect(h.writes).toEqual(["a🙂", "中"]);
  expect(h.get).not.toHaveBeenCalled();
});
import { required } from "./componentTestFixtures";
