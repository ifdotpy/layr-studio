import { useRef, useState } from "react";
import { ApiError, errorText } from "../../../api.ts";
import type { paths } from "../../../generated/api.ts";
import type { SidebarBackend } from "../../sidebar/services.tsx";
export type PeerTeamRequest =
  paths["/api/peer-teams"]["post"]["requestBody"]["content"]["application/json"];

/** Form and drag mutations retain one body and server until confirmed or reloaded. */
export function usePeerTeamMutation(
  refresh: (() => Promise<void>) | undefined,
  complete: (clear: () => void, reload: boolean) => void,
) {
  const request = useRef<PeerTeamRequest | null>(null);
  const backend = useRef<SidebarBackend | null>(null);
  const committed = useRef(false);
  const running = useRef(false);
  const [pending, setPending] = useState(false);
  const [error, setError] = useState("");
  const [rejected, setRejected] = useState(false);
  const clear = () => {
    request.current = null;
    backend.current = null;
    committed.current = false;
    setError("");
    setRejected(false);
  };
  const run = async (reload: boolean) => {
    if (running.current || (!reload && !request.current)) return;
    running.current = true;
    setPending(true);
    if (!reload) setError("");
    try {
      if (!reload && !committed.current && request.current) {
        if (!backend.current)
          throw new Error("The team server is unavailable.");
        await backend.current.post("/api/peer-teams", request.current, {
          timeoutMs: 15000,
        });
        committed.current = true;
      }
      await refresh?.();
      complete(clear, reload);
    } catch (e) {
      setError(errorText(e));
      if (!reload)
        setRejected(
          !committed.current &&
            e instanceof ApiError &&
            e.status >= 400 &&
            e.status < 500,
        );
    } finally {
      running.current = false;
      setPending(false);
    }
  };
  const submit = (body?: PeerTeamRequest, owner?: SidebarBackend) => {
    if (!request.current && body && owner) {
      request.current = body;
      backend.current = owner;
    }
    return run(false);
  };
  return {
    request,
    committed,
    pending,
    error,
    rejected,
    submit,
    reload: () => run(true),
    reportError: setError,
  };
}
