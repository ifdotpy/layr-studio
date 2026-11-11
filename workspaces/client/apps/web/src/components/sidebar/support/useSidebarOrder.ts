import { flowValue } from "../../../flowValue.ts";
import { outboxRequests } from "../../../browserPersistence.ts";
import { requestId as randomUUID } from "studio-claude-bridge/request-id.ts";
import type { SidebarBackend, SidebarServices } from "./services.tsx";
import type { DragEvent, KeyboardEvent } from "react";
import { makeAutoObservable, observableRef, reaction, flow } from "mobx";
import {
  ApiError,
  errorText,
  untilAnswered,
  type PostBody,
  type PostResult,
} from "../../../api.ts";
import type { components } from "../../../generated/api.ts";
import type { DeepReadonly } from "../../../deepReadonly.ts";
type Order = Readonly<Record<string, readonly string[]>>;
type Drag = {
  group: string;
  id: string;
};
export type SidebarDragTarget = Drag & {
  after: boolean;
};
type ServerOrder = DeepReadonly<components["schemas"]["SidebarOrderDto"]>;
type Pending = {
  ownerId?: string;
  body: Extract<
    PostBody<"/api/projects">,
    {
      action: "reorder";
    }
  >;
};
// ProjectsResponse also includes generic JSON; narrow its reorder variant.
function isSidebarOrderResponse(
  value: PostResult<"/api/projects">,
): value is components["schemas"]["SidebarOrderResponse"] {
  return "groups" in value && typeof value.revision === "number";
}
export function sidebarOrderCanWrite(
  sessionToken: string | undefined,
  available: boolean | undefined,
): boolean {
  return available !== false && (!!sessionToken || available === true);
}
export interface SidebarOrderSource {
  readonly stateDir: string;
  readonly runtime:
    | {
        readonly sidebarOrder?: ServerOrder | null | undefined;
      }
    | undefined;
  readonly services: SidebarServices;
  readonly sessionToken?: string | undefined;
}
export class SidebarOrderOwner {
  readonly source: SidebarOrderSource;
  readonly outbox: ReturnType<typeof outboxRequests>;
  readonly notify;
  constructor(source: SidebarOrderSource, notify?: (message: string) => void) {
    this.notify = notify;
    this.source = source;
    this.outbox = outboxRequests(source.services.cache.storage);
    this.order = source.runtime?.sidebarOrder?.groups || {};
    makeAutoObservable(
      this,
      {
        source: false,
        notify: false,
        outbox: false,
        order: observableRef,
        requestBackend: false,
        revision: false,
        busy: false,
        drag: false,
      },
      { autoBind: true },
    );
  }
  start() {
    return reaction(
      () =>
        [
          this.source.stateDir,
          this.source.runtime?.sidebarOrder,
          this.source.sessionToken,
        ] as const,
      () => {
        if (
          !this.source.runtime?.sidebarOrder ||
          !sidebarOrderCanWrite(
            this.source.sessionToken,
            this.source.services.available?.({
              kind: "order",
              group: "projects",
            }),
          ) ||
          this.busy
        )
          return;
        this.adoptServerOrder(this.source.runtime?.sidebarOrder);
        const pending = this.readPending();
        if (pending?.body) void this.send(pending);
      },
      { fireImmediately: true },
    );
  }
  get pendingSuffix() {
    return this.source.stateDir + ":pending";
  }
  readPending = (): Pending | null => {
    try {
      return JSON.parse(
        this.outbox.get("sidebarOrder", this.pendingSuffix) || "null",
      );
    } catch {
      return null;
    }
  };
  requestBackend: SidebarBackend | null = null;
  order: Order = {};
  revision = -1;
  busy = false;
  adoptServerOrder = (value: ServerOrder | null | undefined) => {
    if (!value || value.revision <= this.revision) return;
    this.revision = value.revision;
    if (value.groups !== null) {
      this.order = value.groups;
    }
  };
  drag: Drag | null = null;
  target: SidebarDragTarget | null = null;
  setTarget = (
    value:
      | SidebarDragTarget
      | null
      | ((old: SidebarDragTarget | null) => SidebarDragTarget | null),
  ) => {
    this.target = typeof value === "function" ? value(this.target) : value;
  };
  announcement = "";
  announce = (value: string) => {
    this.announcement = value;
  };
  destination: string | null = null;
  setDestination = (value: string | null) => {
    this.destination = value;
  };
  send = flow(function* (
    this: SidebarOrderOwner,
    request: Pending,
    optimistic?: Order,
  ) {
    if (this.busy) return false;
    this.busy = true;
    const suffix = this.pendingSuffix;
    let clearPending = false;
    try {
      this.outbox.set("sidebarOrder", suffix, JSON.stringify(request));
      if (optimistic) this.order = optimistic;
      const backend =
        this.requestBackend ||
        this.source.services.byOwner(
          request.ownerId || this.source.services.cache.ownerId,
        );
      this.requestBackend = backend;
      // The request ID makes a repeated reorder return the first receipt.
      const result = yield* flowValue(
        untilAnswered(() =>
          backend.post("/api/projects", request.body, { timeoutMs: 30000 }),
        ),
      );
      if (isSidebarOrderResponse(result) && result.revision >= this.revision) {
        this.revision = result.revision;
        this.order = result.groups || {};
      }
      this.adoptServerOrder(this.source.runtime?.sidebarOrder);
      this.requestBackend = null;
      clearPending = true;
      return true;
    } catch (error) {
      if (
        error instanceof ApiError &&
        error.status >= 400 &&
        error.status < 500 &&
        error.status !== 408
      ) {
        this.requestBackend = null;
        clearPending = true;
        this.order = this.source.runtime?.sidebarOrder?.groups || {};
        this.notify?.(
          error.status === 409
            ? "Sidebar order changed in another client. Review the current order."
            : `Could not save sidebar order: ${errorText(error)}`,
        );
      } else {
        this.notify?.(`Could not save sidebar order: ${errorText(error)}`);
      }
      return false;
    } finally {
      this.busy = false;
      if (clearPending) this.outbox.remove("sidebarOrder", suffix);
    }
  });
  rank = (group: string, id: string) => {
    const index = this.order[group]?.indexOf(id) ?? -1;
    return index < 0 ? Number.MAX_SAFE_INTEGER : index;
  };
  move = (
    group: string,
    ids: string[],
    from: string,
    to: string,
    after: boolean,
  ) => {
    if (from === to || !ids.includes(from) || !ids.includes(to)) return;
    const next = ids.filter((id) => id !== from);
    next.splice(next.indexOf(to) + Number(after), 0, from);
    const target = { kind: "order" as const, group };
    if (
      !sidebarOrderCanWrite(
        this.source.sessionToken,
        this.source.services.available?.(target),
      ) ||
      !this.source.runtime?.sidebarOrder ||
      this.busy
    )
      return;
    if (this.readPending()) {
      this.notify?.(
        "Retry the saved sidebar order request before moving another item.",
      );
      return;
    }
    const value = { ...this.order, [group]: next };
    const backend = this.source.services.owner({ kind: "order", group });
    this.requestBackend = backend;
    void this.send(
      {
        ...(backend.ownerId === this.source.services.cache.ownerId
          ? {}
          : { ownerId: backend.ownerId }),
        body: {
          action: "reorder",
          request_id: randomUUID(),
          expected_revision: this.revision,
          groups: Object.fromEntries(
            Object.entries(value).map(([key, ids]) => [key, [...ids]]),
          ),
        },
      },
      value,
    ).then((saved) => {
      if (saved)
        this.announce(
          `Moved to position ${next.indexOf(from) + 1} of ${next.length}`,
        );
    });
  };
  finish = () => {
    this.drag = null;
    this.setTarget(null);
    this.setDestination(null);
  };
  dropBindings = (
    key: string,
    accepts: (source: Drag, event: DragEvent<HTMLElement>) => boolean,
    dropped: (source: Drag) => void,
    fallback: ReturnType<typeof this.bindings> | Record<string, never> = {},
  ) => ({
    ...fallback,
    "data-folder-drop": this.destination === key ? "true" : undefined,
    onDragOver: (event: DragEvent<HTMLElement>) => {
      if (!this.drag || !accepts(this.drag, event)) {
        fallback.onDragOver?.(event);
        return;
      }
      event.preventDefault();
      event.stopPropagation();
      event.dataTransfer.dropEffect = "move";
      this.setTarget(null);
      this.setDestination(key);
    },
    onDragLeave: (event: DragEvent<HTMLElement>) => {
      if (!event.currentTarget.contains(event.relatedTarget as Node | null))
        this.setDestination(null);
      fallback.onDragLeave?.(event);
    },
    onDrop: (event: DragEvent<HTMLElement>) => {
      if (!this.drag || !accepts(this.drag, event)) {
        fallback.onDrop?.(event);
        return;
      }
      event.preventDefault();
      event.stopPropagation();
      const source = this.drag;
      this.finish();
      dropped(source);
    },
  });
  bindings = (group: string, id: string, ids: string[]) => ({
    draggable: true,
    "data-sidebar-group": group,
    "data-sidebar-id": id,
    "data-drop-edge":
      this.target?.group === group && this.target.id === id
        ? this.target.after
          ? "after"
          : "before"
        : undefined,
    onDragStart: (event: DragEvent<HTMLElement>) => {
      event.stopPropagation();
      this.drag = { group, id };
      event.dataTransfer.effectAllowed = "move";
      event.dataTransfer.setData("application/x-studio-sidebar", id);
    },
    onDragOver: (event: DragEvent<HTMLElement>) => {
      if (this.drag?.group !== group || this.drag.id === id) return;
      event.preventDefault();
      event.stopPropagation();
      event.dataTransfer.dropEffect = "move";
      this.setDestination(null);
      const box = event.currentTarget.getBoundingClientRect();
      const after = event.clientY >= box.top + box.height / 2;
      this.setTarget((old) =>
        old?.group === group && old.id === id && old.after === after
          ? old
          : { group, id, after },
      );
    },
    onDragLeave: (event: DragEvent<HTMLElement>) => {
      if (!event.currentTarget.contains(event.relatedTarget as Node | null))
        this.setTarget(null);
    },
    onDrop: (event: DragEvent<HTMLElement>) => {
      if (this.drag?.group !== group) return;
      event.preventDefault();
      event.stopPropagation();
      const box = event.currentTarget.getBoundingClientRect();
      this.move(
        group,
        ids,
        this.drag.id,
        id,
        event.clientY >= box.top + box.height / 2,
      );
      this.finish();
    },
    onDragEnd: this.finish,
    onKeyDown: (event: KeyboardEvent<HTMLElement>) => {
      if (
        !event.altKey ||
        event.ctrlKey ||
        event.metaKey ||
        event.shiftKey ||
        !["ArrowUp", "ArrowDown"].includes(event.key)
      )
        return;
      event.preventDefault();
      const after = event.key === "ArrowDown";
      const visible = Array.from(
        event.currentTarget
          .closest("#chat-list")
          ?.querySelectorAll<HTMLElement>("[data-sidebar-group]") || [],
      )
        .filter(
          (element) =>
            element.dataset.sidebarGroup === group &&
            element.getClientRects().length,
        )
        .map((element) => element.dataset.sidebarId!);
      const neighbors = visible.length ? visible : ids;
      const to = neighbors[neighbors.indexOf(id) + (after ? 1 : -1)];
      if (to) this.move(group, ids, id, to, after);
    },
  });
}
