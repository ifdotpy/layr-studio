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
    ["lost", "refresh", "rejected"].map((failure) => ({ action, failure })),
  ),
)(
  "$action preserves its request through $failure in the shared flow",
  async ({ action, failure }) => {
    const f = fixture();
    const backend = { ...localSidebarBackend };
    const other = { ...localSidebarBackend, post: vi.fn() };
    let activeBackend = backend;
    const post = vi.spyOn(backend, "post").mockResolvedValue({ ok: true });
    if (failure !== "refresh")
      post.mockRejectedValueOnce(
        failure === "lost"
          ? new Error("Lost response")
          : new ApiError("Project changed", 409),
      );
    const refresh = vi.fn(async () => {});
    if (failure === "refresh")
      refresh.mockRejectedValueOnce(new Error("Refresh lost"));
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
      refresh,
      closeForm: close,
      openRoom: () => {},
      edit: () => {},
      toggle: () => {},
      room: () => undefined,
      disabled: () => false,
      candidates: () => [],
    };
    const move = new PeerTeamMoveOwner(backend, refresh);
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
    expect(refresh).toHaveBeenCalledTimes(failure === "refresh" ? 2 : 1);
    if (action !== "move") expect(close).toHaveBeenCalledTimes(1);
    await f.close();
  },
);
