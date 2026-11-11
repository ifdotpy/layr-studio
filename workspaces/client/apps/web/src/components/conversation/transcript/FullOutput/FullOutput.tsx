import { makeAutoObservable, observableRef, onBecomeUnobserved } from "mobx";
import { observer } from "mobx-react-lite";
import { Button } from "@mantine/core";
import { errorText } from "../../../../api.ts";
import type { ServerApi } from "../../../../api.ts";
import { useServerApi } from "../../../servers/ServerServicesProvider/ServerServicesProvider.tsx";
import { fullOutputText } from "./fullOutputCache.ts";

class OutputRead {
  current: { text?: string; loading: boolean; error: string } = {
    loading: false,
    error: "",
  };
  constructor(release: () => void) {
    makeAutoObservable(this, { current: observableRef }, { autoBind: true });
    onBecomeUnobserved(this, "current", release);
  }
  *open(
    hash: string,
    get: ServerApi["get"],
  ): Generator<Promise<string>, void, string> {
    this.current = { loading: true, error: "" };
    try {
      const text = yield fullOutputText(hash, async (hash) => {
        const response = await get("/api/transcript/blob", {
          query: { hash },
          timeoutMs: 15000,
        });
        return response.text;
      });
      this.current = { text, loading: false, error: "" };
    } catch (failure) {
      this.current = { loading: false, error: errorText(failure) };
    }
  }
}
const outputs = new WeakMap<ServerApi["get"], Map<string, OutputRead>>();
function outputRead(get: ServerApi["get"], hash: string) {
  let entries = outputs.get(get);
  if (!entries) outputs.set(get, (entries = new Map()));
  let entry = entries.get(hash);
  if (!entry) {
    const cache = entries;
    entries.set(hash, (entry = new OutputRead(() => cache.delete(hash))));
  }
  return entry;
}

/** Opens the full text of a cut item. It fetches by hash once, on the click. */
export default observer(function FullOutput({
  blob,
}: {
  blob: { hash: string; size: number };
}) {
  const { get } = useServerApi();
  const read = outputRead(get, blob.hash);
  const view = read.current;
  if (view?.text !== undefined)
    return <pre className="full-output">{view.text}</pre>;
  return (
    <>
      <Button
        size="compact-xs"
        variant="subtle"
        loading={view?.loading}
        onClick={() => void read.open(blob.hash, get)}
      >
        Open full output ({Math.ceil(blob.size / 1024)} KB)
      </Button>
      {view?.error && <p className="notice">{view.error}</p>}
    </>
  );
});
