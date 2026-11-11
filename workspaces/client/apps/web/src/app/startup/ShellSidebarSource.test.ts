// @vitest-environment happy-dom
import { StrictMode, createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { autorun, configure, observable, runInAction } from "mobx";
import { observer } from "mobx-react-lite";
import { expect, it, vi } from "vitest";
import { ScreenOwner, ScreenSelection } from "./ScreenOwner.ts";
import { ShellSidebarSource } from "./ShellSidebarSource.ts";
import { createServerServices } from "../../servers/serverServices.ts";

it("supplies every backend on the sidebar's first render and follows service replacement without render writes", () => {
  window.matchMedia = vi.fn(() => ({
    matches: false,
    media: "",
    onchange: null,
    addListener() {},
    removeListener() {},
    addEventListener() {},
    removeEventListener() {},
    dispatchEvent: () => true,
  }));
  configure({ enforceActions: "observed" });
  const warnings = vi.spyOn(console, "warn").mockImplementation(() => {});
  const service = (id: string, credentialId: string) =>
    createServerServices({
      id,
      credentialId,
      origin: `https://${id}.test`,
      label: id,
    });
  const local = service("local", "first"),
    remote = service("remote", "first"),
    replacement = service("remote", "second");
  const services = observable.map(
    [
      ["local", local],
      ["remote", remote],
    ],
    { deep: false },
  );
  const source = new ShellSidebarSource(
    new ScreenSelection(["local", "remote"]),
    (id) => services.get(id),
  );
  const Sidebar = observer(() =>
    createElement("aside", null, [...source.backends.keys()].join(",")),
  );
  try {
    expect(
      renderToStaticMarkup(
        createElement(StrictMode, null, createElement(Sidebar)),
      ),
    ).toBe("<aside>local,remote</aside>");
    expect(source.backends.get("remote")?.storage).toBe(remote.storage);
    runInAction(() => services.set("remote", replacement));
    expect(source.backends.get("remote")?.storage).toBe(replacement.storage);
    expect(
      warnings.mock.calls.filter((args) =>
        String(args[0]).toLowerCase().includes("[mobx]"),
      ),
    ).toEqual([]);
  } finally {
    warnings.mockRestore();
    local.close();
    remote.close();
    replacement.close();
  }
});

it("invalidates an already observed screen activation when its connection is committed", () => {
  const selection = new ScreenSelection(["local", "remote"]);
  const screen = new ScreenOwner();
  const active: boolean[] = [];
  const stop = autorun(() => active.push(screen.active));
  try {
    screen.connect(null, () => selection.current === "remote");
    expect(active).toEqual([true, false]);
    selection.select("remote");
    expect(active).toEqual([true, false, true]);
  } finally {
    stop();
  }
});

it("updates the selector in its lifetime and keeps model reads pure", () => {
  const source = new ShellSidebarSource(
    new ScreenSelection([]),
    () => undefined,
  );
  const select = vi.spyOn(source, "select");
  const stop = source.start();
  const first = source.model;
  select.mockClear();
  expect(source.model).toBe(first);
  expect(select).not.toHaveBeenCalled();
  source.selection.updateServers(["unavailable"]);
  expect(select).toHaveBeenCalledOnce();
  stop();
  select.mockClear();
  source.selection.updateServers([]);
  expect(select).not.toHaveBeenCalled();
});
