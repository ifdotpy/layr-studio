import { transientTestFixture } from "../sync/subscriptions/transientTestFixture.ts";
import { required } from "../components/conversation/ComposerAttachments/componentTestFixtures.ts";
/// <reference lib="es2024.promise" />
import { afterEach, expect, test, vi } from "vitest";
import { validateServerName, watchServerName } from "./serverNames.ts";
import { readServers, writeServers, SERVER_REGISTRY_KEY } from "./registry.ts";
import { setServerCredentialAdapter } from "./transport.ts";

test("server names accept visible Unicode and reject controls and outer spaces", () => {
  for (const name of [
    "Lumina Mac",
    "Igor MBP",
    "Kukuka WSL",
    "Kukuka Windows",
    "Écran",
    "💻".repeat(80),
  ])
    expect(() => validateServerName(name)).not.toThrow();
  for (const name of [
    "",
    " ",
    " outer",
    "outer ",
    "a".repeat(81),
    "💻".repeat(81),
    "line\nname",
    "hidden\u202ename",
    "tab\tname",
    "nul\0name",
    "space\u00a0name",
  ])
    expect(() => validateServerName(name)).toThrow();
});

afterEach(() => {
  writeServers([], {
    length: 0,
    clear() {},
    getItem: () => null,
    key: () => null,
    removeItem() {},
    setItem() {},
  });
  vi.useRealTimers();
  vi.unstubAllGlobals();
});

test("saved server names use their own stream with zero idle status reads", async () => {
  vi.useFakeTimers();
  const server = {
    id: "remote",
    label: "Old",
    origin: "https://remote.tailnet.ts.net",
    credentialId: "credential",
  };
  const values = new Map([[SERVER_REGISTRY_KEY, JSON.stringify([server])]]);
  vi.stubGlobal("localStorage", {
    getItem: (key: string) => values.get(key) ?? null,
    setItem: (key: string, value: string) => values.set(key, value),
  });
  vi.stubGlobal("window", new EventTarget());
  const fetch = vi.fn();
  setServerCredentialAdapter({
    fetch,
    forget: async () => {},
    hasPairAttempt: async () => false,
    pair: async () => server,
  });
  const f = transientTestFixture();
  const stop = watchServerName(server, f.store);
  await vi.advanceTimersByTimeAsync(100);
  expect(f.transport.watchResourceChanges).toHaveBeenCalledWith(
    { kind: "multi-server" },
    expect.any(Function),
    expect.anything(),
  );
  await vi.advanceTimersByTimeAsync(5 * 60_000);
  expect(fetch).not.toHaveBeenCalled();
  const deliver = (id: string, label: string) =>
    f.emit({ kind: "multi-server" }, {
      epoch: "owner",
      revision: 1,
      discoverySnapshot: { identity: { serverId: id, label } },
    } as import("../sync/resourceEvents.ts").ResourceVersion);
  deliver("other", "Wrong");
  await vi.advanceTimersByTimeAsync(0);
  expect(required(readServers()[0]).label).toBe("remote.tailnet.ts.net");
  deliver("remote", "New");
  await vi.advanceTimersByTimeAsync(0);
  expect(required(readServers()[0]).label).toBe("New");
  // An old stream cannot restore a forgotten peer or overwrite a replacement.
  values.set(
    SERVER_REGISTRY_KEY,
    JSON.stringify([
      { ...server, label: "Replacement", credentialId: "replacement" },
    ]),
  );
  deliver("remote", "Stale");
  await vi.advanceTimersByTimeAsync(0);
  expect(required(readServers()[0]).label).toBe("remote.tailnet.ts.net");
  values.set(SERVER_REGISTRY_KEY, "[]");
  deliver("remote", "Removed");
  await vi.advanceTimersByTimeAsync(0);
  expect(readServers()).toEqual([]);
  stop();
  await vi.advanceTimersByTimeAsync(0);
  values.set(SERVER_REGISTRY_KEY, JSON.stringify([server]));
  deliver("remote", "Stopped");
  expect(required(readServers()[0]).label).toBe("remote.tailnet.ts.net");
  expect(fetch).not.toHaveBeenCalled();
  f.close();
});
