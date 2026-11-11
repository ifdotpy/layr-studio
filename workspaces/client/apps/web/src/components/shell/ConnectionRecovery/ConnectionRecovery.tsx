import { useServerApi } from "../../servers/ServerServicesProvider/ServerServicesProvider.tsx";
import { Owned } from "../../ui/Owned/Owned.tsx";
import { ConnectionRecoveryStore } from "./ConnectionRecoveryStore.ts";
import { Button } from "@mantine/core";

export default function ConnectionRecovery({ agentId }: { agentId: string }) {
  const { post } = useServerApi();

  return (
    <Owned create={() => new ConnectionRecoveryStore()}>
      {(state) => (
        <div className="native-error-actions studio-recovery-banner">
          <Button
            size="compact-xs"
            variant="light"
            loading={state.pending}
            disabled={state.pending}
            onClick={() => void state.check(post, agentId)}
          >
            Check connection
          </Button>
          {state.message && <p role="status">{state.message}</p>}
          {state.details && (
            <details>
              <summary>Details</summary>
              <p>{state.details}</p>
            </details>
          )}
        </div>
      )}
    </Owned>
  );
}
