import { makeAutoObservable, observableRef } from "mobx";
import { errorText, type GetResult } from "../../../../api.ts";
import { flowValue } from "../../../../flowValue.ts";
import { resourceData, type ReadSlot } from "../../../../store/runtime.ts";
import type { ServerApi } from "../../../../servers/net/api.ts";
import type { createServerStore } from "../../../../store/server.ts";
import type { SearchSource } from "../support/SearchOverlay.tsx";

type Result = GetResult<"/api/search">["results"][number];
type Source = (Result | GetResult<"/api/search/item">) & { loading: boolean };

export class SearchStore {
  source: Source | null = null;
  sourceError = "";
  sourceRequest = 0;
  query = "";
  search = "";
  read: ReadSlot<GetResult<"/api/search">> | null = null;
  constructor() {
    makeAutoObservable(this, { read: observableRef }, { autoBind: true });
  }
  get state() {
    const current = this.read?.current;
    return {
      data: current ? resourceData(current) : undefined,
      loading: current?.status === "loading",
      error: current?.status === "error" ? errorText(current.error) : "",
    };
  }
  submit(api: ServerApi, store: ReturnType<typeof createServerStore>) {
    const q = this.query.trim();
    if (!q) return;
    this.search = q;
    this.read = store.reads.search(api, { q });
    void this.read.read().catch(() => {});
  }
  *loadSource(api: ServerApi, result: Result) {
    const request = ++this.sourceRequest;
    this.source = { ...result, loading: true };
    this.sourceError = "";
    try {
      const record = yield* flowValue(
        api.get("/api/search/item", { query: { id: result.id } }),
      );
      if (request === this.sourceRequest)
        this.source = { ...result, ...record, loading: false };
    } catch (error) {
      if (request === this.sourceRequest) {
        this.sourceError = errorText(error);
        this.source = { ...result, loading: false };
      }
    }
  }
  closeSource() {
    this.sourceRequest++;
    this.source = null;
  }
  openSource(actions: SearchSource) {
    const source = this.source;
    if (!source) return;
    if (source.kind === "plan") actions.navigate("plan", source.agent);
    else {
      actions.select(source.room || source.agent, source.id);
      actions.close();
    }
    this.closeSource();
  }
}
