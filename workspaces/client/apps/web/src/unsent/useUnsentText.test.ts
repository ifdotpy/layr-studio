// @vitest-environment happy-dom
import { act, createElement, StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { observer } from "mobx-react-lite";
import { beforeEach, expect, it, vi } from "vitest";

const memory = vi.hoisted(() => ({
  rows: new Map<string, unknown>(),
  read: vi.fn<(key: string) => Promise<unknown>>(),
  write: vi.fn<(key: string, value: unknown) => Promise<void>>(),
  server: "server",
}));
vi.mock(
  "../components/servers/ServerServicesProvider/ServerServicesProvider.tsx",
  () => ({
    useServerServices: () => ({ owner: { id: memory.server } }),
  }),
);
vi.mock("../indexedDb.ts", () => ({
  indexedDb: () => ({
    get: memory.read,
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
import { useUnsentText } from "./useUnsentText.ts";
import { UNSENT_FORMAT_VERSION } from "./schema.ts";

beforeEach(() => {
  vi.stubGlobal("IS_REACT_ACT_ENVIRONMENT", true);
  memory.rows.clear();
  memory.server = "server";
  memory.read
    .mockReset()
    .mockImplementation(async (key) => memory.rows.get(key));
  memory.write.mockReset().mockImplementation(async (key, value) => {
    memory.rows.set(key, value);
  });
});
async function fixture(id = "chat") {
  let current!: ReturnType<typeof useUnsentText>;
  const Probe = observer(function Probe({ id }: { id: string }) {
    current = useUnsentText("reply", id);
    return createElement("output", null, current[0] ?? "empty");
  });
  const host = document.createElement("div"),
    root = createRoot(host);
  const render = async (id: string) =>
    act(async () =>
      root.render(
        createElement(StrictMode, null, createElement(Probe, { id })),
      ),
    );
  await render(id);
  return {
    host,
    render,
    get current() {
      return current;
    },
    close: () => act(async () => root.unmount()),
  };
}
it("restores through the observer hook and ignores a late read after the user edits", async () => {
  await unsentStore("server", "reply").put("chat", "Saved draft");
  const f = await fixture();
  try {
    expect(f.host.textContent).toBe("Saved draft");
    const old = Promise.withResolvers<unknown>();
    memory.read.mockReturnValue(old.promise);
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
  memory.read.mockReturnValue(delayed.promise);
  const f = await fixture();
  try {
    memory.server = "other-server";
    memory.read.mockResolvedValue({
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
    await f.current[2]("Submitted");
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
