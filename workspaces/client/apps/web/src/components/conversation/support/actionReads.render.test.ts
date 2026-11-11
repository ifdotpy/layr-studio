import { createNet } from "../../../servers/net/net.ts";
// @vitest-environment happy-dom
import {
  act,
  createElement as h,
  StrictMode,
  useMemo,
  useRef,
  useState,
} from "react";
import { FrontendLocality } from "../../../generated/api.ts";
import { observer } from "mobx-react-lite";
import { createRoot } from "react-dom/client";
import { MantineProvider } from "@mantine/core";
import { afterEach, expect, it, vi } from "vitest";
import type { GetResult, ServerApi } from "../../../api.ts";
import { createServerStore } from "../../../store/server.ts";
import { createSubscriptions } from "../../../sync/subscriptions/module.ts";
import { ServerStoreProvider } from "../../servers/ServerServicesProvider/ServerServicesProvider.tsx";
import FilePreview from "../FilePreview/FilePreview.tsx";
import { PreviewSelection } from "./PreviewSelection.ts";
import DirectoryField from "../../projects/DirectoryField/DirectoryField.tsx";
import { SkillAutocomplete } from "./SkillAutocomplete.ts";
import Analytics, {
  type AnalyticsSelection,
} from "../../shell/Analytics/Analytics.tsx";
import {
  useServerApi,
  useServerNet,
  useServerStore,
} from "../../servers/ServerServicesProvider/ServerServicesProvider.tsx";

const fixture = vi.hoisted(() => ({
  get: vi.fn<ServerApi["get"]>(),
  sessionId: "one",
}));
const streamSession = () => ({
  workspaceId: "workspace",
  sessionId: fixture.sessionId,
  locality: FrontendLocality.local,
  routes: [],
  cachePolicies: [],
});
vi.mock(
  "../../servers/ServerServicesProvider/ServerServicesProvider.tsx",
  async (original) => {
    const actual =
      await original<
        typeof import("../../servers/ServerServicesProvider/ServerServicesProvider.tsx")
      >();
    return {
      ...actual,
      useServerNet: () => {
        const store = actual.useServerStore();
        return useMemo(() => {
          const boundApi = { ...api, getStreamSession: streamSession };
          vi.spyOn(boundApi, "get").mockImplementation(fixture.get);
          return createNet(boundApi, () => store);
        }, [store]);
      },
      useServerApi: () => ({
        ...api,
        get: fixture.get,
        getStreamSession: streamSession,
      }),
      useServerFileDialogs: () => ({
        canPickDirectory: () => false,
        pickDirectory: async () => null,
      }),
    };
  },
);
import * as api from "../../../servers/net/api.ts";
Object.assign(globalThis, { IS_REACT_ACT_ENVIRONMENT: true });
afterEach(() => {
  fixture.get.mockReset();
  fixture.sessionId = "one";
  vi.restoreAllMocks();
  vi.useRealTimers();
  document.body.replaceChildren();
});

const Previews = observer(function Previews() {
  const [revision, setRevision] = useState(0);
  const net = useServerNet();
  const [leftOwner] = useState(() => new PreviewSelection()),
    [rightOwner] = useState(() => new PreviewSelection());
  const left = leftOwner.current(net),
    right = rightOwner.current(net);
  const openLeft = (target: Parameters<PreviewSelection["set"]>[1]) =>
    leftOwner.set(net, target);
  const openRight = (target: Parameters<PreviewSelection["set"]>[1]) =>
    rightOwner.set(net, target);
  return h(
    "div",
    { "data-revision": revision },
    h(
      "button",
      {
        onClick: () => {
          openLeft({ asset: "same" });
          openRight({ asset: "same" });
        },
      },
      "Open both",
    ),
    h("button", { onClick: () => openLeft({ asset: "new" }) }, "Open newer"),
    h(
      "button",
      {
        onClick: () => {
          fixture.sessionId = "two";
          setRevision(revision + 1);
        },
      },
      "Change session",
    ),
    h(
      "button",
      {
        onClick: () => {
          openLeft(null);
          openRight(null);
        },
      },
      "Close both",
    ),
    left && h(FilePreview, { target: left, onClose: () => openLeft(null) }),
    right && h(FilePreview, { target: right, onClose: () => openRight(null) }),
  );
});
const MountedPreview = observer(function MountedPreview() {
  const net = useServerNet();
  const [owner] = useState(() => new PreviewSelection());
  const target = owner.current(net);
  const open = (target: Parameters<PreviewSelection["set"]>[1]) =>
    owner.set(net, target);
  return h(
    "div",
    null,
    h("button", { onClick: () => open({ asset: "image" }) }, "Open"),
    h("button", { onClick: () => open(null) }, "Close"),
    h(FilePreview, { target, onClose: () => open(null) }),
  );
});
function AnalyticsView() {
  const api = useServerApi(),
    store = useServerStore();
  const [selected, update] = useState<{
    store: typeof store;
    value: AnalyticsSelection | null;
  }>();
  const selection = selected?.store === store ? selected.value : null;
  const setSelection = (value: AnalyticsSelection | null) =>
    update({ store, value });
  return h(
    "div",
    null,
    h(
      "button",
      {
        onClick: () => {
          const query = {
            agent: "agent",
            scope: "agent" as const,
            limit: 30,
            offset: 0,
            to_time: Math.floor(Date.now() / 1000),
          };
          const read = store.reads.analytics(api, query);
          void read.read().catch(() => {});
          setSelection({ query, read });
        },
      },
      "Open analytics",
    ),
    selection &&
      h(Analytics, {
        agent: { id: "agent", name: "Agent" },
        selection,
        onClose: () => setSelection(null),
      }),
  );
}
async function render(children: React.ReactNode) {
  const subscriptions = createSubscriptions({
    watchResourceChanges: () => () => {},
    watchResourceConnection: () => () => {},
  });
  const store = createServerStore(subscriptions);
  const owners = [{ store, subscriptions }];
  const host = document.body.appendChild(document.createElement("div")),
    root = createRoot(host);
  await act(async () =>
    root.render(
      h(
        StrictMode,
        null,
        h(
          MantineProvider,
          null,
          h(ServerStoreProvider, { value: store }, children),
        ),
      ),
    ),
  );
  return {
    host,
    store,
    async switchOwner() {
      const subscriptions = createSubscriptions({
        watchResourceChanges: () => () => {},
        watchResourceConnection: () => () => {},
      });
      const store = createServerStore(subscriptions);
      owners.push({ store, subscriptions });
      await act(async () =>
        root.render(
          h(
            StrictMode,
            null,
            h(
              MantineProvider,
              null,
              h(ServerStoreProvider, { value: store }, children),
            ),
          ),
        ),
      );
    },
    async close() {
      await act(async () => root.unmount());
      for (const owner of owners) {
        owner.store.dispose();
        owner.subscriptions.dispose();
      }
    },
  };
}

it("preview opens share bytes and discard an earlier target after navigation", async () => {
  const old = Promise.withResolvers<GetResult<"/api/file">>();
  fixture.get.mockReturnValueOnce(old.promise).mockResolvedValueOnce({
    name: "new-file",
    mime: "text/plain",
    base64: "bmV3",
  });
  const allocate = vi.spyOn(URL, "createObjectURL");
  const view = await render(h(Previews));
  expect(fixture.get).not.toHaveBeenCalled();
  await act(async () => view.host.querySelectorAll("button")[0]!.click());
  expect(fixture.get).toHaveBeenCalledTimes(1);
  await act(async () => view.host.querySelectorAll("button")[1]!.click());
  expect(fixture.get).toHaveBeenCalledTimes(2);
  await act(async () => {
    old.resolve({ name: "old-file", mime: "text/plain", base64: "b2xk" });
    await old.promise;
  });
  expect(document.body.textContent).toContain("new-file");
  // The second consumer still intentionally displays its original target.
  expect(document.body.textContent).toContain("old-file");
  const dialogs = document.querySelectorAll('[role="dialog"]');
  expect(dialogs[0]?.textContent).toContain("new-file");
  expect(dialogs[0]?.textContent).not.toContain("old-file");
  await view.close();
  expect(allocate).not.toHaveBeenCalled();
});

it("an open preview retains its pending slot across a session change", async () => {
  const pending = Promise.withResolvers<GetResult<"/api/file">>();
  fixture.get.mockReturnValue(pending.promise);
  vi.spyOn(URL, "createObjectURL").mockReturnValue("blob:session");
  vi.spyOn(URL, "revokeObjectURL").mockImplementation(() => {});
  const view = await render(h(Previews));
  await act(async () => view.host.querySelectorAll("button")[0]!.click());
  await act(async () => view.host.querySelectorAll("button")[2]!.click());
  await act(async () => {
    pending.resolve({
      name: "retained-file",
      mime: "text/plain",
      base64: "YQ==",
    });
    await pending.promise;
  });
  expect(document.body.textContent).toContain("retained-file");
  expect(fixture.get).toHaveBeenCalledTimes(1);
  await view.close();
});

it("closing the final file view releases its settled bytes and reopening rereads", async () => {
  fixture.get.mockResolvedValue({
    name: "file",
    mime: "text/plain",
    base64: "YQ==",
  });
  vi.spyOn(URL, "createObjectURL").mockReturnValue("blob:release");
  vi.spyOn(URL, "revokeObjectURL").mockImplementation(() => {});
  const view = await render(h(Previews));
  await act(async () => view.host.querySelectorAll("button")[0]!.click());
  const slot = view.store.reads.file(
    { ...api, getStreamSession: streamSession },
    { asset: "same" },
  );
  expect(slot.current.status).toBe("live");
  await act(async () => view.host.querySelectorAll("button")[3]!.click());
  expect(slot.current.status).toBe("absent");
  await act(async () => view.host.querySelectorAll("button")[0]!.click());
  expect(fixture.get).toHaveBeenCalledTimes(2);
  await view.close();
});

it("a pending preview closes when its server owner changes", async () => {
  const pending = Promise.withResolvers<GetResult<"/api/file">>();
  fixture.get.mockReturnValue(pending.promise);
  const view = await render(h(Previews));
  await act(async () => view.host.querySelectorAll("button")[0]!.click());
  await view.switchOwner();
  await act(async () => {
    pending.resolve({
      name: "old-owner-file",
      mime: "text/plain",
      base64: "YQ==",
    });
    await pending.promise;
  });
  expect(document.body.textContent).not.toContain("old-owner-file");
  expect(document.querySelector('[role="dialog"]')).toBeNull();
  expect(fixture.get).toHaveBeenCalledTimes(1);
  await view.close();
});

it("a mounted preview clears its local display failure between opens", async () => {
  fixture.get.mockResolvedValue({
    name: "image.png",
    mime: "image/png",
    base64: "YQ==",
  });
  vi.spyOn(URL, "createObjectURL").mockReturnValue("blob:image");
  vi.spyOn(URL, "revokeObjectURL").mockImplementation(() => {});
  const view = await render(h(MountedPreview));
  await act(async () => view.host.querySelectorAll("button")[0]!.click());
  await act(async () =>
    document.querySelector("img")!.dispatchEvent(new Event("error")),
  );
  expect(document.querySelector('[role="alert"]')).not.toBeNull();
  await act(async () => view.host.querySelectorAll("button")[1]!.click());
  await act(async () => view.host.querySelectorAll("button")[0]!.click());
  expect(document.querySelector('[role="alert"]')).toBeNull();
  expect(document.querySelector("img")).not.toBeNull();
  await view.close();
});

const analyticsData = {
  pagination: { hasMore: true, total: 100, limit: 30, offset: 0 },
  detailPagination: {
    turns: { hasMore: true, total: 1, limit: 100, offset: 0 },
    rateLimits: { hasMore: true, total: 1, limit: 100, offset: 0 },
  },
  turns: [],
  rateLimits: [],
} satisfies GetResult<"/api/analytics">;
const buttonWithText = (label: string) =>
  Array.from(document.querySelectorAll<HTMLButtonElement>("button")).find(
    (button) => button.textContent?.trim() === label,
  )!;
it.each(["turns", "snapshots"])(
  "analytics shows a failed older %s page and clears its error after retry",
  async (detail) => {
    fixture.get
      .mockResolvedValueOnce(analyticsData)
      .mockRejectedValueOnce(new Error("detail failed"))
      .mockResolvedValueOnce({ turns: [], rateLimits: [] });
    const view = await render(h(AnalyticsView));
    await act(async () => buttonWithText("Open analytics").click());
    await act(async () => buttonWithText("Activity").click());
    await act(async () => buttonWithText(`Load 100 older ${detail}`).click());
    expect(document.querySelector('[role="alert"]')?.textContent).toContain(
      "detail failed",
    );
    await act(async () => buttonWithText(`Load 100 older ${detail}`).click());
    expect(document.querySelector('[role="alert"]')).toBeNull();
    expect(fixture.get).toHaveBeenCalledTimes(3);
    await view.close();
  },
);

it("analytics renews the time range on period and scope changes while pagination keeps it", async () => {
  vi.useFakeTimers({ toFake: ["Date"] });
  vi.setSystemTime(new Date("2040-01-01T00:00:00Z"));
  fixture.get.mockResolvedValue(analyticsData);
  const view = await render(h(AnalyticsView));
  await act(async () => buttonWithText("Open analytics").click());
  vi.setSystemTime(new Date("2040-01-01T00:01:00Z"));
  const period = document.querySelector<HTMLSelectElement>(
    '[aria-label="Analytics period"]',
  )!;
  await act(async () => {
    period.value = "3600";
    period.dispatchEvent(new Event("change", { bubbles: true }));
  });
  let now = Math.floor(Date.now() / 1000);
  expect(fixture.get).toHaveBeenLastCalledWith(
    "/api/analytics",
    expect.objectContaining({
      query: expect.objectContaining({ to_time: now, from_time: now - 3600 }),
    }),
  );
  vi.setSystemTime(new Date("2040-01-01T00:02:00Z"));
  const scope = document.querySelector<HTMLSelectElement>(
    '[aria-label="Analytics scope"]',
  )!;
  await act(async () => {
    scope.value = "team";
    scope.dispatchEvent(new Event("change", { bubbles: true }));
  });
  now = Math.floor(Date.now() / 1000);
  expect(fixture.get).toHaveBeenLastCalledWith(
    "/api/analytics",
    expect.objectContaining({
      query: expect.objectContaining({
        scope: "team",
        to_time: now,
        from_time: now - 3600,
      }),
    }),
  );
  await act(async () =>
    Array.from(document.querySelectorAll<HTMLButtonElement>('[role="tab"]'))
      .find((button) => button.textContent?.includes("Tool calls"))!
      .click(),
  );
  vi.setSystemTime(new Date("2040-01-01T00:03:00Z"));
  await act(async () => buttonWithText("Next").click());
  expect(fixture.get).toHaveBeenLastCalledWith(
    "/api/analytics",
    expect.objectContaining({
      query: expect.objectContaining({
        offset: 30,
        to_time: now,
        from_time: now - 3600,
      }),
    }),
  );
  await view.close();
});

const Skills = observer(function Skills({ agent }: { agent: string }) {
  const input = useRef<HTMLTextAreaElement>(null);
  const [skills] = useState(() => new SkillAutocomplete());
  const net = useServerNet();
  const read = net.reads.skills(
    { agent },
    {
      workspace: "workspace",
      account: "account",
      cwd: "/project",
      provider: "codex",
    },
  );
  const updateRange = () => skills.updateRange(true, read, input.current);
  return h(
    "div",
    null,
    h("textarea", { ref: input, onKeyUp: updateRange }),
    h("span", null, skills.matches.map((skill) => skill.name).join(",")),
  );
});
it("skill input shares one agent catalog and keeps another agent isolated", async () => {
  const pending = Promise.withResolvers<GetResult<"/api/skills">>();
  fixture.get.mockReturnValue(pending.promise);
  const view = await render(
    h(
      "div",
      null,
      h(Skills, { agent: "one" }),
      h(Skills, { agent: "one" }),
      h(Skills, { agent: "two" }),
    ),
  );
  expect(fixture.get).not.toHaveBeenCalled();
  await act(async () => {
    for (const input of view.host.querySelectorAll("textarea")) {
      input.value = "$";
      input.selectionStart = input.selectionEnd = 1;
      input.dispatchEvent(
        new KeyboardEvent("keyup", { key: "$", bubbles: true }),
      );
    }
  });
  expect(fixture.get).toHaveBeenCalledTimes(2);
  await act(async () => {
    pending.resolve({ skills: [], errors: [] });
    await pending.promise;
  });
  await act(async () =>
    view.host
      .querySelector("textarea")!
      .dispatchEvent(
        new KeyboardEvent("keyup", { key: "Shift", bubbles: true }),
      ),
  );
  expect(fixture.get).toHaveBeenCalledTimes(2);
  await view.close();
});

it("the parent opens the initial directory read and a delayed response preserves a typed path", async () => {
  const first = Promise.withResolvers<GetResult<"/api/directories">>();
  const second = Promise.withResolvers<GetResult<"/api/directories">>();
  fixture.get
    .mockReturnValueOnce(first.promise)
    .mockReturnValueOnce(second.promise);
  const choose = vi.fn();
  const view = await render(
    h(DirectoryField, { label: "Folder", value: "/a", onChange: choose }),
  );
  expect(fixture.get).not.toHaveBeenCalled();
  await act(async () =>
    view.host
      .querySelector<HTMLButtonElement>('[aria-label="Choose folder"]')!
      .click(),
  );
  expect(fixture.get).toHaveBeenCalledTimes(1);
  expect(document.body.textContent).not.toContain("No folders inside");
  const input = document.querySelector<HTMLInputElement>(
    ".directory-picker input",
  )!;
  await act(async () => {
    Object.getOwnPropertyDescriptor(
      HTMLInputElement.prototype,
      "value",
    )!.set!.call(input, "/b");
    input.dispatchEvent(new Event("input", { bubbles: true }));
  });
  await act(async () => {
    first.resolve({ path: "/a", parent: "/", directories: [] });
    await first.promise;
  });
  expect(input.value).toBe("/b");
  await act(async () =>
    document
      .querySelector(".directory-picker form")!
      .dispatchEvent(new Event("submit", { bubbles: true, cancelable: true })),
  );
  expect(fixture.get).toHaveBeenCalledTimes(2);
  await act(async () => {
    second.resolve({ path: "/b", parent: "/", directories: [] });
    await second.promise;
  });
  const select = Array.from(
    document.querySelectorAll<HTMLButtonElement>(".directory-picker button"),
  ).find((button) => button.textContent === "Use this folder")!;
  expect(select.disabled).toBe(false);
  await act(async () => select.click());
  expect(choose).toHaveBeenCalledWith("/b");
  await view.close();
});
