import { whileObserved } from "../../../observed.ts";
import { makeAutoObservable } from "mobx";
import type { ServerApi, PostBody, PostResult } from "../../../api.ts";

type FederationResponse = PostResult<"/api/federation">;
type FederationSnapshot = Extract<FederationResponse, { peers: unknown[] }>;
type FederationInvitation = Extract<
  FederationResponse,
  { invitation: unknown }
>["invitation"];

function isFederationInvitation(value: unknown): value is FederationInvitation {
  if (!value || typeof value !== "object" || Array.isArray(value)) return false;
  const invitation = value as Record<string, unknown>;
  return (
    invitation.protocol === 1 &&
    ["inviteId", "token", "stateId", "label", "origin", "publicKey"].every(
      (key) => typeof invitation[key] === "string",
    ) &&
    typeof invitation.expires === "number"
  );
}

export class FederationSettingsStore {
  membersByLead: Record<string, string[]> = {};
  state: FederationSnapshot | null = null;
  busy = false;
  private generation = 0;
  readonly api;
  readonly notify;
  private readonly stop;
  constructor(
    api: Pick<ServerApi, "post"> | null,
    notify: (message: string) => void,
  ) {
    this.api = api;
    this.notify = notify;
    makeAutoObservable(
      this,
      { api: false, notify: false },
      { autoBind: true, deep: false },
    );
    this.stop = whileObserved(this, "state", () => {
      if (this.api) void this.run({ action: "status" });
      return this.cancel;
    });
  }
  members(leadId: string) {
    return this.membersByLead[leadId] ?? [leadId];
  }
  toggleMember(leadId: string, id: string, checked: boolean) {
    const members = new Set(this.members(leadId));
    if (checked) members.add(id);
    else members.delete(id);
    this.membersByLead = { ...this.membersByLead, [leadId]: [...members] };
  }
  *pair(
    inviteText: string,
    change: (text: string) => void,
    notify = this.notify,
  ) {
    try {
      const invitation: unknown = JSON.parse(inviteText);
      if (!isFederationInvitation(invitation)) throw new Error();
      const result: FederationResponse | null = yield this.run({
        action: "accept_peer",
        invitation,
      });
      if (result) change("");
    } catch {
      notify("Paste a valid federation invitation JSON document.");
    }
  }
  *invite(
    serverLabel: string,
    expectedUser: string,
    change: (text: string) => void,
  ) {
    const result: FederationResponse | null = yield this.run({
      action: "create_invite",
      label: serverLabel,
      expected_user: expectedUser || undefined,
    });
    if (result && "invitation" in result)
      change(JSON.stringify(result.invitation, null, 2));
  }
  cancel() {
    this.generation++;
  }
  *run(body: PostBody<"/api/federation">) {
    if (!this.api) return null;
    const generation = this.generation;
    this.busy = true;
    try {
      const result: FederationResponse = yield this.api.post(
        "/api/federation",
        body,
      );
      if (generation !== this.generation) return null;
      if ("peers" in result) this.state = result;
      return result;
    } catch (error) {
      if (generation === this.generation)
        this.notify(error instanceof Error ? error.message : String(error));
      return null;
    } finally {
      if (generation === this.generation) this.busy = false;
    }
  }
  dispose() {
    this.cancel();
    this.stop();
  }
}
