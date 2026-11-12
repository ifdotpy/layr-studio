import { flowResult } from "mobx";
import type { ServerServices } from "../../servers/serverServices.ts";
import { UnsentText } from "../../unsent/useUnsentText.ts";
import { FormDraft } from "./FormDraft.ts";
import type * as Api from "../../api.ts";
import { McpInventoryStore } from "../../components/shell/McpInventory/McpInventory.tsx";
import { WorkspaceResource } from "./WorkspaceResource.ts";

type Tools = {
  state: WorkspaceResource<Api.GetResult<"/api/capabilities">>;
  inventory: McpInventoryStore;
};

export class WorkspaceResources {
  readonly reads;
  private readonly forms = new Map<string, FormDraft>();
  private readonly text: UnsentText;
  form(key: string) {
    let form = this.forms.get(key);
    if (!form) {
      form = new FormDraft(this.text, key);
      this.forms.set(key, form);
    }
    return form;
  }
  private readonly states: { load(): unknown; dispose(): void }[] = [];
  private readonly tools = new Map<string, Tools>();
  readonly api: Pick<Api.ServerApi, "get" | "post">;
  readonly owner: string;
  private readonly watch: ServerServices["watchResourceReads"] | undefined;
  constructor(
    api: WorkspaceResources["api"],
    owner = "local",
    watch?: ServerServices["watchResourceReads"],
  ) {
    this.api = api;
    this.watch = watch;
    this.text = new UnsentText(owner);
    this.owner = owner;
    const read = <P extends Api.ApiGetPath>(path: P) => {
      const entries = new Map<string, WorkspaceResource<Api.GetResult<P>>>();
      return (query: Api.GetOptions<P>["query"]) => {
        const key = JSON.stringify(query ?? {});
        let state = entries.get(key);
        if (!state) {
          state = this.createRead(path, query);
          entries.set(key, state);
        }
        return state;
      };
    };
    this.reads = {
      "/api/changes": read("/api/changes"),
      "/api/checkpoints": read("/api/checkpoints"),
      "/api/profiles": read("/api/profiles"),
      "/api/rules": read("/api/rules"),
    };
  }
  private createRead<P extends Api.ApiGetPath>(
    path: P,
    query: Api.GetOptions<P>["query"],
    receive?: (data: Api.GetResult<P>) => void,
    watch?: WorkspaceResource<Api.GetResult<P>>["watch"],
  ) {
    const state = new WorkspaceResource(
      (signal, etag, readMetadata) =>
        this.api.get(path, { query, signal, etag, readMetadata }),
      receive,
      watch,
    );
    this.states.push(state);
    return state;
  }
  capabilities(agent: string, account?: string) {
    const watch = this.watch;
    const key = JSON.stringify([agent, account]);
    let entry = this.tools.get(key);
    if (!entry) {
      const inventory = new McpInventoryStore(
        this.owner,
        agent,
        async (cursor): Promise<Api.GetResult<"/api/capabilities"> | void> => {
          if (!cursor) {
            await flowResult(state.load());
            return;
          }
          return this.api.get("/api/capabilities", {
            query: {
              agent,
              serversCursor: cursor,
            },
          });
        },
        (body) => this.api.post("/api/mcp/reconnect", body),
      );
      const state: Tools["state"] = this.createRead(
        "/api/capabilities",
        { agent },
        inventory.receive,
        account && watch
          ? (read) =>
              watch(
                { kind: "native-catalog", accountKey: account },
                read,
                () => {},
                { waitForBaseline: true },
              )
          : undefined,
      );
      entry = { state, inventory };
      this.tools.set(key, entry);
    }
    return entry;
  }
  reload = () => {
    for (const state of this.states) void state.load();
  };
  dispose = () => {
    for (const state of this.states) state.dispose();
    for (const { inventory } of this.tools.values()) inventory.dispose();
  };
}
