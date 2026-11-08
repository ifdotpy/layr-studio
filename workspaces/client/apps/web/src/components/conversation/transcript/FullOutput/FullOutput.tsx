import { useMemo } from "react";
import { makeAutoObservable, observableRef, runInAction } from "mobx";
import { observer } from "mobx-react-lite";
import {
  createTextBlobMirror,
  type TextBlobResponse,
} from "../../../../generated/observableResources.ts";
import { Button } from "@mantine/core";
import { errorText } from "../../../../api.ts";
import { useServerApi } from "../../../servers/ServerServicesProvider/ServerServicesProvider.tsx";
import { fullOutputText } from "./fullOutputCache.ts";

/** Opens the full text of a cut item. It fetches by hash once, on the click. */
export default observer(function FullOutput({
  blob,
}: {
  blob: { hash: string; size: number };
}) {
  const { get } = useServerApi();
  const view = useMemo(() => {
    const state: {
      output: TextBlobResponse | null;
      loading: boolean;
      error: string;
    } = { output: null, loading: false, error: "" };
    return makeAutoObservable(state, { output: observableRef });
  }, [get, blob.hash, blob.size]);
  const open = async () => {
    runInAction(() => {
      view.loading = true;
      view.error = "";
    });
    try {
      const text = await fullOutputText(blob.hash, async (hash) => {
        const response = await get("/api/transcript/blob", {
          query: { hash },
          timeoutMs: 15000,
        });
        return response.text;
      });
      const output = createTextBlobMirror({
        hash: blob.hash,
        size: blob.size,
        text,
      }).current;
      runInAction(() => {
        view.output = output;
      });
    } catch (failure) {
      runInAction(() => {
        view.error = errorText(failure);
      });
    } finally {
      runInAction(() => {
        view.loading = false;
      });
    }
  };
  if (view.output) return <pre className="full-output">{view.output.text}</pre>;
  return (
    <>
      <Button
        size="compact-xs"
        variant="subtle"
        loading={view.loading}
        onClick={() => void open()}
      >
        Open full output ({Math.ceil(blob.size / 1024)} KB)
      </Button>
      {view.error && <p className="notice">{view.error}</p>}
    </>
  );
});
