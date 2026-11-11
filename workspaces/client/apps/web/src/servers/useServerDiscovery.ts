import { makeAutoObservable, reaction, flowResult } from "mobx";
import { requestId as randomUUID } from "studio-claude-bridge/request-id.ts";
import {
  capturePreferenceWrite,
  readPreferenceFields,
} from "../sync/uiPreferenceStore.ts";
import { ServerManagementRequests } from "./managementRequests.ts";
import { resourceData } from "../store/runtime.ts";
import { errorText } from "../api.ts";
import {
  discoverySnapshot,
  effectiveDiscoveryAliases,
} from "./discoveryModel.ts";
import type { ServerAccessRequest } from "./accessContract.ts";
import { readServers, setLocalServerDetails } from "./registry.ts";
import { serverCredentialAdapter } from "./net/transport.ts";
import { adoptTrustedServers } from "./trustedServers.ts";
import type { ServerServices } from "./serverServices.ts";

/** Shell-owned management actions and derived streamed discovery. */
export class ServerDiscovery {
  error = "";
  busy = false;
  pending: ServerAccessRequest | null = null;
  readonly services;
  readonly enabled;
  readonly localAvailable;
  readonly workspaceIds;
  readonly management;
  constructor(
    services: {
      api: Pick<ServerServices["api"], "refreshSession" | "serverAccess">;
      store: Pick<ServerServices["store"], "multiServer">;
    },
    enabled: boolean,
    localAvailable: boolean,
    workspaceIds: () => Record<string, string | undefined>,
  ) {
    this.services = services;
    this.enabled = enabled;
    this.localAvailable = localAvailable;
    this.workspaceIds = workspaceIds;
    this.management = new ServerManagementRequests(
      location.origin,
      localStorage,
      async (name, work) => {
        if (!navigator.locks)
          throw new Error(
            "Server settings need a browser with Web Locks support.",
          );
        await navigator.locks.request(name, work);
      },
      async (body) => {
        if (
          !this.enabled &&
          body.action === "name" &&
          body.serverId !== "local"
        ) {
          const server = readServers().find((row) => row.id === body.serverId);
          if (!server) throw new Error("Select a paired server.");
          const response = await serverCredentialAdapter().fetch(
            server,
            new Request(server.origin + "/api/multi-server", {
              method: "POST",
              headers: { "Content-Type": "application/json" },
              body: JSON.stringify({ ...body, serverId: "local" }),
              signal: AbortSignal.timeout(15000),
            }),
          );
          if (!response.ok) {
            const value = await response.json();
            throw new Error(value.error || "The server refused the name.");
          }
          return;
        }
        const session = await this.services.api.refreshSession();
        await this.services.api.serverAccess(
          "POST",
          body,
          body.action === "discover"
            ? 105000
            : body.action === "name"
              ? 40000
              : 15000,
          session.token,
        );
      },
    );
    makeAutoObservable(
      this,
      { services: false, management: false, workspaceIds: false },
      { autoBind: true },
    );
  }
  get state() {
    return this.localAvailable
      ? resourceData(this.services.store.multiServer().current)
      : undefined;
  }
  get checkedAt() {
    return this.state ? Date.now() / 1000 : null;
  }
  get snapshot() {
    const snapshot = this.state ? discoverySnapshot(this.state) : null;
    return snapshot
      ? {
          ...snapshot,
          aliases: effectiveDiscoveryAliases(
            snapshot.aliases,
            this.workspaceIds(),
            readPreferenceFields("user"),
          ),
        }
      : null;
  }
  updatePending() {
    try {
      this.pending = this.management.pending()[0] || null;
    } catch (failure) {
      this.error = errorText(failure);
    }
  }
  *adopt() {
    const snapshot = this.snapshot;
    if (!snapshot) return;
    try {
      yield adoptTrustedServers(snapshot);
    } catch (failure) {
      this.error = errorText(failure);
    }
  }
  start() {
    this.updatePending();
    const stop = reaction(
      () => this.snapshot,
      (snapshot) => {
        if (snapshot?.localLabel || snapshot?.aliases?.local)
          setLocalServerDetails({
            ...(snapshot.localLabel ? { label: snapshot.localLabel } : {}),
            ...(snapshot.aliases?.local
              ? { alias: snapshot.aliases.local }
              : {}),
          });
        if (snapshot) void this.adopt();
      },
      { fireImmediately: true },
    );
    window.addEventListener("storage", this.updatePending);
    return () => {
      stop();
      window.removeEventListener("storage", this.updatePending);
    };
  }
  *run(request: ServerAccessRequest) {
    if ((!this.enabled && request.action !== "name") || this.busy) return false;
    this.busy = true;
    this.error = "";
    let confirmed = false;
    try {
      yield this.management.run(request);
      confirmed = true;
      if (request.action === "alias") {
        const identity = this.workspaceIds()[request.serverId];
        if (identity) {
          const key = `server-alias:${identity}`;
          const previous =
            readPreferenceFields("user")[JSON.stringify([key])]?.value;
          capturePreferenceWrite(
            key,
            JSON.stringify(request.alias),
            JSON.stringify(previous ?? null),
          );
        }
      }
      return true;
    } catch (failure) {
      this.error = errorText(failure);
      return confirmed;
    } finally {
      this.updatePending();
      this.busy = false;
    }
  }
  get actions(): Pick<
    ServerDiscovery,
    "find" | "setAutoPair" | "setAutoUpdate"
  > {
    return this;
  }
  find() {
    void this.run({ action: "discover", requestId: randomUUID() });
  }
  setAutoPair(autoPair: boolean) {
    void this.run({ action: "settings", autoPair, requestId: randomUUID() });
  }
  setAutoUpdate(autoUpdate: boolean) {
    void this.run({ action: "settings", autoUpdate, requestId: randomUUID() });
  }
  setName(serverId: string, label: string): Promise<boolean> {
    return flowResult(
      this.run({ action: "name", serverId, label, requestId: randomUUID() }),
    );
  }
  setAlias(serverId: string, alias: string): Promise<boolean> {
    return flowResult(
      this.run({ action: "alias", serverId, alias, requestId: randomUUID() }),
    );
  }
  revoke(id: string) {
    void this.run({ action: "revoke", clientId: id, requestId: randomUUID() });
  }
  allowAgain(clientId: string) {
    void this.run({ action: "unrevoke", clientId, requestId: randomUUID() });
  }
  get retry() {
    const pending = this.pending;
    return pending
      ? () => {
          void this.run(pending);
        }
      : undefined;
  }
}
