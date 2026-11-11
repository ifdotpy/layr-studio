import { makeAutoObservable } from "mobx";
import { requestId as randomUUID } from "studio-claude-bridge/request-id.ts";
import { ApiError, errorText, type PostBody } from "../../../api.ts";
import { serverMutationResponseIsUncertain } from "../../../servers/mutationIdentity.ts";
import type { useServerApi } from "../../servers/ServerServicesProvider/ServerServicesProvider.tsx";
import type { useConversationActions } from "../support/navigation.ts";
import type { StateCollections } from "../../../generated/observableResources.ts";
export type RadioRoom =
  StateCollections["rooms"] extends ReadonlyMap<string, infer T> ? T : never;

type RadioRequest = Extract<
  PostBody<"/api/peer-teams">,
  { action: "radio"; radio_action: "send" | "pass" | "stop" }
>;
type RadioActionInput =
  | { radio_action: "pass"; target: string }
  | { radio_action: "stop" }
  | { radio_action: "send"; text: string; target: string; rounds: 1 | 2 };
type Attempt = {
  body: RadioRequest;
  acknowledged?: boolean;
  rejected?: boolean;
};
type RadioScope = {
  api: ReturnType<typeof useServerApi>;
  area: ReturnType<typeof useConversationActions>;
};
/** Retains the exact radio command until its existing server boundary confirms it. */
export class RadioDelivery {
  attempt: Attempt | null;
  pending = false;
  error = "";
  target = "both";

  readonly identity: { roomId: string; key: string };
  constructor(identity: RadioDelivery["identity"], attempt: Attempt | null) {
    this.identity = identity;
    this.attempt = attempt;
    makeAutoObservable(this, { identity: false }, { autoBind: true });
  }

  get frozen() {
    return this.pending || !!this.attempt;
  }
  setTarget(value: string | null) {
    this.target = value || "both";
  }
  remember(value: Attempt | null, api: RadioScope["api"]) {
    api.save(this.identity.key, value);
    this.attempt = value;
  }
  *execute(
    next: Attempt,
    { api, area }: RadioScope,
  ): Generator<Promise<unknown>, void, unknown> {
    if (this.pending) return;
    this.pending = true;
    this.error = "";
    this.remember(next, api);
    try {
      if (!next.acknowledged && !next.rejected) {
        yield api.post("/api/peer-teams", next.body, {
          timeoutMs: 15000,
        });
        next = { ...next, acknowledged: true };
        this.remember(next, api);
      }
      if (
        next.acknowledged &&
        next.body.radio_action === "send" &&
        area.scope.services.drafts.getDraft(this.identity.roomId) ===
          next.body.text
      )
        area.delivery.editDraft("", this.identity.roomId);
      this.remember(null, api);
    } catch (e) {
      if (
        !next.acknowledged &&
        e instanceof ApiError &&
        !serverMutationResponseIsUncertain(e.status, e.response)
      ) {
        next = { ...next, rejected: true };
        this.remember(next, api);
      }
      this.error = errorText(e);
    } finally {
      this.pending = false;
    }
  }
  command(
    input: RadioActionInput,
    room: RadioRoom,
    state: StateCollections,
    scope: RadioScope,
  ) {
    const radio = room.radio!;
    if (
      this.frozen ||
      (input.radio_action === "send" &&
        (radio.controls.inputBlocked || !input.text.trim()))
    )
      return;
    const path =
      room.projectPath || state.peerTeams.get(radio.teamId)?.projectPath;
    const teamId = radio.teamId || room.peerTeamId;
    if (!path || !teamId) {
      scope.area.notices.notify(
        `The shared chat ${path ? "identity" : "project"} is unavailable. Refresh Studio.`,
      );
      return;
    }
    const body: RadioRequest = {
      action: "radio",
      path,
      team_id: teamId,
      request_id: randomUUID(),
      expected_revision: radio.revision,
      ...input,
    };
    void this.execute({ body }, scope);
  }
}
