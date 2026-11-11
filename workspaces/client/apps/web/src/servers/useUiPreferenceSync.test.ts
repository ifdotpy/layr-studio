import { afterEach, beforeEach, expect, it, vi } from "vitest";
import {
  readServers,
  writeServers,
  SERVER_REGISTRY_KEY,
  type StudioServer,
} from "./registry.ts";
import {
  readPreferenceFields,
  receivePreferenceFields,
} from "../sync/uiPreferenceStore.ts";
import { preferenceMemory } from "./storage.ts";

const mocks = vi.hoisted(() => ({
  effects: [] as (() => void | (() => void))[],
  streams: [] as EventTarget[],
  fetch: vi.fn(async () => Response.json({})),
}));
vi.mock(
  "../components/servers/ServerServicesProvider/ServerServicesProvider.tsx",
  () => ({
    useServerApi: () => ({ fetchApiRequest: mocks.fetch }),
  }),
);
vi.mock("./net/eventSource.ts", () => ({
  SignedEventSource: class extends EventTarget {
    constructor() {
      super();
      mocks.streams.push(this);
    }
    close() {}
  },
}));
import { startUiPreferenceSync } from "./useUiPreferenceSync.ts";

const local = { id: "local", origin: "http://local.invalid", label: "Local" };
const paired = {
  id: "paired",
  origin: "https://paired.tailnet.ts.net",
  credentialId: "paired-credential",
  label: "Paired",
  alias: "PAI",
};
let cleanups: (() => void)[];
let storageValues: Map<string, string>;
function mount(
  servers: StudioServer[] = [
    local,
    { ...paired, workspaceId: "paired-workspace" },
  ],
) {
  cleanups.push(startUiPreferenceSync(servers));
}
beforeEach(() => {
  preferenceMemory.clear();
  mocks.effects.length = 0;
  mocks.streams.length = 0;
  mocks.fetch.mockClear();
  cleanups = [];
  const values = (storageValues = new Map<string, string>());
  vi.stubGlobal("localStorage", {
    getItem: (key: string) => values.get(key) ?? null,
    setItem: (key: string, value: string) => values.set(key, value),
    get length() {
      return values.size;
    },
    key: (index: number) => [...values.keys()][index] ?? null,
  });
  vi.stubGlobal("window", new EventTarget());
  vi.stubGlobal("location", { origin: "http://local.invalid" });
  localStorage.setItem(SERVER_REGISTRY_KEY, JSON.stringify([paired]));
});

it("captures a user reorder in preferences while persisting only connection fields", () => {
  const first = { ...paired, workspaceId: "paired-workspace" };
  const second = {
    ...paired,
    id: "second",
    credentialId: "second-credential",
    origin: "https://second.tailnet.ts.net",
    alias: "SEC",
    workspaceId: "second-workspace",
  };
  writeServers([first, second]);
  mount([{ ...local, workspaceId: "local-workspace" }, first, second]);
  writeServers([second, first]);
  expect(
    readPreferenceFields("user")[JSON.stringify(["server-order"])]?.value,
  ).toEqual(["local-workspace", "second-workspace", "paired-workspace"]);
  expect(JSON.parse(storageValues.get(SERVER_REGISTRY_KEY)!)).toEqual([
    { id: "paired", origin: paired.origin, credentialId: paired.credentialId },
    { id: "second", origin: second.origin, credentialId: second.credentialId },
  ]);
});
afterEach(() => {
  cleanups.forEach((cleanup) => cleanup());
  vi.unstubAllGlobals();
});

it("leaves preference transport to mounted server projections", () => {
  mount();
  // Ordinary revision-only frames must have no independent preference listener.
  for (let revision = 1; revision <= 10; revision++) {
    for (const stream of mocks.streams) {
      stream.dispatchEvent(
        new MessageEvent("resources", {
          data: JSON.stringify({
            protocol: 4,
            workspaceId: "local-workspace",
            epoch: "epoch",
            reason: "change",
            resources: [{ kind: "state" }],
            resourceVersions: [{ revision }],
          }),
        }),
      );
    }
  }
  expect(mocks.streams).toHaveLength(0);
  expect(mocks.fetch).not.toHaveBeenCalled();
});

it("applies a server preference using the streamed workspace identity", () => {
  const key = JSON.stringify(["server-label:paired-workspace"]);
  receivePreferenceFields("user", {
    [key]: { value: "Remote name", timestamp: 1, writer: "remote" },
  });
  mount();
  expect(readServers()[0]?.label).toBe("Remote name");
  expect(
    JSON.parse(localStorage.getItem(SERVER_REGISTRY_KEY)!)[0].label,
  ).toBeUndefined();
});

it("applies streamed preference changes through the observable cache after startup", () => {
  mount();
  receivePreferenceFields("user", {
    '["server-label:paired-workspace"]': {
      value: "Live name",
      timestamp: 2,
      writer: "remote",
    },
  });
  expect(readServers()[0]?.label).toBe("Live name");
});
