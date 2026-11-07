import type { PostBody } from "./api";
import { type Agent } from "./types";
import { type NewChatWorkspaceMode } from "./components/newChatSettings";

type AssertNever<T extends never> = T;

export type NewChatModeUsesRequestOwner = AssertNever<
  | Exclude<
      NewChatWorkspaceMode,
      NonNullable<PostBody<"/api/leads">["workspaceMode"]>
    >
  | Exclude<
      Exclude<NonNullable<PostBody<"/api/leads">["workspaceMode"]>, "layr">,
      NewChatWorkspaceMode
    >
>;

// @ts-expect-error Shared folders are worker workspaces, not new-chat modes.
const invalidChatMode: NewChatWorkspaceMode = "shared";
const invalidWorkspace: Agent = {
  id: "invalid-workspace",
  // @ts-expect-error Agent workspace modes must come from the generated closed domain.
  workspaceMode: "unrecognized-workspace",
};

void invalidChatMode;
void invalidWorkspace;
