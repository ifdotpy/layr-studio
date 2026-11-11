import { makeAutoObservable, observableShallow } from "mobx";
import { errorText, type ServerApi } from "../../../api.ts";
import { fullOutputText } from "./fullOutputCache.ts";
type View = { text?: string; loading: boolean; error: string };
/** One transcript owns its reads; entries are created only by an explicit open. */
export class FullOutputs {
  current = new Map<string, View>();
  constructor() {
    makeAutoObservable(
      this,
      { current: observableShallow },
      { autoBind: true },
    );
  }
  *open(
    hash: string,
    get: ServerApi["get"],
  ): Generator<Promise<string>, void, string> {
    if (this.current.get(hash)?.loading) return;
    this.current.set(hash, { loading: true, error: "" });
    try {
      const text = yield fullOutputText(hash, () =>
        get("/api/transcript/blob", { query: { hash }, timeoutMs: 15000 }).then(
          (response) => response.text,
        ),
      );
      this.current.set(hash, { text, loading: false, error: "" });
    } catch (failure) {
      this.current.set(hash, { loading: false, error: errorText(failure) });
    }
  }
}
