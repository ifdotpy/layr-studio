import type {
  ResourceRef,
  ResourceData,
} from "../sync/subscriptions/contracts";
import type { ComponentHookCell } from "./componentTestFixtures";
import { type EffectCallback, type ComponentProps } from "react";
import { componentSnapshot } from "./componentTestFixtures";
import { afterEach, expect, it, vi } from "vitest";
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
  post: vi.fn(async () => ({ status: "completed", processes: [] })),
  storage: {},
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
    post: h.post,
    syncGet: h.get,
    saved: (_key: string, value: unknown) => value,
  }),
  useServerStorage: () => h.storage,
  useServerServices: () => ({ owner: { id: "local" } }),
  useServerSync: () => ({}),
  useServerResources: () => ({ watchResourceConnection: h.watchConnection }),
  useServerTranscripts: () => ({ peekTranscript: () => undefined }),
}));
import { UserMessage } from "./shell/messages/UserMessages";
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
  h.post.mockClear();
  h.nullRefs = 0;
  h.refs = [];
  h.errors.clear();
  vi.unstubAllGlobals();
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
import { required } from "./componentTestFixtures";
