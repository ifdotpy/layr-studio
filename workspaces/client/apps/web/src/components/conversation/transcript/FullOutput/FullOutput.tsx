import { observer } from "mobx-react-lite";
import { Button } from "@mantine/core";
import { useServerApi } from "../../../servers/ServerServicesProvider/ServerServicesProvider.tsx";
import type { FullOutputs } from "../../support/FullOutputs.ts";

/** Opens the full text of a cut item. It fetches by hash once, on the click. */
export default observer(function FullOutput({
  blob,
  owner,
}: {
  blob: { hash: string; size: number };
  owner: FullOutputs;
}) {
  const { get } = useServerApi();
  const view = owner.current.get(blob.hash);
  if (view?.text !== undefined)
    return <pre className="full-output">{view.text}</pre>;
  return (
    <>
      <Button
        size="compact-xs"
        variant="subtle"
        loading={view?.loading ?? false}
        onClick={() => void owner.open(blob.hash, get)}
      >
        Open full output ({Math.ceil(blob.size / 1024)} KB)
      </Button>
      {view?.error && <p className="notice">{view.error}</p>}
    </>
  );
});
