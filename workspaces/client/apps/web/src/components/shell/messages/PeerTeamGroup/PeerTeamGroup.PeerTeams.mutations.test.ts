import { beforeEach, afterEach } from "vitest";
import { PeerTeamMoveOwner } from "../../../../app/projects/peerTeamMutation.ts";
// @vitest-environment happy-dom
import { act, createElement, Fragment } from "react";
import { MantineProvider } from "@mantine/core";
import { expect, it, vi } from "vitest";
import { ApiError } from "../../../../api.ts";
import { fixture } from "../../../sidebar/chat-row/support/testFixture.ts";
import {
  localSidebarBackend,
  SidebarBackendProvider,
} from "../../../sidebar/support/services.tsx";
import {
  PeerTeamActionsProvider,
  PeerTeamForm,
  PeerTeamMoveFeedback,
  type PeerTeamActions,
} from "../PeerTeamGroup/PeerTeamGroup.tsx";

it.each(
  (["save", "delete", "move"] as const).flatMap((action) =>
    ["lost", "success", "rejected"].map((failure) => ({ action, failure })),
  ),
)(
  "$action preserves its request through $failure in the shared flow",
  async ({ action, failure }) => {
    const f = fixture();
    const backend = { ...localSidebarBackend };
    const other = { ...localSidebarBackend, post: vi.fn() };
    let activeBackend = backend;
    const post = vi.spyOn(backend, "post").mockResolvedValue({ ok: true });
    if (failure !== "success")
      post.mockRejectedValueOnce(
        failure === "lost"
          ? new Error("Lost response")
          : new ApiError("Project changed", 409),
      );
    const close = vi.fn();
    const project = { id: "/source", path: "/source", peerTeamsRevision: 7 };
    const team = {
      id: "team",
      name: "Original",
      members: ["a", "b"],
      projectPath: "/source",
    };
    const area: PeerTeamActions = {
      scope: "scope",
      closeForm: close,
      openRoom: () => {},
      edit: () => {},
      toggle: () => {},
      room: () => undefined,
      disabled: () => false,
      candidates: () => [],
    };
    const move = new PeerTeamMoveOwner(backend);
    function Move() {
      return createElement(
        Fragment,
        null,
        createElement(
          "button",
          { onClick: () => move.move(project, "a", "team") },
          "Move",
        ),
        createElement(PeerTeamMoveFeedback, { owner: move }),
      );
    }
    const render = () =>
      act(async () =>
        f.root.render(
          createElement(
            MantineProvider,
            null,
            createElement(
              SidebarBackendProvider,
              { value: activeBackend },
              createElement(
                PeerTeamActionsProvider,
                { value: area },
                action === "move"
                  ? createElement(Move)
                  : createElement(PeerTeamForm, { project, team, action }),
              ),
            ),
          ),
        ),
      );
    await render();
    const click = async (text: string) =>
      act(async () => {
        [...document.querySelectorAll("button")]
          .find((button) => button.textContent === text)
          ?.click();
      });
    await click(
      action === "save"
        ? "Save team"
        : action === "delete"
          ? "Dissolve"
          : "Move",
    );
    expect(post).toHaveBeenCalledTimes(1);
    const request = post.mock.calls[0]?.[1];
    expect(request).toMatchObject({
      expected_revision: 7,
      request_id: expect.any(String),
      action,
    });
    activeBackend = other;
    await render();
    project.peerTeamsRevision = 99;
    team.name = "Changed during retry";
    if (failure !== "success")
      await click(
        failure === "rejected"
          ? "Reload project"
          : action === "move"
            ? "Retry move"
            : "Retry",
      );
    expect(post).toHaveBeenCalledTimes(failure === "lost" ? 2 : 1);
    if (failure === "lost") expect(post.mock.calls[1]?.[1]).toBe(request);
    expect(other.post).not.toHaveBeenCalled();
    if (action !== "move") expect(close).toHaveBeenCalledTimes(1);
    await f.close();
  },
);

let renderErrors: ReturnType<typeof vi.spyOn>;
beforeEach(() => {
  renderErrors = vi.spyOn(console, "error");
});
afterEach(() => {
  const warnings = renderErrors.mock.calls.filter((args: unknown[]) =>
    args.some((arg: unknown) =>
      String(arg).includes("Cannot update a component"),
    ),
  );
  renderErrors.mockRestore();
  expect(warnings).toEqual([]);
});

it("uses the current backend and close callback after a mounted form rerender before its first save", async () => {
  const f = fixture();
  const originalPost = vi.fn().mockResolvedValue({ ok: true });
  const currentPost = vi.fn().mockResolvedValue({ ok: true });
  const originalClose = vi.fn(),
    currentClose = vi.fn();
  const project = { id: "project", path: "/source", peerTeamsRevision: 7 };
  const team = {
    id: "team",
    name: "Draft team",
    members: ["a", "b"],
    projectPath: "/source",
  };
  const actions: PeerTeamActions = {
    scope: "scope",
    closeForm: originalClose,
    openRoom() {},
    edit() {},
    toggle() {},
    room: () => undefined,
    disabled: () => false,
    candidates: () => [],
  };
  const render = (post: typeof originalPost, closeForm: typeof originalClose) =>
    act(async () =>
      f.root.render(
        createElement(
          MantineProvider,
          null,
          createElement(
            SidebarBackendProvider,
            { value: { ...localSidebarBackend, post } },
            createElement(
              PeerTeamActionsProvider,
              { value: { ...actions, closeForm } },
              createElement(PeerTeamForm, { project, team, action: "save" }),
            ),
          ),
        ),
      ),
    );
  try {
    await render(originalPost, originalClose);
    await render(currentPost, currentClose);
    await act(async () => {
      document
        .querySelector<HTMLFormElement>(".peer-team-form")
        ?.dispatchEvent(
          new Event("submit", { bubbles: true, cancelable: true }),
        );
    });
    expect(originalPost).not.toHaveBeenCalled();
    expect(currentPost).toHaveBeenCalledTimes(1);
    expect(currentPost.mock.calls[0]?.[1]).toMatchObject({
      path: "/source",
      team_id: "team",
      expected_revision: 7,
    });
    expect(originalClose).not.toHaveBeenCalled();
    expect(currentClose).toHaveBeenCalledTimes(1);
  } finally {
    await f.close();
  }
});

it("uses the current close callback after an uncertain save while retrying the pinned request and backend", async () => {
  const f = fixture();
  const post = vi
    .fn()
    .mockRejectedValueOnce(new Error("Lost response"))
    .mockResolvedValue({ ok: true });
  const replacementPost = vi.fn();
  const originalClose = vi.fn(),
    currentClose = vi.fn();
  const project = { id: "project", path: "/source", peerTeamsRevision: 7 };
  const team = {
    id: "team",
    name: "Draft",
    members: ["a", "b"],
    projectPath: "/source",
  };
  const actions: PeerTeamActions = {
    scope: "scope",
    closeForm: originalClose,
    openRoom() {},
    edit() {},
    toggle() {},
    room: () => undefined,
    disabled: () => false,
    candidates: () => [],
  };
  const render = (send: typeof post, closeForm: typeof originalClose) =>
    act(async () =>
      f.root.render(
        createElement(
          MantineProvider,
          null,
          createElement(
            SidebarBackendProvider,
            { value: { ...localSidebarBackend, post: send } },
            createElement(
              PeerTeamActionsProvider,
              { value: { ...actions, closeForm } },
              createElement(PeerTeamForm, { project, team, action: "save" }),
            ),
          ),
        ),
      ),
    );
  try {
    await render(post, originalClose);
    await act(async () =>
      document
        .querySelector<HTMLFormElement>(".peer-team-form")!
        .dispatchEvent(
          new Event("submit", { bubbles: true, cancelable: true }),
        ),
    );
    expect(post).toHaveBeenCalledTimes(1);
    const body = post.mock.calls[0]![1];
    await render(replacementPost, currentClose);
    await act(async () =>
      [...document.querySelectorAll("button")]
        .find((button) => button.textContent === "Retry")!
        .click(),
    );
    expect(post).toHaveBeenCalledTimes(2);
    expect(post.mock.calls[1]![1]).toBe(body);
    expect(replacementPost).not.toHaveBeenCalled();
    expect(originalClose).not.toHaveBeenCalled();
    expect(currentClose).toHaveBeenCalledTimes(1);
  } finally {
    await f.close();
  }
});
