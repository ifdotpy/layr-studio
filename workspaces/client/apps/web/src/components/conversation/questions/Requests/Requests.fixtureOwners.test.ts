// @vitest-environment happy-dom
import { act, createElement } from "react";
import { createRoot } from "react-dom/client";
import { MantineProvider } from "@mantine/core";
import type { Root } from "react-dom/client";
import { afterEach, expect, it, vi } from "vitest";
import type { RequestAreaOwner } from "../RequestArea/RequestArea.tsx";
import { requestTestEnvironment } from "./requestTestFixture.ts";
const capture = vi.hoisted(() => ({
  owners: [] as RequestAreaOwner[],
  roots: [] as Root[],
  cleanups: [] as (() => void)[],
}));
vi.mock("react-dom/client", async (original) => {
  const actual = await original<typeof import("react-dom/client")>();
  return {
    ...actual,
    createRoot: (...args: Parameters<typeof actual.createRoot>) => {
      const root = actual.createRoot(...args);
      capture.roots.push(root);
      return root;
    },
  };
});
vi.mock("../RequestArea/RequestArea.tsx", async (original) => {
  const actual =
    await original<typeof import("../RequestArea/RequestArea.tsx")>();
  return {
    ...actual,
    RequestArea: (props: Parameters<typeof actual.RequestArea>[0]) => {
      capture.owners.push(props.owner);
      return actual.RequestArea(props);
    },
  };
});
vi.mock(
  "../../../projects/ProjectDirectoryPicker/ProjectDirectoryPicker.tsx",
  () => ({ default: () => null }),
);
requestTestEnvironment();
afterEach(async () => {
  await act(async () =>
    capture.roots.splice(0).forEach((root) => root.unmount()),
  );
  window.dispatchEvent(new Event("pagehide"));
  capture.owners.splice(0).forEach((owner) => owner.dispose());
  capture.cleanups.splice(0).forEach((dispose) => dispose());
});
const callers = [
  {
    name: "semantic audit stories",
    load: async () => {
      const story =
        await import("../../../../design/SemanticAudit.stories.tsx");
      const { composeStories } = await import("@storybook/react-vite");
      capture.cleanups.push(story.default.beforeEach());
      const { PrimaryLight } = composeStories(story);
      createRoot(document.getElementById("root")!).render(
        createElement(MantineProvider, null, createElement(PrimaryLight)),
      );
    },
  },
  {
    name: "request stories",
    load: async () => {
      const story = await import("./Requests.stories.tsx");
      capture.cleanups.push(
        story.default.beforeEach({ args: story.default.args }),
      );
      createRoot(document.getElementById("root")!).render(
        createElement(
          MantineProvider,
          null,
          createElement(story.default.component, story.default.args),
        ),
      );
    },
  },
  {
    name: "browser-open approvals",
    load: () =>
      import("../../../../../tests/questions/browser-open-approval-ui.fixture.tsx"),
  },
  {
    name: "phone sign-in",
    load: () =>
      import("../../../../../tests/questions/sign-in-card-phone-ui.fixture.tsx"),
  },
  {
    name: "stdin approval",
    load: () =>
      import("../../../../../tests/terminal/stdin-approval-fixture.tsx"),
  },
  {
    name: "inline request identity",
    load: () =>
      import("../../../../../tests/questions/question-card-identity-ui.fixture.tsx"),
  },
  {
    name: "workspace dialog drafts",
    load: () =>
      import("../../../../../tests/drafts/dialog-drafts-ui.fixture.tsx"),
  },
  {
    name: "structured request errors",
    load: async () => {
      await import("../../../../../tests/errors/structured-operational-errors-ui.fixture.tsx");
      await window.renderCase("request", "Fixture error");
    },
  },
];
it.each(callers)(
  "$name renders and acts through its explicit request owner",
  async ({ load }) => {
    document.body.innerHTML = '<div id="root"></div>';
    await act(async () => {
      await load();
    });
    const card = document.querySelector<HTMLElement>("[data-request]");
    expect(card).not.toBeNull();
    const id = card!.dataset.request!;
    const owner = capture.owners.find((owner) => owner.views.has(id));
    expect(owner).toBeDefined();
    const view = owner!.views.get(id)!;
    expect(view.mounted).toBe(true);
    await act(async () => view.toggle());
    expect(view.open).toBe(true);
    if (view.question)
      expect(document.querySelector(".request-answer-form")).not.toBeNull();
    await act(async () => view.close());
    expect(owner!.views.get(id)).toBe(view);
    expect(view.open).toBe(false);
  },
);
