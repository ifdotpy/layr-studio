import {
  useServerApi,
  useServerResources,
} from "../servers/ServerServicesContext";
import { useEffect, useEffectEvent, type RefObject } from "react";
import type { PostResult } from "../api";
import { subscribeProgress } from "../sync/progressSubscription";
import { currentClaudeLoginReadMatches } from "./claudeCodeSubmission";

type Receipt = PostResult<"/api/accounts/claude/login">;

export type ClaudeLoginProgressModel = {
  locked: boolean;
  requestId: string;
  readGeneration: number;
  refresh: () => void;
};

export function useClaudeLoginProgress(
  model: RefObject<ClaudeLoginProgressModel>,
  enabled: boolean,
  {
    apply,
    readFailed,
    failed,
  }: {
    apply: (result: Receipt) => void;
    readFailed?: (error: unknown) => void;
    failed: (error: unknown) => void;
  },
) {
  const { get } = useServerApi();
  const resources = useServerResources();
  const requestId = model.current.requestId;
  const applyReceipt = useEffectEvent(apply);
  const handleReadFailure = useEffectEvent((error: unknown) => {
    if (readFailed) readFailed(error);
    else throw error;
  });
  const handleFailure = useEffectEvent(failed);
  useEffect(() => {
    if (!requestId || !enabled) return;
    let live = true;
    const current = () =>
      live && !model.current.locked && model.current.requestId === requestId;
    const readStatus = async () => {
      if (model.current.locked) return;
      const generation = model.current.readGeneration;
      try {
        const result = await get("/api/accounts/claude/login", {
          query: { request_id: requestId },
        });
        if (
          current() &&
          currentClaudeLoginReadMatches(
            model.current.requestId,
            requestId,
            result.requestId,
            model.current.readGeneration,
            generation,
          )
        )
          applyReceipt(result);
      } catch (error) {
        if (current() && model.current.readGeneration === generation)
          handleReadFailure(error);
      }
    };
    const stop = subscribeProgress(
      resources,
      { kind: "auth-progress", requestId },
      (progress) => {
        if (
          progress.resource.kind !== "auth-progress" ||
          !progress.value ||
          typeof progress.value !== "object" ||
          progress.value.status === "duplicate" ||
          progress.value.status === "uncertain" ||
          !current()
        )
          return;
        model.current.readGeneration += 1;
        applyReceipt({ ...progress.value, status: progress.value.status });
      },
      readStatus,
      (error) => {
        if (live) handleFailure(error);
      },
    );
    const refresh = stop.refresh;
    model.current.refresh = refresh;
    return () => {
      live = false;
      stop();
      if (model.current.refresh === refresh) model.current.refresh = () => {};
    };
  }, [enabled, requestId, resources, get, model]);
}
