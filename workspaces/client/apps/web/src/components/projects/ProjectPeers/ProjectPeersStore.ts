import { whileObserved } from "../../../observed.ts";
import { makeAutoObservable, observableRef } from "mobx";
import {
  errorText,
  type ServerApi,
  type PostBody,
  type PostResult,
} from "../../../api.ts";
import type { components } from "../../../generated/api.ts";
import { fingerprintOf, parseProjectInvitation } from "./peerHelpers.ts";

type Reply = PostResult<"/api/federation">;
type Draft = { pasted: string; localProject: string; confirmed: boolean };

/** A project's grants load when the peer section observes them. */
export function createProjectPeersStore(
  projectId: string,
  api: Pick<ServerApi, "post">,
  notify: (message: string) => void,
) {
  const grants: components["schemas"]["ProjectGrant"][] = [];
  const form = makeAutoObservable(
    {
      grants,
      inviteText: "",
      pasted: "",
      invitation: parseProjectInvitation(""),
      fingerprint: "",
      localProject: projectId,
      confirmed: false,
      busy: false,
      change(values: Partial<Draft>) {
        Object.assign(this, values);
      },
      create() {
        return this.run({
          action: "create_project_invite",
          project_id: projectId,
        });
      },
      revoke(grant_id: string) {
        return this.run({ action: "revoke_project_grant", grant_id });
      },
      accept() {
        return this.invitation
          ? this.run({
              action: "accept_project_peer",
              invitation: this.invitation,
              project_id: this.localProject,
            })
          : undefined;
      },
      *run(
        body: PostBody<"/api/federation">,
      ): Generator<Promise<Reply>, void, Reply> {
        this.busy = true;
        try {
          const result = yield api.post("/api/federation", body);
          if (body.action === "create_project_invite") {
            if (!("invitation" in result))
              throw new Error("No invitation returned.");
            this.inviteText = JSON.stringify(result.invitation, null, 2);
          } else {
            if (!("grants" in result))
              throw new Error("The server sent no project grants.");
            this.grants = result.grants;
            if (body.action === "accept_project_peer") {
              this.invitation = null;
              this.pasted = "";
            }
          }
        } catch (error) {
          notify(errorText(error));
        } finally {
          this.busy = false;
        }
      },
      *choose(): Generator<Promise<string>, void, string> {
        this.busy = true;
        try {
          const invitation = parseProjectInvitation(this.pasted);
          if (!invitation) throw new Error("Paste a valid project invitation.");
          const fingerprint = yield fingerprintOf(invitation.publicKey);
          this.invitation = invitation;
          this.fingerprint = fingerprint;
          this.confirmed = false;
        } catch (error) {
          notify(errorText(error));
        } finally {
          this.busy = false;
        }
      },
    },
    { grants: observableRef },
    { autoBind: true },
  );
  whileObserved(form, "grants", () => {
    void form.run({ action: "list_project_grants", project_id: projectId });
    return () => {};
  });
  return form;
}
