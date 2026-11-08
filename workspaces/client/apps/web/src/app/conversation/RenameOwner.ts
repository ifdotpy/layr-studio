import type { ServerAreas } from "../ServerAreasProvider.tsx";
import { makeAutoObservable, observable, reaction, runInAction } from "mobx";
import { errorText } from "../../api.ts";
import { randomUUID } from "../../uuid.ts";
import { submitRename, watchRenameResult } from "../../renameRequest.ts";

type PendingRename = { text: string; id: string };
export class RenameOwner {
  pending = observable.map<string, PendingRename>({}, { deep: false });
  readonly areas;
  constructor(areas: ServerAreas) {
    this.areas = areas;
    makeAutoObservable(this, { areas: false }, { autoBind: true });
  }
  get storageKey() {
    return `studio-pending-renames:${this.areas.startup.creationScope || ""}`;
  }
  persist() {
    this.areas.scope.services.api.save(
      this.storageKey,
      Object.fromEntries(this.pending),
    );
  }
  start() {
    const restore = reaction(
      () => this.areas.startup.creationScope,
      () => this.restore(),
      {
        fireImmediately: true,
      },
    );
    let stops: (() => void)[] = [];
    const watch = reaction(
      () => [...this.pending],
      (entries) => {
        stops.forEach((stop) => stop());
        stops = entries.map(([agentId, request]) =>
          watchRenameResult(
            this.areas.scope.services.resources,
            agentId,
            request.id,
            (receipt) => this.settle(agentId, request.id, receipt),
          ),
        );
      },
      { fireImmediately: true },
    );
    return () => {
      restore();
      watch();
      stops.forEach((stop) => stop());
    };
  }
  restore() {
    this.pending.replace(
      this.areas.startup.creationScope
        ? this.areas.scope.services.api.saved<Record<string, PendingRename>>(
            this.storageKey,
            {},
          )
        : {},
    );
  }
  settle(
    id: string,
    requestId: string,
    receipt: Awaited<ReturnType<typeof submitRename>>,
  ) {
    if (this.pending.get(id)?.id !== requestId || receipt.status === "pending")
      return;
    this.pending.delete(id);
    this.persist();
    if (receipt.status === "failed")
      this.areas.notices.notify(
        receipt.error || "The title could not be generated.",
      );
  }
  async submit(id: string, text: string, name?: string | null) {
    const { scope } = this.areas;
    if (name && name.length > 80)
      throw new Error("A name must have 1 to 80 characters.");
    const previous = this.pending.get(id);
    const request =
      previous?.text === text ? previous : { text, id: randomUUID() };
    if (request !== previous) {
      this.pending.set(id, request);
      this.persist();
    }
    const result = await submitRename(
      { id, name, request_id: request.id },
      scope.services.api.post,
      scope.services.sync.refreshState,
    );
    runInAction(() => {
      this.settle(id, request.id, result);
      if (
        result.status === "pending" &&
        this.pending.get(id)?.id === request.id
      )
        this.areas.notices.notify(
          "Rename requested. Waiting for live updates. Use /rename again to check this request.",
        );
    });
  }
  async rename(id: string, name: string) {
    const { scope, notices } = this.areas;
    try {
      await scope.services.api.post("/api/rename", {
        id,
        name,
        request_id: randomUUID(),
      });
      void scope.services.sync
        .refreshState()
        .catch((error) => notices.notify(errorText(error)));
    } catch (error) {
      notices.notify(errorText(error));
      throw error;
    }
  }
}
