import { expect, it } from "vitest";
import { createStateMirror } from "../generated/observableResources.ts";
import { agentSummaryFixture } from "../servers/agentSummaryFixture.ts";
import {
  createSidebarViewModel,
  SidebarProjectView,
} from "./sidebar/viewModel.ts";

it("keeps an unnamed managed lead visible and searches its legacy project label", () => {
  const mirror = createStateMirror();
  mirror.apply([
    {
      collection: "agentSummary",
      id: "lead",
      value: agentSummaryFixture("lead", {
        name: null,
        project: "Legacy project",
      }),
    },
  ]);
  const view = createSidebarViewModel(
    () => mirror.current,
    () => ({
      query: "legacy project",
      archived: false,
      opened: null,
      rank: () => 0,
    }),
  );
  expect(view.visibleRows.map((row) => row.agent.id)).toEqual(["lead"]);
});
it("uses the generated project's path when its name is missing", () => {
  expect(
    new SidebarProjectView(
      { id: "p", path: "/work/project", name: null },
      true,
      false,
    ).name,
  ).toBe("/work/project");
});
