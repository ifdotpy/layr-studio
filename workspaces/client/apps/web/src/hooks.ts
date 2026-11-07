import { autorun } from "mobx";
import { selectMessageConnection } from "./hooks/messageConnection";
import { useConversationHistory } from "./conversation/historySession";
import { useCacheAfterPaint } from "./sync/subscriptions/useCacheAfterPaint";
import {
  useServerApi,
  useServerSync,
  useServerResources,
} from "./servers/ServerServicesContext";
import {
  useCallback,
  useEffect,
  useLayoutEffect,
  useMemo,
  useRef,
  useState,
} from "react";
import { errorText, type PostResult } from "./api";
import "./sync/client";
import "./sync/transcriptCache";
import { ReadRecoveryBudget, retryableReadError } from "./sync/readRetry";
import {
  initialSnapshotProjectionStatus,
  snapshotProjectionError,
  snapshotProjectionStatusReducer,
  type SnapshotProjectionStatusAction,
} from "./hooks/snapshotProjectionStatus";
import type { UsageAccount } from "./components/Usage";
import type { UiPreferenceProjection } from "./sync/entityProjection";
import type { Agent, Snapshot } from "./types";
import {
  createChatSnapshotSelector,
  retainArray,
  retainUsageAccounts,
} from "./hooks/snapshotSelection";
const STATE_REF = { kind: "state" } as const;
// Event callbacks see the latest committed selection without invalidating
// consumers when another chat changes. Never call these during render.
export function useCommittedCallback<Args extends unknown[], Result>(
  callback: (...args: Args) => Result,
): (...args: Args) => Result {
  const current = useRef(callback);
  useLayoutEffect(() => {
    current.current = callback;
  });
  return useCallback((...args: Args) => current.current(...args), []);
}

export function useRetainedArray<T>(values: T[]): T[] {
  const previous = useRef(values);
  const selected = retainArray(previous.current, values);
  useLayoutEffect(() => {
    previous.current = selected;
  }, [selected]);
  return selected;
}

// A team change that leaves every account unchanged keeps the array identity.
export function useRetainedUsageAccounts(values: UsageAccount[]) {
  const previous = useRef(values);
  const selected = retainUsageAccounts(previous.current, values);
  useLayoutEffect(() => {
    previous.current = selected;
  }, [selected]);
  return selected;
}

export function useChatSnapshot(data: Snapshot | null, rootId?: string) {
  const select = useMemo(createChatSnapshotSelector, []);
  return select(data, rootId);
}

export function useSnapshot(
  acceptUiPreferences?: (preferences: UiPreferenceProjection) => void,
) {
  const { refreshSession, saved, save } = useServerApi();
  const { refreshProjection, subscribeStateProjection, syncDatabase } =
    useServerSync();
  const { watchResourceConnection } = useServerResources();

  const onUiPreferences = useCommittedCallback(
    acceptUiPreferences ?? (() => {}),
  );
  const [data, setData] = useState<Snapshot | null>(null);
  // A repeated report must not even call the owner's render: a reducer
  // dispatch runs the component body before it can bail out.
  const projectionStatusRef = useRef(initialSnapshotProjectionStatus);
  const [projectionStatus, setProjectionStatus] = useState(
    initialSnapshotProjectionStatus,
  );
  const dispatchProjectionStatus = useCallback(
    (action: SnapshotProjectionStatusAction) => {
      const next = snapshotProjectionStatusReducer(
        projectionStatusRef.current,
        action,
      );
      if (next === projectionStatusRef.current) return;
      projectionStatusRef.current = next;
      setProjectionStatus(next);
    },
    [],
  );
  const [workspaceId, setWorkspaceId] = useState("");
  const [created, setCreated] = useState<{ scope: string; agents: Agent[] }>(
    () => {
      return saved<{ scope: string; agents: Agent[] }>(
        "codex-confirmed-chats",
        { scope: "", agents: [] },
      );
    },
  );
  // Match the durable creation request's workspace scope.
  const scope = data?.stateDir || "";
  const currentScope = useRef(scope);
  currentScope.current = scope;
  const rememberCreated = useCallback(
    (agent: PostResult<"/api/leads">, expectedScope: string) => {
      if (currentScope.current !== expectedScope)
        throw new Error("The workspace changed before the new chat opened.");
      // A fresh draft has no native thread. Existing-thread retries await its projection.
      setCreated((old) => ({
        scope: expectedScope,
        agents: [
          ...(old.scope === expectedScope ? old.agents : []).filter(
            (a) => a.id !== agent.id,
          ),
          agent,
        ],
      }));
    },
    [],
  );
  const forgetCreated = useCallback((ids: string[]) => {
    setCreated((old) => ({
      ...old,
      agents: old.agents.filter((a) => !ids.includes(a.id)),
    }));
  }, []);
  useEffect(() => {
    if (!data || !created.agents.length) return;
    setCreated((old) => {
      if (old.scope !== scope)
        return old.agents.length ? { scope, agents: [] } : old;
      const remaining = old.agents.filter(
        (a) => !data?.threads.some((row) => row.id === a.id),
      );
      return remaining.length === old.agents.length
        ? old
        : { scope, agents: remaining };
    });
  }, [data, scope, created]);
  useEffect(() => {
    save("codex-confirmed-chats", created);
  }, [created]);
  const visibleData = useMemo(() => {
    if (!data || created.scope !== scope || !created.agents.length) return data;
    const missing = created.agents.filter(
      (a) => !data.threads.some((row) => row.id === a.id),
    );
    if (!missing.length) return data;
    const runtime = data.runtime;
    return {
      ...data,
      threads: [...data.threads, ...missing],
      runtime: {
        ...runtime,
        agents: [
          ...runtime.agents.filter(
            (a) => !missing.some((row) => row.id === a.id),
          ),
          ...missing,
        ],
      },
    };
  }, [data, created, scope]);
  useCacheAfterPaint(STATE_REF, visibleData);
  const generation = useRef(0);
  const replicated = useRef(false);
  const sessionToken = useRef("");
  const credentialRetry = useRef<ReturnType<typeof setTimeout> | undefined>(
    undefined,
  );
  const startupExhausted = useRef(false);
  const credentialRecovery = useRef(new ReadRecoveryBudget());
  const retryInitialization = useRef<(() => Promise<void>) | undefined>(
    undefined,
  );
  const refresh = useCallback(
    async (
      credentialsOnly = replicated.current,
      resetRecovery = true,
    ): Promise<void> => {
      if (resetRecovery) {
        startupExhausted.current = false;
        credentialRecovery.current = new ReadRecoveryBudget();
        if (retryInitialization.current) {
          await retryInitialization.current();
          return;
        }
      }
      if (credentialRecovery.current.exhaustedError) return;
      clearTimeout(credentialRetry.current);
      credentialRetry.current = undefined;
      const request = ++generation.current;
      try {
        const session = await refreshSession();
        if (request !== generation.current) return;
        if (request === generation.current) {
          sessionToken.current = session.token;
          setData((old) =>
            old && old.token !== session.token
              ? { ...old, token: session.token }
              : old,
          );
        }
        if (!credentialsOnly) {
          const projection = await refreshProjection({});
          if (request !== generation.current) return;
          setData({ ...projection, token: session.token });
        }
        if (request !== generation.current) return;
        startupExhausted.current = false;
        dispatchProjectionStatus({ type: "startup-recovered" });
      } catch (e) {
        if (request !== generation.current) return;
        let reported = e;
        let delay: number | undefined;
        if (retryableReadError(e)) {
          try {
            delay = credentialRecovery.current.nextDelay(e);
          } catch (exhausted) {
            reported = exhausted;
            startupExhausted.current = true;
          }
        }
        dispatchProjectionStatus({
          type: "startup-failed",
          error: errorText(reported),
        });
        if (
          delay !== undefined &&
          !document.hidden &&
          navigator.onLine !== false
        ) {
          credentialRetry.current = setTimeout(() => {
            credentialRetry.current = undefined;
            if (
              request === generation.current &&
              !document.hidden &&
              navigator.onLine !== false
            )
              void refresh(credentialsOnly, false);
          }, delay);
        }
      }
    },
    [],
  );
  useEffect(() => {
    let stopped = false;
    let recovery = new ReadRecoveryBudget();
    let initialized = false;
    let retry: ReturnType<typeof setTimeout> | undefined;
    const initialize = async () => {
      if (stopped || recovery.exhaustedError) return;
      let storage: Awaited<ReturnType<typeof syncDatabase>>;
      try {
        storage = await syncDatabase();
      } catch (error) {
        if (stopped) return;
        let reported = error;
        let delay: number | undefined;
        if (retryableReadError(error)) {
          try {
            delay = recovery.nextDelay(error);
          } catch (exhausted) {
            reported = exhausted;
            startupExhausted.current = true;
          }
        }
        dispatchProjectionStatus({
          type: "startup-failed",
          error: errorText(reported),
        });
        if (delay !== undefined)
          retry = setTimeout(() => void initialize(), delay);
        return;
      }
      if (stopped) return;
      initialized = true;
      retryInitialization.current = undefined;
      setWorkspaceId(storage.workspaceId);
      try {
        await refresh(true, false);
      } catch (error) {
        if (stopped) return;
        dispatchProjectionStatus({
          type: "startup-failed",
          error: errorText(error),
        });
      }
    };
    const restart = async () => {
      clearTimeout(retry);
      recovery = new ReadRecoveryBudget();
      startupExhausted.current = false;
      credentialRecovery.current = new ReadRecoveryBudget();
      if (initialized) await refresh(true, false);
      else await initialize();
    };
    retryInitialization.current = restart;
    let interrupted = false;
    const stopConnection = watchResourceConnection(
      (status, activity, detail) => {
        dispatchProjectionStatus({
          type: "transport-changed",
          error:
            status === "degraded" ||
            status === "offline" ||
            activity === "offline"
              ? selectMessageConnection(status, activity, detail).message
              : "",
        });
        if (status === "degraded" || status === "offline") interrupted = true;
        if (status === "live" && interrupted) {
          interrupted = false;
          void restart();
        }
      },
    );
    void initialize();
    return () => {
      stopped = true;
      clearTimeout(retry);
      generation.current++;
      clearTimeout(credentialRetry.current);
      credentialRetry.current = undefined;
      retryInitialization.current = undefined;
      stopConnection();
    };
  }, [refresh]);
  useEffect(
    () =>
      subscribeStateProjection(
        (next) => {
          if (next) {
            replicated.current = true;
            dispatchProjectionStatus({
              type: startupExhausted.current
                ? "projection-recovered"
                : "data-received",
            });
            setData({ ...next, token: sessionToken.current });
          }
        },
        (error) => {
          dispatchProjectionStatus(
            error === null
              ? { type: "projection-recovered" }
              : { type: "projection-failed", error: errorText(error) },
          );
        },
        acceptUiPreferences ? onUiPreferences : undefined,
      ),
    [],
  );
  return {
    data: visibleData,
    error: snapshotProjectionError(projectionStatus),
    transportError: projectionStatus.transportError,
    refresh,
    workspaceId,
    rememberCreated,
    forgetCreated,
    creationScope: scope,
  };
}
// Compatibility callers outside W3 retain an immutable hook result. W3 roots
// consume the scoped session directly and observe model fields only in leaves.
export { transcriptMessages } from "./conversation/messageView";
export function useMessages(
  id: string | null,
  kind: "agent" | "room" | "legacy",
  managed = false,
  workspace = "",
  syncWorkspaceId = "",
  enabled = true,
) {
  const session = useConversationHistory(
    id,
    kind,
    managed,
    workspace,
    syncWorkspaceId,
    enabled,
  );
  const read = () => ({
    session,
    loaded: session.loaded,
    items: session.items.map((item) => ({ ...item })),
    notice: session.notice,
    before: session.before,
    after: session.after,
    historyVersion: session.historyVersion,
    historical: session.historical,
    pageLoading: session.pageLoading,
    connection: session.connection,
    older: session.older,
    newer: session.newer,
    setPageAnchor: session.setPageAnchor,
    showLatest: session.showLatest,
    ensureMessage: session.ensureMessage,
    reload: session.reload,
  });
  const [snapshot, setSnapshot] = useState(read);
  useEffect(() => autorun(() => setSnapshot(read())), [session]);
  return snapshot.session === session ? snapshot : read();
}
