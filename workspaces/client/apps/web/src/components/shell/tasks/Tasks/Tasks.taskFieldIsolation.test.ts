// @vitest-environment happy-dom
import { act, createElement, type ReactNode } from "react";
import { createRoot } from "react-dom/client";
import { MantineProvider } from "@mantine/core";
import { afterEach, expect, it, vi } from "vitest";
import {
  createWorkMirror,
  createAgentDetailMirror,
} from "../../../../generated/observableResources.ts";
import type { WorkSubscriptionData } from "../../../../generated/observableResources.ts";
const h = vi.hoisted(() => ({
  store: {},
  storage: new Storage(),
  post: vi.fn(),
  renders: new Map<string, number>(),
  buttons: new Map<string, number>(),
}));
vi.mock(
  "../../../servers/ServerServicesProvider/ServerServicesProvider.tsx",
  () => ({
    useServerStore: () => h.store,
    useServerApi: () => ({ post: h.post }),
    useServerStorage: () => h.storage,
  }),
);
vi.mock("@mantine/core", async (original) => {
  const m = await original<typeof import("@mantine/core")>();
  return {
    ...m,
    Button: (props: Parameters<typeof m.Button<"button">>[0]) => {
      if (typeof props.children === "string")
        h.buttons.set(props.children, (h.buttons.get(props.children) || 0) + 1);
      return createElement(m.Button<"button">, props);
    },
  };
});
vi.mock("mobx-react-lite", async (original) => {
  const m = await original<typeof import("mobx-react-lite")>();
  return {
    ...m,
    observer: (View: (props: { task?: { id: string } }) => ReactNode) =>
      m.observer(function Counted(props: { task?: { id: string } }) {
        if (props.task) {
          const key = `${props.task.id}:${View.name}`;
          h.renders.set(key, (h.renders.get(key) || 0) + 1);
        }
        return View(props);
      }),
  };
});
import Tasks from "./Tasks.tsx";
import { ApiError } from "../../../../api.ts";
import { WorkStatus } from "../../../../generated/api.ts";
const item = (id: string): WorkSubscriptionData["items"][number] => ({
  id,
  rootId: "lead",
  title: id,
  status: WorkStatus.review,
  created: 1,
  updated: 1,
  version: 1,
  policy: {
    blockedBy: [],
    displayStatus: WorkStatus.review,
    acceptAvailable: true,
    gateOverrideRequired: true,
  },
});
afterEach(() => {
  h.storage.clear();
  vi.unstubAllGlobals();
  h.post.mockReset();
  h.renders.clear();
  h.buttons.clear();
});
it("keeps versioned decision drafts and unchanged task leaves, and retries a lost decision exactly", async () => {
  vi.stubGlobal("IS_REACT_ACT_ENVIRONMENT", true);
  Object.defineProperty(document, "fonts", {
    configurable: true,
    value: new EventTarget(),
  });
  const items = [item("A"), item("B")],
    dto = { rootId: "lead", items, tasks: items };
  const work = createWorkMirror(dto),
    detail = createAgentDetailMirror({ id: "lead" });
  h.store = {
    work: () => ({ current: { status: "live", data: work.current } }),
    agentDetail: () => ({ current: { status: "live", data: detail.current } }),
  };
  const host = document.createElement("div"),
    root = createRoot(host);
  document.body.append(host);
  const click = async (name: string) =>
    act(async () => {
      const button = [...host.querySelectorAll("button")].find(
        (node) => node.textContent === name,
      )!;
      expect(button).toBeTruthy();
      button.click();
    });
  const fill = async (label: string, value: string) =>
    act(async () => {
      const input = [...host.querySelectorAll("textarea")].find((node) =>
        host
          .querySelector(`label[for="${node.id}"]`)
          ?.textContent?.startsWith(label),
      )!;
      Object.getOwnPropertyDescriptor(
        HTMLTextAreaElement.prototype,
        "value",
      )!.set!.call(input, value);
      input.dispatchEvent(new Event("input", { bubbles: true }));
    });
  try {
    await act(async () =>
      root.render(
        createElement(
          MantineProvider,
          {},
          createElement(Tasks, { agentId: "lead" }),
        ),
      ),
    );
    await act(async () =>
      host
        .querySelector<HTMLButtonElement>(".task-review-list button")!
        .click(),
    );
    await fill("Decision reason", "Original approval");
    await fill("Gate override reason", "Original override");
    const before = new Map(h.renders),
      buttonsBefore = new Map(h.buttons);
    await act(async () =>
      work.apply({ ...dto, items: [{ ...items[0]!, version: 2 }, items[1]!] }),
    );
    expect(
      [...host.querySelectorAll("textarea")].map((node) => node.value),
    ).toContain("Original approval");
    expect(
      [...host.querySelectorAll("textarea")].map((node) => node.value),
    ).toContain("Original override");
    expect(
      [...host.querySelectorAll("dd")].map((node) => node.textContent),
    ).toContain("2");
    const expected = new Map(before);
    expect(before.has("A:TaskFacts")).toBe(true);
    expected.set("A:TaskFacts", before.get("A:TaskFacts")! + 1);
    expect(h.renders).toEqual(expected);
    for (const label of ["Update task", "Accept with override", "Reject task"])
      expect(
        h.buttons.get(label),
        `${label} rebuilt for a version-only update`,
      ).toBe(buttonsBefore.get(label));
    h.post.mockRejectedValueOnce(new Error("reply lost")).mockResolvedValue({});
    await click("Accept with override");
    await act(async () =>
      work.apply({
        ...dto,
        items: [
          {
            ...items[0]!,
            version: 3,
            policy: { ...items[0]!.policy, acceptAvailable: false },
          },
          items[1]!,
        ],
      }),
    );
    await fill("Decision reason", "Changed approval");
    await fill("Gate override reason", "Changed override");
    await click("Accept with override");
    expect(h.post.mock.calls).toHaveLength(2);
    expect(h.post.mock.calls[1]).toEqual(h.post.mock.calls[0]);
    expect(h.post.mock.calls[0]?.[1]).toMatchObject({
      action: "accept",
      task_id: "A",
      version: 2,
      result: "Original approval",
      override_reason: "Original override",
      id: expect.any(String),
    });
    await act(async () =>
      work.apply({ ...dto, items: [{ ...items[0]!, version: 4 }, items[1]!] }),
    );
    h.post.mockRejectedValueOnce(
      new ApiError("stale version", 409, { error: "stale version" }),
    );
    await click("Accept with override");
    expect(h.storage.length).toBe(0);
    await fill("Decision reason", "Revised approval");
    await act(async () =>
      work.apply({ ...dto, items: [{ ...items[0]!, version: 5 }, items[1]!] }),
    );
    for (const name of ["Accept with override", "Reject task"])
      expect(
        [...host.querySelectorAll("button")].find(
          (button) => button.textContent === name,
        )?.disabled,
      ).toBe(false);
    await click("Accept with override");
    expect(h.post.mock.calls[3]?.[1]).toMatchObject({
      version: 5,
      result: "Revised approval",
    });
    expect(h.post.mock.calls[3]?.[1].id).not.toBe(h.post.mock.calls[2]?.[1].id);
    await click("Create task");
    const field = (label: string) =>
      [
        ...host.querySelectorAll<HTMLInputElement | HTMLTextAreaElement>(
          "input, textarea",
        ),
      ].find((node) =>
        host
          .querySelector(`label[for="${node.id}"]`)
          ?.textContent?.startsWith(label),
      )!;
    for (const [label, value] of [
      ["Title", " New draft "],
      ["Description", "Draft description"],
      ["Owner agent ID", " worker "],
      ["Result schema", "false"],
    ]) {
      await act(async () => {
        const input = field(label!),
          prototype =
            input instanceof HTMLInputElement
              ? HTMLInputElement.prototype
              : HTMLTextAreaElement.prototype;
        Object.getOwnPropertyDescriptor(prototype, "value")!.set!.call(
          input,
          value,
        );
        input.dispatchEvent(new Event("input", { bubbles: true }));
      });
    }
    await act(async () => {
      const dependencies = host.querySelector<HTMLSelectElement>(
        'select[aria-label="Dependencies"]',
      )!;
      for (const option of dependencies.options)
        option.selected = option.value === "B";
      dependencies.dispatchEvent(new Event("change", { bubbles: true }));
    });
    await click("Save task");
    expect(h.post.mock.calls.at(-1)?.[1]).toMatchObject({
      action: "create",
      agent: "lead",
      title: "New draft",
      description: "Draft description",
      owner: "worker",
      dependencies: ["B"],
      result_schema: false,
    });
  } finally {
    await act(async () => root.unmount());
    host.remove();
  }
});
