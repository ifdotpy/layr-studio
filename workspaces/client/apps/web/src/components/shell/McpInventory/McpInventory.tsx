import { requestId as randomUUID } from "studio-claude-bridge/request-id.ts";
import { sessionOutboxRequests } from "../../../browserPersistence.ts";
import { Button } from "@mantine/core";
import { flowResult, makeAutoObservable, observableRef } from "mobx";
import { observer } from "mobx-react-lite";
import {
  errorText,
  type GetResult,
  type PostBody,
  type PostResult,
} from "../../../api.ts";

type Inventory = GetResult<"/api/capabilities">;
type Reconnect = PostBody<"/api/mcp/reconnect">;
type Result = PostResult<"/api/mcp/reconnect">;
type Props = { state: McpInventoryStore; query: string };
type Server = { name: string; status: string; error: string };

export function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

export function mcpServers(entries: Inventory["mcp"]): Server[] {
  return entries.flatMap((entry) => {
    if (!isRecord(entry)) return [];
    const name = typeof entry.name === "string" ? entry.name : entry.serverName;
    if (typeof name !== "string" || !name.trim()) return [];
    const detail = entry.error;
    const error =
      typeof detail === "string"
        ? detail
        : isRecord(detail) && typeof detail.message === "string"
          ? detail.message
          : "";
    return [
      {
        name,
        status: typeof entry.status === "string" ? entry.status : "Unknown",
        error,
      },
    ];
  });
}

export class McpInventoryStore {
  inventory: Inventory | null = null;
  rows: Server[] = [];
  pageBusy = false;
  pageError = "";
  feedback: Record<string, { busy: boolean; message: string }> = {};
  requests = new Map<string, Reconnect>();
  private revision = 0;
  private seen = new Set<string>();
  private active = true;
  readonly outbox = sessionOutboxRequests();
  readonly agent: string;
  readonly owner: string;
  readonly loadPage: (cursor?: string) => Promise<Inventory>;
  readonly reconnect: (body: Reconnect) => Promise<Result>;
  constructor(
    owner: string,
    agent: string,
    loadPage: McpInventoryStore["loadPage"],
    reconnect: McpInventoryStore["reconnect"],
  ) {
    this.owner = owner;
    this.agent = agent;
    this.loadPage = loadPage;
    this.reconnect = reconnect;
    makeAutoObservable(
      this,
      {
        inventory: observableRef,
        outbox: false,
        loadPage: false,
        reconnect: false,
      },
      { autoBind: true },
    );
  }
  dispose() {
    this.active = false;
  }
  receive(initial: Inventory) {
    this.revision++;
    this.inventory = initial;
    this.rows = mcpServers(initial.mcp);
    this.seen.clear();
    this.pageError = "";
    this.pageBusy = false;
    this.restore();
  }
  requestSuffix(name: string) {
    const session = this.inventory?.mcpSession;
    return JSON.stringify([this.owner, this.agent, session, name]);
  }
  requestKey(name: string) {
    return this.outbox.key("mcpReconnect", this.requestSuffix(name));
  }
  report(key: string, message: string, busy = false) {
    this.feedback[key] = { busy, message };
  }
  restore() {
    const session = this.inventory?.mcpSession;
    if (!session) return;
    for (const row of this.rows) {
      const key = this.requestKey(row.name);
      try {
        const requestId = this.outbox.get(
          "mcpReconnect",
          this.requestSuffix(row.name),
        );
        if (requestId && !this.requests.has(key)) {
          this.requests.set(key, {
            id: this.agent,
            name: row.name,
            session,
            request_id: requestId,
          });
          this.report(
            key,
            "Reconnect outcome is unconfirmed. Check the reconnect result.",
          );
        }
      } catch {
        /* The click handler reports unavailable storage before any write. */
      }
    }
  }
  *load(cursor?: string): Generator<Promise<Inventory>, void, Inventory> {
    if (this.pageBusy) return;
    const current = ++this.revision;
    this.pageBusy = true;
    this.pageError = "";
    try {
      if (cursor && this.seen.has(cursor))
        throw new Error(
          "The server list cursor did not advance. Refresh the tools list.",
        );
      const page = yield this.loadPage(cursor);
      if (!this.active || current !== this.revision) return;
      if (cursor) {
        if (
          JSON.stringify(page.mcpSession) !==
          JSON.stringify(this.inventory?.mcpSession)
        )
          throw new Error("The agent session changed. Refresh the tools list.");
        if (page.errors.length) throw new Error(page.errors.join(" "));
        this.seen.add(cursor);
        this.rows = [
          ...new Map(
            [...this.rows, ...mcpServers(page.mcp)].map((row) => [
              row.name,
              row,
            ]),
          ).values(),
        ];
        this.inventory = page;
        this.restore();
      } else this.receive(page);
      this.pageError = page.errors.join(" ");
    } catch (error) {
      if (this.active && current === this.revision)
        this.pageError = errorText(error);
    } finally {
      if (this.active && current === this.revision) this.pageBusy = false;
    }
  }
  *reconnectServer(
    name: string,
  ): Generator<Promise<Result | void>, void, Result> {
    const suffix = this.requestSuffix(name);
    const storageKey = this.requestKey(name);
    if (!this.inventory?.mcpSession || this.feedback[storageKey]?.busy) return;
    let body = this.requests.get(storageKey);
    if (!body) {
      // Keep an uncertain request across a page reload. Failure to persist must
      // stop the mutation before it reaches the server.
      try {
        const requestId =
          this.outbox.get("mcpReconnect", suffix) || randomUUID();
        body = {
          id: this.agent,
          name,
          session: this.inventory.mcpSession,
          request_id: requestId,
        };
        this.outbox.set("mcpReconnect", suffix, requestId);
        this.requests.set(storageKey, body);
      } catch (error) {
        this.report(
          storageKey,
          `Cannot save the reconnect request. ${errorText(error)}`,
        );
        return;
      }
    }
    this.report(storageKey, "Reconnect in progress.", true);
    try {
      const result = yield this.reconnect(body);
      if (!this.active) return;
      this.report(storageKey, result.message);
      if (result.status !== "unknown") {
        try {
          this.outbox.remove("mcpReconnect", suffix);
          this.requests.delete(storageKey);
        } catch {
          /* A saved settled identity can still be replayed safely. */
        }
      }
      if (result.status === "completed") yield flowResult(this.load());
    } catch (error) {
      if (this.active) this.report(storageKey, errorText(error));
    }
  }
}

export const McpInventory = observer(function McpInventory({
  state,
  query,
}: Props) {
  const filtered = state.rows.filter((row) =>
    `${row.name} ${row.status} ${row.error}`
      .toLowerCase()
      .includes(query.toLowerCase()),
  );
  return (
    <div className="workspace-mcp">
      <Button
        variant="subtle"
        loading={state.pageBusy}
        onClick={() => void state.load()}
      >
        Refresh servers
      </Button>
      {!state.rows.length && (
        <p className="workspace-muted">No MCP servers returned.</p>
      )}
      {!!state.rows.length && !filtered.length && (
        <p className="workspace-muted">No matching MCP servers.</p>
      )}
      {!state.inventory?.mcpSession && !!state.rows.length && (
        <p className="workspace-error">
          No active session. Refresh the tools list before reconnect.
        </p>
      )}
      {filtered.map((row) => (
        <section
          className="workspace-mcp-server"
          key={row.name}
          aria-label={`MCP server ${row.name}`}
        >
          <strong>{row.name}</strong>
          <span>Status: {row.status}</span>
          {row.error && <p className="workspace-error">Error: {row.error}</p>}
          <Button
            size="xs"
            loading={!!state.feedback[state.requestKey(row.name)]?.busy}
            disabled={!state.inventory?.mcpSession}
            onClick={() => void state.reconnectServer(row.name)}
          >
            {state.requests.has(state.requestKey(row.name))
              ? "Check reconnect result"
              : "Reconnect"}
          </Button>
          {state.feedback[state.requestKey(row.name)] && (
            <p role="status">
              {state.feedback[state.requestKey(row.name)]?.message}
            </p>
          )}
        </section>
      ))}
      {state.pageError && (
        <p role="alert" className="workspace-error">
          {state.pageError}
        </p>
      )}
      {state.inventory?.serversCursor && (
        <Button
          variant="light"
          loading={state.pageBusy}
          onClick={() =>
            void state.load(state.inventory?.serversCursor ?? undefined)
          }
        >
          Load more servers
        </Button>
      )}
    </div>
  );
});
