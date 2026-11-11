// @vitest-environment happy-dom
import { act, createElement, StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { action, autorun, getDependencyTree } from "mobx";
import { observer } from "mobx-react-lite";
import { beforeEach, expect, it, vi } from "vitest";

const memory = vi.hoisted(() => ({
  rows: new Map<string, unknown>(),
  read: vi.fn<(key: string) => Promise<unknown>>(),
  write: vi.fn<(key: string, value: unknown) => Promise<void>>(),
  server: "server",
  entries: vi.fn<() => Promise<[string, unknown][]>>(),
}));
vi.mock("../app/ServerAreasProvider.tsx", () => ({
  useServerAreas: () => ({ unsentText: owner }),
}));
vi.mock("../indexedDb.ts", () => ({
  indexedDb: () => ({
    get: memory.read,
    entries: memory.entries,
    delete: async (key: string) => {
      memory.rows.delete(key);
    },
    put: memory.write,
    update: async (key: string, change: (value: unknown) => unknown) => {
      const next = change(memory.rows.get(key));
      if (next === undefined) memory.rows.delete(key);
      else memory.rows.set(key, next);
    },
  }),
}));
import {
  flushUnsentText,
  unsentKey,
  unsentStore,
  UNSENT_FAILURE_EVENT,
} from "./store.ts";
import { UnsentText, useUnsentText } from "./useUnsentText.ts";
import { UNSENT_FORMAT_VERSION } from "./schema.ts";

let owner: UnsentText;
beforeEach(() => {
  vi.stubGlobal("IS_REACT_ACT_ENVIRONMENT", true);
  memory.rows.clear();
  memory.entries.mockReset().mockImplementation(async () => [...memory.rows]);
  memory.server = "server";
  memory.read
    .mockReset()
    .mockImplementation(async (key) => memory.rows.get(key));
  memory.write.mockReset().mockImplementation(async (key, value) => {
    memory.rows.set(key, value);
  });
});
async function fixture(id = "chat") {
  owner = new UnsentText(memory.server);
  let server = memory.server;
  let current!: ReturnType<typeof useUnsentText>;
  const Probe = observer(function Probe({ id }: { id: string }) {
    current = useUnsentText("reply", id);
    return createElement("output", null, current[0] ?? "empty");
  });
  const host = document.createElement("div"),
    root = createRoot(host);
  const render = async (id: string) => {
    if (server !== memory.server) {
      server = memory.server;
      owner = new UnsentText(server);
    }
    return act(async () =>
      root.render(
        createElement(
          StrictMode,
          null,
          createElement(Probe, { id, key: server }),
        ),
      ),
    );
  };
  await render(id);
  return {
    host,
    render,
    get current() {
      return current;
    },
    hide: () => act(async () => root.render(null)),
    close: () => act(async () => root.unmount()),
  };
}
it("restores through the observer hook and ignores a late read after the user edits", async () => {
  await unsentStore("server", "reply").put("chat", "Saved draft");
  const f = await fixture();
  try {
    expect(f.host.textContent).toBe("Saved draft");
    const old = Promise.withResolvers<unknown>();
    memory.read.mockReturnValueOnce(old.promise);
    await f.render("other");
    expect(f.host.textContent).toBe("empty");
    await act(async () => f.current[1]("Typed draft"));
    await act(async () =>
      old.resolve({
        version: UNSENT_FORMAT_VERSION,
        kind: "text",
        value: "Old draft",
      }),
    );
    expect(f.host.textContent).toBe("Typed draft");
  } finally {
    await f.close();
  }
});
it("isolates server and subject changes and disposes stale restores", async () => {
  const delayed = Promise.withResolvers<unknown>();
  memory.read.mockReturnValueOnce(delayed.promise);
  const f = await fixture();
  try {
    memory.server = "other-server";
    memory.rows.set(unsentKey("other-server", "reply", "chat"), {
      version: UNSENT_FORMAT_VERSION,
      kind: "text",
      value: "Other server",
    });
    await f.render("chat");
    await act(async () =>
      delayed.resolve({
        version: UNSENT_FORMAT_VERSION,
        kind: "text",
        value: "Wrong server",
      }),
    );
    expect(f.host.textContent).toBe("Other server");
  } finally {
    await f.close();
  }
});
it("deletes only the submitted draft and keeps a newer saved edit", async () => {
  const f = await fixture();
  try {
    await act(async () => f.current[1]("Submitted"));
    await unsentStore("server", "reply").put("chat", "Newer edit");
    await act(async () => f.current[2]("Submitted"));
    expect(await unsentStore("server", "reply").get("chat")).toBe("Newer edit");
    await act(async () => f.current[1](null));
    expect(f.host.textContent).toBe("empty");
  } finally {
    await f.close();
  }
});
it("keeps pending writes in the existing close barrier after the hook unmounts", async () => {
  const pending = Promise.withResolvers<void>();
  memory.write.mockReturnValue(pending.promise);
  const f = await fixture();
  await act(async () => f.current[1]("Draft"));
  await f.close();
  let closed = false;
  const flushing = flushUnsentText().then(() => {
    closed = true;
  });
  await Promise.resolve();
  expect(closed).toBe(false);
  pending.resolve();
  await flushing;
  expect(closed).toBe(true);
});
it("reports failed persistence and leaves the close barrier refusal intact", async () => {
  const failure = vi.fn();
  window.addEventListener(UNSENT_FAILURE_EVENT, failure);
  const f = await fixture();
  try {
    memory.write.mockRejectedValueOnce(new Error("Storage full"));
    await act(async () => f.current[1]("Keep this visible"));
    expect(f.host.textContent).toBe("Keep this visible");
    expect(failure).toHaveBeenCalled();
    await expect(flushUnsentText()).rejects.toThrow("draft is not saved");
    expect(memory.rows.has(unsentKey("server", "reply", "chat"))).toBe(false);
  } finally {
    window.removeEventListener(UNSENT_FAILURE_EVENT, failure);
    await f.close();
  }
});

it.each([false, true])(
  "reopening the same owner's editor clears submitted text and preserves a newer edit (%s)",
  async (newer) => {
    const f = await fixture();
    try {
      await act(async () => f.current[1]("Submitted"));
      if (newer) await act(async () => f.current[1]("Newer"));
      await act(async () => f.current[2]("Submitted"));
      await f.hide();
      await f.render("chat");
      expect(f.host.textContent).toBe(newer ? "Newer" : "empty");
    } finally {
      await f.close();
    }
  },
);

it.each(["update", "delete"])(
  "reopening reconciles an external persisted %s with the same owner",
  async (change) => {
    const f = await fixture();
    const stopOther = autorun(() => owner.read("reply", "another-editor"));
    try {
      await act(async () => f.current[1]("Draft A"));
      await f.hide();
      if (change === "update")
        await unsentStore("server", "reply").put("chat", "External draft");
      else await unsentStore("server", "reply").delete("chat");
      await f.render("chat");
      expect(f.host.textContent).toBe(
        change === "update" ? "External draft" : "empty",
      );
    } finally {
      stopOther();
      await f.close();
    }
  },
);

it("evicts the view and cached value when an editor abandons observation in one tick", async () => {
  owner = new UnsentText("server");
  const key = owner.key("reply", "abandoned");
  action(() => owner.values.set(key, "cached"))();
  const first = autorun(() => owner.read("reply", "abandoned"));
  const original = getDependencyTree(first).dependencies?.[0]?.name;
  expect(original).toBeDefined();
  first();
  await Promise.resolve();
  expect(memory.read).not.toHaveBeenCalled();
  expect(owner.values.has(key)).toBe(false);
  const second = autorun(() => owner.read("reply", "abandoned"));
  const replacement = getDependencyTree(second).dependencies?.[0]?.name;
  second();
  await Promise.resolve();
  expect(replacement).toBeDefined();
  expect(replacement).not.toBe(original);
});
