import { flow, makeAutoObservable } from "mobx";
import { z } from "zod";
import { flowValue } from "../../flowValue.ts";
import type { ServerAreas } from "../ServerAreasProvider.tsx";
import type { Intent, RequestEntry } from "../../servers/RequestRegistry.ts";

const oldRenames = z.record(
  z.string(),
  z.object({ text: z.string(), id: z.string() }),
);
const oldSidebarRename = z.object({
  body: z.object({
    id: z.string(),
    name: z.string().nullable().optional(),
    request_id: z.string(),
  }),
});
export class RenameOwner {
  readonly areas;
  constructor(areas: ServerAreas) {
    this.areas = areas;
    makeAutoObservable(this, { areas: false }, { autoBind: true });
  }
  wait = flow(function* (this: RenameOwner, entry: RequestEntry) {
    yield* flowValue(entry.completion);
    if (entry.state === "unknown" || entry.state === "failed")
      throw new Error(entry.errorText);
    if (entry.state === "confirmed")
      this.areas.notices.notify("Rename requested. Waiting for live update.");
  });
  submit = flow(function* (
    this: RenameOwner,
    id: string,
    text: string,
    name?: string | null,
  ) {
    if (name && name.length > 80)
      throw new Error("A name must have 1 to 80 characters.");
    const { requests, storage, api } = this.areas.scope.services;
    const intent: Intent = {
      key: [
        this.areas.startup.workspaceId || api.getWorkspaceId(),
        "rename",
        id,
      ],
      path: "/api/rename",
      body: { id, name },
    };
    const current = requests.latest(intent.key);
    if (current?.state === "sending")
      return yield* flowValue(this.wait(current));
    // One-time conversion only; no parallel pending map or status lifecycle remains.
    const key = text
      ? `studio-pending-renames:${this.areas.startup.creationScope}`
      : `studio-sidebar-rename:${id}`;
    const raw = storage.getItem(key);
    let restored: RequestEntry | undefined;
    if (raw && text) {
      const old = oldRenames.parse(JSON.parse(raw));
      if (old[id]?.text === text) {
        restored = requests.restore(intent, old[id].id);
        delete old[id];
        if (Object.keys(old).length) storage.setItem(key, JSON.stringify(old));
        else storage.removeItem(key);
      }
    } else if (raw) {
      const { body } = oldSidebarRename.parse(JSON.parse(raw));
      if (body.name !== name)
        throw new Error("Retry the saved name before changing it.");
      restored = requests.restore(
        { ...intent, body: { id: body.id, name: body.name } },
        body.request_id,
      );
      storage.removeItem(key);
    }
    const entry = restored || requests.start(intent);
    if (restored) yield* flowValue(requests.retry(entry.requestId));
    yield* flowValue(this.wait(entry));
  });
  rename(id: string, name: string) {
    return this.submit(id, "", name);
  }
}
