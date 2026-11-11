import { makeAutoObservable, observableRef } from "mobx";
import { requestId } from "studio-claude-bridge/request-id.ts";
import { errorText } from "../../../api.ts";
import { outboxRequests } from "../../../browserPersistence.ts";
import type { ServerAccessRequest } from "../../../servers/accessContract.ts";
import type { PostResult, ServerApi } from "../../../servers/net/api.ts";

/** Access mutations keep the original durable body through an uncertain result. */
export class ServerAccessForm {
  invitation: {
    via: ServerApi;
    value: string;
    grant: "full" | "observe";
  } | null = null;
  label = "Studio server";
  grant: "full" | "observe" = "full";
  error = "";
  busy = false;
  constructor() {
    makeAutoObservable(this, { invitation: observableRef }, { autoBind: true });
  }
  readPending(storage: Storage): ServerAccessRequest | null {
    try {
      return JSON.parse(
        outboxRequests(storage).get("serverAccess", "") || "null",
      );
    } catch {
      return null;
    }
  }
  setGrant(value: string) {
    this.grant = value === "observe" ? "observe" : "full";
  }
  createInvitation(api: ServerApi, storage: Storage) {
    return this.run(
      {
        action: "create_invite",
        label: this.label,
        grant: this.grant,
        requestId: requestId(),
      },
      api,
      storage,
    );
  }
  revoke(clientId: string, api: ServerApi, storage: Storage) {
    return this.run(
      { action: "revoke", clientId, requestId: requestId() },
      api,
      storage,
    );
  }
  retry(api: ServerApi, storage: Storage) {
    const pending = this.readPending(storage);
    if (pending) void this.run(pending, api, storage);
  }
  *run(next: ServerAccessRequest, api: ServerApi, storage: Storage) {
    const pending = this.readPending(storage);
    const outbox = outboxRequests(storage);
    if (this.busy) return;
    if (
      pending &&
      JSON.stringify({ ...pending, requestId: "" }) !==
        JSON.stringify({ ...next, requestId: "" })
    ) {
      this.error =
        "Retry the saved server access request before you start another request.";
      return;
    }
    const body = pending || next;
    try {
      outbox.set("serverAccess", "", JSON.stringify(body));
    } catch {
      this.error = "The request could not be saved. No server access changed.";
      return;
    }
    this.busy = true;
    this.error = "";
    try {
      yield api.refreshSession();
      const result: PostResult<"/api/multi-server"> = yield api.serverAccess(
        "POST",
        body,
      );
      if ("invitation" in result) {
        this.invitation = {
          via: api,
          value: JSON.stringify(result.invitation, null, 2),
          grant:
            "grant" in body && body.grant === "observe" ? "observe" : "full",
        };
      }
      outbox.remove("serverAccess", "");
    } catch (error) {
      this.error = errorText(error);
    } finally {
      this.busy = false;
    }
  }
}
