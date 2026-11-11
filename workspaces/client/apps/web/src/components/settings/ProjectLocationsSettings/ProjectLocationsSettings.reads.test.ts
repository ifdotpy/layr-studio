// @vitest-environment happy-dom
import { act, createElement } from "react";
import { createRoot } from "react-dom/client";
import { MantineProvider } from "@mantine/core";
import { expect, it, vi } from "vitest";
import { createServerServices } from "../../../servers/serverServices.ts";
import { localServer } from "../../../servers/registry.ts";
import { ServerServicesProvider } from "../../servers/ServerServicesProvider/ServerServicesProvider.tsx";
import type { LocationProjectView } from "../../../servers/projectLocations.ts";
import ProjectLocationsSettings from "./ProjectLocationsSettings.tsx";

function mountedLocations() {
  vi.stubGlobal("IS_REACT_ACT_ENVIRONMENT", true);
  const originalWarn = console.warn;
  const warning = vi
    .spyOn(console, "warn")
    .mockImplementation((message) => originalWarn(message));
  const services = createServerServices(localServer());
  const get = vi.spyOn(services.api, "get");
  const post = vi.spyOn(services.api, "post");
  vi.spyOn(services.api, "refreshSession").mockResolvedValue({
    token: "fixture",
  });
  const host = document.createElement("div"),
    root = createRoot(host);
  document.body.append(host);
  const render = (project: LocationProjectView) =>
    act(async () =>
      root.render(
        createElement(
          MantineProvider,
          null,
          createElement(ServerServicesProvider, {
            services,
            children: createElement(ProjectLocationsSettings, { project }),
          }),
        ),
      ),
    );
  const menu = async () => {
    const button = host.querySelector<HTMLButtonElement>(
      ".project-location-actions",
    );
    expect(button).not.toBeNull();
    await act(async () => button?.click());
  };
  return {
    get,
    post,
    host,
    render,
    menu,
    async close() {
      await act(async () => root.unmount());
      services.close();
      host.remove();
      warning.mockRestore();
      vi.unstubAllGlobals();
    },
  };
}
const location = { serverId: "local", path: "/one", projectId: "project" };
const initial: LocationProjectView = {
  id: "project",
  name: "Project",
  path: "/one",
  locations: [location],
};

it("location menu reads once and location changes do not refetch folder info", async () => {
  const view = mountedLocations();
  view.get.mockResolvedValue({ gitHead: "initial-head" });
  try {
    await view.render(initial);
    expect(view.get).not.toHaveBeenCalled();
    await view.menu();
    expect(view.get).toHaveBeenCalledTimes(1);
    const requested = [...view.get.mock.calls];
    for (const locations of [
      [{ ...location }],
      [{ ...location, path: "/two" }],
      // Keep /two selected: returning to an evicted /one read may load it again.
      [
        { ...location, path: "/two" },
        { ...location, serverId: "remote" },
      ],
    ]) {
      await view.render({ ...initial, locations });
      expect(view.get.mock.calls).toEqual(requested);
      if (locations[0]?.path === "/two")
        expect(view.host.textContent).not.toContain("initial-head");
    }
  } finally {
    await view.close();
  }
});

it("overlapping location HEAD actions keep their requested paths isolated", async () => {
  const view = mountedLocations();
  const one = Promise.withResolvers<{ gitHead: string }>(),
    two = Promise.withResolvers<{ gitHead: string }>();
  view.get.mockReturnValueOnce(one.promise).mockReturnValueOnce(two.promise);
  try {
    await view.render(initial);
    await view.menu();
    await view.menu();
    await view.render({
      ...initial,
      locations: [{ ...location, path: "/two" }],
    });
    await view.menu();
    expect(view.get).toHaveBeenCalledTimes(2);
    expect(view.get).toHaveBeenNthCalledWith(
      2,
      "/api/project-locations",
      expect.objectContaining({
        query: expect.objectContaining({ path: "/two" }),
      }),
    );
    await act(async () => two.resolve({ gitHead: "head-two" }));
    await act(async () => one.resolve({ gitHead: "head-one" }));
    expect(view.host.textContent).toContain("head-two");
    expect(view.host.textContent).not.toContain("head-one");
  } finally {
    await view.close();
  }
});

it("retries a location mutation with the same request identity after an uncertain failure", async () => {
  const view = mountedLocations();
  view.get.mockResolvedValue({ gitHead: "head" });
  view.post.mockRejectedValue(new Error("offline"));
  try {
    await view.render({
      ...initial,
      locations: [location, { ...location, serverId: "remote" }],
    });
    for (let attempt = 0; attempt < 2; attempt++) {
      await view.menu();
      const remove = [
        ...view.host.querySelectorAll<HTMLElement>("[role=menuitem]"),
      ].find((item) => item.textContent === "Remove");
      expect(remove).toBeDefined();
      await act(async () => remove?.click());
    }
    expect(view.post).toHaveBeenCalledTimes(2);
    const first = view.post.mock.calls[0]?.[1],
      second = view.post.mock.calls[1]?.[1];
    expect(first).toEqual(
      expect.objectContaining({
        action: "remove_location",
        project: "project",
        server: "local",
        request_id: expect.any(String),
      }),
    );
    expect(second).toEqual(first);
  } finally {
    await view.close();
  }
});
