// @vitest-environment happy-dom
import { ProjectServerSelection } from "../../../app/projects/CreationOwner.ts";
import { createServerServices } from "../../../servers/serverServices.ts";
import { autorun } from "mobx";
import { beforeEach, expect, it, vi } from "vitest";
import { ProjectServerRead } from "./ProjectServerRead.ts";

const read = vi.hoisted(() =>
  vi.fn<
    (typeof import("../../../app/projects/newChatSettings.ts"))["readNewChatResource"]
  >(),
);
vi.mock("../../../app/projects/newChatSettings.ts", () => ({
  readNewChatResource: read,
}));
beforeEach(() => {
  read.mockReset();
});

it("loads once for concurrent observers and aborts when the last one leaves", async () => {
  read.mockImplementation(
    (_connection, _path, options) =>
      new Promise((_resolve, reject) => {
        options?.signal?.addEventListener("abort", () =>
          reject(new Error("aborted")),
        );
      }),
  );
  const view = new ProjectServerRead();
  expect(read).not.toHaveBeenCalled();
  const first = autorun(() => view.value);
  const second = autorun(() => view.value);
  await Promise.resolve();
  expect(read).toHaveBeenCalledTimes(1);
  const signal = read.mock.calls[0]?.[2]?.signal;
  first();
  expect(signal?.aborted).toBe(false);
  second();
  await Promise.resolve();
  expect(signal?.aborted).toBe(true);
  await Promise.resolve();
  expect(view.error).toBe("");
});

it("publishes a capability failure through the observed store", async () => {
  read.mockRejectedValue(new Error("offline"));
  const view = new ProjectServerRead();
  const stop = autorun(() => view.value);
  await Promise.resolve();
  await Promise.resolve();
  expect(view.error).toContain("offline");
  stop();
});

it("keeps the selected server's services until selection changes and closes only foreign services", async () => {
  const first = { id: "first", label: "First", origin: "https://first.test" };
  const second = {
    id: "second",
    label: "Second",
    origin: "https://second.test",
  };
  const inherited = createServerServices(first);
  const foreign = createServerServices(second);
  const inheritedClose = vi.spyOn(inherited, "close");
  const foreignClose = vi.spyOn(foreign, "close");
  const create = vi.fn(() => foreign);
  const owner = new ProjectServerSelection(
    {
      id: "p",
      name: "Project",
      path: "/p",
      locations: [first, second].map((row) => ({
        serverId: row.id,
        projectId: "p",
        path: "/p",
      })),
    },
    () =>
      [first, second].map((connection) => ({
        id: connection.id,
        label: connection.label,
        connection,
      })),
    "first",
    (id) => (id === "first" ? inherited : undefined),
    create,
  );
  expect(owner.services).toBe(inherited);
  const original = owner.read;
  owner.select("first");
  expect(owner.read).toBe(original);
  owner.select("second");
  expect(owner.services).toBe(foreign);
  expect(owner.read).not.toBe(original);
  owner.dispose();
  expect(foreignClose).toHaveBeenCalledTimes(1);
  expect(inheritedClose).not.toHaveBeenCalled();
  inherited.close();
});
