import { RecoveryStatus as GeneratedApiRecoveryStatus } from "../../../generated/api.ts";
import { useServerApi } from "../../servers/ServerServicesProvider/ServerServicesProvider.tsx";
import { useRef, useState } from "react";
import { Button } from "@mantine/core";
import { errorText } from "../../../api.ts";

export default function ConnectionRecovery({ agentId }: { agentId: string }) {
  const { post } = useServerApi();

  const [pending, setPending] = useState(false);
  const [message, setMessage] = useState("");
  const [details, setDetails] = useState("");
  const lock = useRef(false);
  const check = async () => {
    if (lock.current) return;
    lock.current = true;
    setPending(true);
    setMessage("");
    setDetails("");
    try {
      const result = await post(
        "/api/connection-recovery",
        { id: agentId },
        { timeoutMs: 100000 },
      );
      if (
        result.status === GeneratedApiRecoveryStatus.reconciled &&
        ["completed", "failed", "interrupted"].includes(result.outcome || "")
      ) {
        setMessage("Connection checked.");
      } else if (result.status === GeneratedApiRecoveryStatus.superseded) {
        setMessage(
          "The conversation changed while the connection was checked. Review its current state.",
        );
      } else {
        setDetails(result.error ? errorText(result.error) : "");
        setMessage(
          result.error
            ? "Could not confirm the previous outcome. Review the history before continuing."
            : "Connection checked. The previous outcome remains unconfirmed. Review the history before continuing.",
        );
      }
    } catch (error) {
      setMessage("Could not finish the connection check.");
      setDetails(errorText(error));
    } finally {
      lock.current = false;
      setPending(false);
    }
  };
  return (
    <div className="native-error-actions studio-recovery-banner">
      <Button
        size="compact-xs"
        variant="light"
        loading={pending}
        disabled={pending}
        onClick={() => void check()}
      >
        Check connection
      </Button>
      {message && <p role="status">{message}</p>}
      {details && (
        <details>
          <summary>Details</summary>
          <p>{details}</p>
        </details>
      )}
    </div>
  );
}
