import { makeAutoObservable } from "mobx";
import { requestId } from "studio-claude-bridge/request-id.ts";
import { errorText } from "../../../api.ts";
import { outboxRequests } from "../../../browserPersistence.ts";
import { setServerExcluded } from "../../../servers/excludedServers.ts";
import { serverCredentialAdapter } from "../../../servers/net/transport.ts";
import { parseInvitation } from "../../../servers/pairing.ts";
import {
  readServers,
  serveOrigin,
  setLocalServerDetails,
  writeServers,
  type StudioServer,
} from "../../../servers/registry.ts";
import type { ServerDiscovery } from "../../../servers/useServerDiscovery.ts";

/** Manual pairing retains the saved request identity until the outcome is known. */
export class PairingForm {
  origin = "";
  code = "";
  label = "";
  error = "";
  busy = false;
  manual = false;
  revoke: { id: string; label: string } | null = null;
  request: { origin: string; code: string; id: string } | null = null;
  constructor() {
    makeAutoObservable(this, {}, { autoBind: true });
  }
  setManual(value: boolean) {
    this.manual = value;
  }
  setRevoke(value: typeof this.revoke) {
    this.revoke = value;
  }
  *saveAlias(
    localEnabled: boolean,
    setAlias: ServerDiscovery["setAlias"],
    server: StudioServer,
    paired: boolean,
    alias: string,
  ) {
    if (localEnabled && (server.id === "local" || paired)) {
      const saved: boolean = yield setAlias(server.id, alias);
      return saved;
    }
    if (server.id === "local") setLocalServerDetails({ alias });
    writeServers(
      readServers().map((row) =>
        row.id === server.id ? { ...row, alias } : row,
      ),
    );
    return true;
  }
  *forget(server: StudioServer, remove: (server: StudioServer) => void) {
    this.error = "";
    try {
      setServerExcluded(server.id, true);
      this.busy = true;
      yield serverCredentialAdapter().forget(server);
      remove(server);
    } catch (error) {
      this.error = errorText(error);
    } finally {
      this.busy = false;
    }
  }
  *pair(add: (server: StudioServer) => void) {
    if (this.busy) return;
    this.error = "";
    const outbox = outboxRequests();
    try {
      const address = serveOrigin(this.origin);
      const code = this.code.trim();
      if (!code) {
        this.error = "Paste the pairing invitation.";
        return;
      }
      const invitation = parseInvitation(code, address);
      const saved = JSON.parse(outbox.get("pairing", "") || "null");
      if (
        !this.request ||
        this.request.origin !== address ||
        this.request.code !== code
      ) {
        const id =
          saved?.origin === address && saved?.inviteId === invitation.inviteId
            ? saved.id
            : requestId();
        this.request = { origin: address, code, id };
        outbox.set(
          "pairing",
          "",
          JSON.stringify({
            origin: address,
            inviteId: invitation.inviteId,
            id,
          }),
        );
      }
      this.busy = true;
      const server: StudioServer = yield serverCredentialAdapter().pair(
        address,
        this.request.code,
        this.request.id,
      );
      setServerExcluded(server.id, false);
      add({ ...server, label: this.label.trim() || server.label });
      this.code = this.origin = this.label = "";
      this.request = null;
      outbox.remove("pairing", "");
    } catch (error) {
      this.error = errorText(error);
    } finally {
      this.busy = false;
    }
  }
}
