import { createRef, useEffect, useMemo, useRef, type ReactNode } from "react";
import { makeAutoObservable, runInAction } from "mobx";
import { ApiError, errorText } from "../../api";
import { serverMutationResponseIsUncertain } from "../../servers/mutationIdentity";
import {
  useServerApi,
  useServerServices,
  useServerStorage,
  useServerStore,
} from "../../servers/ServerServicesContext";
import {
  agentId,
  type RequestsCollections,
} from "../../generated/observableResources";
import { serverViewId } from "../../servers/environment";
import { reportUnsentFailure, unsentStore } from "../../unsent/store";
import { nativeThreadError } from "../../nativeErrors";
import { approvalThreadId, isDeferred } from "../../approvals";
import type { Json } from "../../types";
import { createArea } from "../area";
import { removeAnswerDraft } from "./requestDrafts";
import {
  browserOpenDetails,
  isFieldRequest,
  requestQuestions,
} from "./requestData";
import { approveSignInPage } from "./openSignIn";

export type RequestModel =
  RequestsCollections["requests"] extends ReadonlyMap<string, infer T>
    ? T
    : never;
type RequestCommands = {
  scope: string;
  refresh(): Promise<void>;
  notify(message: string): void;
  onAnswerOpen?: (() => void) | undefined;
  onAnswerPosition?: ((node: HTMLElement) => void) | undefined;
};
const [Provider, useRequestActions] = createArea<RequestCommands>("Requests");
export const useRequestScope = useRequestActions;
export function RequestArea({
  children,
  ...commands
}: RequestCommands & { children?: ReactNode }) {
  return <Provider value={commands}>{children}</Provider>;
}
export const answerKey = (scope: string, request: RequestModel) =>
  JSON.stringify([
    scope,
    request.agent,
    request.id,
    request.request.kind,
    requestQuestions(request).map(
      ({ id, question, isSecret, isOther, multiSelect, options }) => ({
        id,
        question,
        isSecret,
        isOther,
        multiSelect,
        options,
      }),
    ),
  ]);
export function requestRenderProbe(leaf: string, id?: string) {
  if (typeof window !== "undefined")
    window.__studioRequestRenderProbe?.(leaf, id);
}
declare global {
  interface Window {
    __studioRequestRenderProbe?: (leaf: string, id?: string) => void;
  }
}

export function useRequestView(request: RequestModel) {
  const commands = useRequestActions(),
    api = useServerApi(),
    store = useServerStore();
  const services = useServerServices(),
    storage = useServerStorage();
  const latest = useRef(commands);
  latest.current = commands;
  const scope = commands.scope;
  const view = useMemo(() => {
    const server = services?.owner.id ?? serverViewId ?? "local";
    const value = {
      request,
      card: createRef<HTMLDivElement>(),
      trigger: createRef<HTMLButtonElement>(),
      open: false,
      sending: false,
      error: "",
      pending: false,
      mounted: true,
      get draftKey() {
        return answerKey(scope, request);
      },
      get owner() {
        const current = store.state().current;
        return current.status === "live" || current.status === "stale"
          ? current.data.agents.get(request.agent)
          : undefined;
      },
      get detail() {
        const owner = this.owner;
        if (!owner) return undefined;
        const current = store.agentDetail(agentId(owner.id)).current;
        return current.status === "live" || current.status === "stale"
          ? current.data
          : undefined;
      },
      get unavailable() {
        return !!this.owner && !this.detail;
      },
      get blocked() {
        return (
          !!nativeThreadError(this.owner, this.detail) &&
          approvalThreadId(request) === this.owner?.threadId
        );
      },
      get disabled() {
        return this.sending || this.unavailable;
      },
      get browserOpen() {
        return browserOpenDetails(request);
      },
      get question() {
        return isFieldRequest(request);
      },
      notify(text: string) {
        latest.current.notify(text);
      },
      close() {
        this.open = false;
        this.trigger.current?.focus();
      },
      toggle() {
        if (!this.open) latest.current.onAnswerOpen?.();
        this.open = !this.open;
      },
      position(node: HTMLElement) {
        const transcript = node.closest<HTMLElement>("#messages");
        const requests = node.closest<HTMLElement>("#requests");
        if (!node.querySelector(".request-answer-form") || !transcript) return;
        if (requests) {
          const cardBounds = node.getBoundingClientRect(),
            requestsBounds = requests.getBoundingClientRect();
          if (
            cardBounds.top < requestsBounds.top ||
            cardBounds.top >= requestsBounds.bottom
          )
            requests.scrollTop += cardBounds.top - requestsBounds.top - 12;
        }
        if (latest.current.onAnswerPosition)
          latest.current.onAnswerPosition(node);
        else
          transcript.scrollTop +=
            node.getBoundingClientRect().top -
            transcript.getBoundingClientRect().top -
            12;
      },
      async post(body: Json, submitted?: string): Promise<boolean> {
        if (this.unavailable) {
          this.notify("Chat details are not available yet.");
          return false;
        }
        if (this.blocked) {
          this.notify("This chat stopped as a precaution. Open another chat.");
          return false;
        }
        if (this.pending) return false;
        this.pending = true;
        this.sending = true;
        this.error = "";
        const draftKey = this.draftKey;
        try {
          const approve = async () => {
            const response = await api.command.answer(request.id, body);
            if (response.kind !== "answer")
              throw new Error("Studio returned another result for an answer.");
            return response.result;
          };
          if (this.browserOpen && body.decision === "accept")
            await approveSignInPage({
              url: this.browserOpen.url,
              requestId: request.id,
              owner: services?.owner.id,
              approve,
              onOpenError: (error) =>
                this.notify(
                  `Studio approved the link, but could not confirm that its page opened: ${errorText(error)}`,
                ),
            });
          else await approve();
          void removeAnswerDraft(storage, server, draftKey, submitted).catch(
            reportUnsentFailure,
          );
          if (!this.mounted) return true;
          runInAction(() => {
            this.open = false;
          });
          await latest.current.refresh();
          return true;
        } catch (error) {
          if (
            submitted !== undefined &&
            error instanceof ApiError &&
            !serverMutationResponseIsUncertain(error.status, error.response)
          )
            void unsentStore(server, "answer")
              .deleteIfEqual(draftKey, submitted)
              .catch(reportUnsentFailure);
          if (this.mounted) {
            runInAction(() => {
              this.error = errorText(error);
            });
            this.notify(errorText(error));
          }
          return false;
        } finally {
          runInAction(() => {
            this.pending = false;
            if (this.mounted) this.sending = false;
          });
        }
      },
      async defer(remove = false) {
        if (this.pending) return;
        this.pending = true;
        this.sending = true;
        this.error = "";
        const draftKey = this.draftKey;
        try {
          if (remove) {
            await api.post("/api/questions/delete", { id: request.id });
            void removeAnswerDraft(storage, server, draftKey).catch(
              reportUnsentFailure,
            );
          } else
            await api.post("/api/questions/defer", {
              id: request.id,
              deferred: !isDeferred(request),
            });
          if (this.mounted)
            runInAction(() => {
              this.open = false;
            });
          await latest.current.refresh();
        } catch (error) {
          if (this.mounted) {
            runInAction(() => {
              this.error = errorText(error);
            });
            this.notify(errorText(error));
          }
        } finally {
          runInAction(() => {
            this.pending = false;
            if (this.mounted) this.sending = false;
          });
        }
      },
    };
    return makeAutoObservable(
      value,
      {
        request: false,
        card: false,
        trigger: false,
        pending: false,
        mounted: false,
      },
      { autoBind: true },
    );
  }, [request, store, storage, scope]);
  useEffect(() => {
    view.mounted = true;
    return () => {
      view.mounted = false;
    };
  }, [view]);
  return view;
}
export type RequestView = ReturnType<typeof useRequestView>;
