// @vitest-environment happy-dom
import { act, createElement, Fragment } from "react";
import { MantineProvider } from "@mantine/core";
import { expect, it, vi } from "vitest";
import { ApiError } from "../../../api.ts";
import { fixture } from "../../chat-row/testFixture.ts";
import {
  localSidebarBackend,
  SidebarBackendProvider,
} from "../../sidebar/services.tsx";
import {
  PeerTeamActionsProvider,
  PeerTeamForm,
  usePeerTeamMove,
  type PeerTeamActions,
} from "./PeerTeams.tsx";

const shared = vi.hoisted(() => vi.fn());
vi.mock("./peerTeamMutation.ts", async (original) => {
  const actual = await original<typeof import("./peerTeamMutation.ts")>();
  return {
    ...actual,
    usePeerTeamMutation: (
      ...args: Parameters<typeof actual.usePeerTeamMutation>
    ) => {
      shared();
      return actual.usePeerTeamMutation(...args);
    },
  };
});

it.each(
  (["save", "delete", "move"] as const).flatMap((action) =>
    ["lost", "refresh", "rejected"].map((failure) => ({ action, failure })),
  ),
)(
  "$action preserves its request through $failure in the shared flow",
  async ({ action, failure }) => {
    const f = fixture();
    const backend = { ...localSidebarBackend };
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
    function Move() {
      const move = usePeerTeamMove(refresh);
      return createElement(
        Fragment,
        null,
        createElement(
          "button",
          { onClick: () => move.move(project, "a", "team") },
          "Move",
        ),
        move.feedback,
      );
    }
    shared.mockClear();
    await act(async () =>
      f.root.render(
        createElement(
          MantineProvider,
          null,
          createElement(
            SidebarBackendProvider,
            { value: backend },
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
    expect(shared).toHaveBeenCalled();
    expect(post).toHaveBeenCalledTimes(1);
    const request = post.mock.calls[0]?.[1];
    expect(request).toMatchObject({
      expected_revision: 7,
      request_id: expect.any(String),
      action,
    });
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
    expect(refresh).toHaveBeenCalledTimes(failure === "refresh" ? 2 : 1);
    if (action !== "move") expect(close).toHaveBeenCalledTimes(1);
    await f.close();
  },
);
