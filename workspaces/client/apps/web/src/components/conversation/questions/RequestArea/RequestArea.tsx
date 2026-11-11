import { AnswerSelection } from "../../support/AnswerSelection.ts";
import { CopyFeedback } from "../../../../stores/CopyFeedback.ts";
import { pasteBody, RESULT_SENT_NOTE } from "../../support/signInCardModel.ts";
import { elicitationSchema } from "../../support/requestData.ts";
import { answerList } from "../AnswerFields/AnswerFields.tsx";
import {
  questionAnswer,
  savedAnswer as serializeAnswer,
} from "../../support/answerData.ts";
import { createRef, type ReactNode } from "react";
import { reaction, flowResult, makeAutoObservable, observable } from "mobx";
import { ApiError, errorText } from "../../../../api.ts";
import { serverMutationResponseIsUncertain } from "../../../../servers/mutationIdentity.ts";
import type { ServerApi } from "../../../../api.ts";
import type { createServerStore } from "../../../../store/server.ts";
import { resourceData } from "../../../../store/runtime.ts";
import { requestDraftsForStorage } from "../support/requestDrafts.ts";
import { savedAnswer } from "../AnswerForm/AnswerForm.tsx";
import type {
  AnswerValue,
  AnswerValues,
} from "../AnswerFields/AnswerFields.tsx";
import {
  agentId,
  type RequestsCollections,
} from "../../../../generated/observableResources.ts";
import { reportUnsentFailure, unsentStore } from "../../../../unsent/store.ts";
import { nativeThreadError } from "../../../../nativeErrors.ts";
import {
  approvalParams,
  approvalThreadId,
  isDeferred,
} from "../../../../approvals.ts";
import type { Json } from "../../../../types.ts";
import { createArea } from "../../../../app/area.ts";
import { removeAnswerDraft } from "../support/requestDrafts.ts";
import {
  browserOpenDetails,
  isFieldRequest,
  requestQuestions,
} from "../../support/requestData.ts";
import {
  createRequestsView,
  type RequestScope,
} from "../../support/requestView.ts";
import { approveSignInPage } from "../support/openSignIn.ts";

export type RequestModel =
  RequestsCollections["requests"] extends ReadonlyMap<string, infer T>
    ? T
    : never;
export type RequestCommands = {
  scope: string;
  notify(message: string): void;
  onAnswerOpen?: (() => void) | undefined;
  onAnswerPosition?: ((node: HTMLElement) => void) | undefined;
};
const [Provider, useRequestOwner] = createArea<RequestAreaOwner>("Requests");
export { useRequestOwner };
export function RequestArea({
  owner,
  children,
}: {
  owner: RequestAreaOwner;
  children?: ReactNode;
}) {
  return <Provider value={owner}>{children}</Provider>;
}
export const answerKey = (
  scope: string,
  request: RequestModel,
  questions = requestQuestions(request),
) =>
  JSON.stringify([
    scope,
    request.agent,
    request.id,
    request.request.kind,
    questions,
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

export type RequestDependencies = {
  api(): ServerApi;
  store: ReturnType<typeof createServerStore>;
  storage: Storage;
  server: string;
};
/** Created and started by the selected conversation owner, never by React. */
export class RequestAreaOwner {
  readonly list;
  views = observable.map<string, RequestView>([], { deep: false });
  readonly dependencies: RequestDependencies;
  readonly commands: () => RequestCommands;
  #stop: (() => void) | undefined;
  constructor(
    dependencies: RequestDependencies,
    commands: () => RequestCommands,
    scope: RequestScope | (() => RequestScope) = { all: true },
  ) {
    this.list = createRequestsView(dependencies.store, scope);
    this.dependencies = dependencies;
    this.commands = commands;
    makeAutoObservable(
      this,
      {
        list: false,
        dependencies: false,
        commands: false,
      },
      { autoBind: true },
    );
  }
  start() {
    if (this.#stop) return;
    const { store, storage } = this.dependencies;
    const read = () => {
      const data = resourceData(store.requests().current);
      return data && [...data.requests.values()];
    };
    const receive = (requests: readonly RequestModel[] | undefined) => {
      if (!requests) return;
      const active = new Set(
        requests.map((request) => answerKey(this.commands().scope, request)),
      );
      for (const request of requests) {
        if (this.views.get(request.id)?.request === request) continue;
        this.views.get(request.id)?.dispose();
        this.views.set(request.id, createRequestView(this, request));
      }
      for (const [id, view] of this.views)
        if (!requests.some((request) => request.id === id)) {
          view.dispose();
          this.views.delete(id);
        }
      const drafts = requestDraftsForStorage(storage);
      for (const key of drafts.keys())
        if (JSON.parse(key)[0] === this.commands().scope && !active.has(key))
          drafts.delete(key);
    };
    receive(read());
    this.#stop = reaction(read, receive);
  }

  dispose() {
    this.#stop?.();
    this.#stop = undefined;
    for (const view of this.views.values()) view.dispose();
    this.views.clear();
  }
}
export function useRequestView(request: RequestModel) {
  return useRequestOwner().views.get(request.id)!;
}
function createRequestView(area: RequestAreaOwner, request: RequestModel) {
  const { api, store, storage, server } = area.dependencies;
  const commands = area.commands,
    scope = commands().scope;
  const drafts = requestDraftsForStorage(storage);
  const buffer = unsentStore(server, "answer");
  let stop: () => void;
  const value = {
    request,
    answerSelection: new AnswerSelection(),
    signInCopy: new CopyFeedback(),
    signInAddress: "",
    setSignInAddress(address: string) {
      this.signInAddress = address;
    },
    *pasteSignIn() {
      try {
        const text: string = yield navigator.clipboard.readText();
        if (text) {
          this.signInAddress = text.trim();
          return true;
        }
      } catch {
        /* The field remains available for a manual paste. */
      }
      return false;
    },
    *sendSignIn(): Generator<Promise<boolean>, void, boolean> {
      const body = pasteBody(this.signInAddress);
      if (body && !this.disabled && (yield this.post(body))) {
        this.signInAddress = "";
        this.notify(RESULT_SENT_NOTE);
      }
    },
    get answerReady() {
      const elicitation = request.request.kind === "elicitation";
      const required = new Set(elicitationSchema(request)?.required);
      return (
        [...required].every((id) => this.questions.some((q) => q.id === id)) &&
        (elicitation || this.questions.length > 0) &&
        this.questions
          .filter((q) => !elicitation || required.has(q.id))
          .every((q) =>
            answerList(q, this.values[q.id]).some((answer) => answer.trim()),
          )
      );
    },
    submitAnswer() {
      if (this.disabled || !this.answerReady) return;
      try {
        void this.post(
          questionAnswer(this, this.values),
          serializeAnswer(this, this.values),
        );
      } catch (error) {
        this.notify(errorText(error));
      }
    },
    card: createRef<HTMLDivElement>(),
    trigger: createRef<HTMLButtonElement>(),
    open: false,
    sending: false,
    error: "",
    mounted: true,
    *restore(key: string) {
      const questions = this.questions;
      const saved: string | undefined = yield buffer.get(key);
      if (this.mounted)
        drafts.set(key, {
          ...restoredAnswers(saved ?? null, questions),
          ...drafts.get(key),
        });
    },
    get values(): AnswerValues {
      return drafts.get(this.draftKey) ?? {};
    },
    setValue(id: string, value: AnswerValue) {
      const next = { ...this.values, [id]: value };
      drafts.set(this.draftKey, next);
      void buffer
        .put(this.draftKey, savedAnswer(this, next))
        .catch(reportUnsentFailure);
    },
    dispose() {
      this.mounted = false;
      stop();
    },
    get draftKey() {
      return answerKey(scope, request, this.questions);
    },
    get owner() {
      return resourceData(store.state().current)?.agents.get(request.agent);
    },
    get detail() {
      const owner = this.owner;
      if (!owner) return undefined;
      return resourceData(store.agentDetail(agentId(owner.id)).current);
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
    get questions() {
      return requestQuestions(request);
    },
    get stdinApproval() {
      return (
        request.request.kind === "exec" &&
        approvalParams(request).kind === "writeStdin"
      );
    },
    get question() {
      return isFieldRequest(request);
    },
    notify(text: string) {
      commands().notify(text);
    },
    close() {
      this.open = false;
      this.trigger.current?.focus();
    },
    toggle() {
      if (!this.open) commands().onAnswerOpen?.();
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
      const position = commands().onAnswerPosition;
      if (position) position(node);
      else
        transcript.scrollTop +=
          node.getBoundingClientRect().top -
          transcript.getBoundingClientRect().top -
          12;
    },
    post(body: Json, submitted?: string) {
      if (this.unavailable || this.blocked) {
        this.notify(
          this.unavailable
            ? "Chat details are not available yet."
            : "This chat stopped as a precaution. Open another chat.",
        );
        return Promise.resolve(false);
      }
      const approve = async () => {
        const response = await api().command.answer(request.id, body);
        if (response.kind !== "answer")
          throw new Error("Studio returned another result for an answer.");
        return response.result;
      };
      const operation =
        this.browserOpen && body.decision === "accept"
          ? () =>
              approveSignInPage({
                url: this.browserOpen!.url,
                requestId: request.id,
                owner: server,
                approve,
                onOpenError: (error) =>
                  this.notify(
                    `Studio approved the link, but could not confirm that its page opened: ${errorText(error)}`,
                  ),
              })
          : approve;
      return flowResult(this.mutate(operation, true, submitted));
    },
    defer(remove = false) {
      return flowResult(
        this.mutate(
          () =>
            remove
              ? api().post("/api/questions/delete", { id: request.id })
              : api().post("/api/questions/defer", {
                  id: request.id,
                  deferred: !isDeferred(request),
                }),
          remove,
        ),
      );
    },
    *mutate(
      operation: () => Promise<unknown>,
      remove: boolean,
      submitted?: string,
    ) {
      if (this.sending) return false;
      this.sending = true;
      this.error = "";
      const draftKey = this.draftKey;
      try {
        yield operation();
        if (remove)
          void removeAnswerDraft(storage, server, draftKey, submitted).catch(
            reportUnsentFailure,
          );
        if (this.mounted) this.open = false;
        return true;
      } catch (error) {
        if (
          submitted !== undefined &&
          error instanceof ApiError &&
          !serverMutationResponseIsUncertain(error.status, error.response)
        )
          void buffer
            .deleteIfEqual(draftKey, submitted)
            .catch(reportUnsentFailure);
        if (this.mounted) {
          this.error = errorText(error);
          this.notify(this.error);
        }
        return false;
      } finally {
        this.sending = false;
      }
    },
  };
  const view = makeAutoObservable(
    value,
    {
      request: false,
      card: false,
      trigger: false,
      mounted: false,
    },
    { autoBind: true },
  );
  stop = reaction(
    () => view.draftKey,
    (key) => {
      void flowResult(view.restore(key)).catch(reportUnsentFailure);
    },
    { fireImmediately: true },
  );
  return view;
}
export type RequestView = ReturnType<typeof createRequestView>;
function restoredAnswers(
  savedAnswer: string | null,
  questions: ReturnType<typeof requestQuestions>,
): AnswerValues {
  try {
    const saved: unknown = JSON.parse(savedAnswer || "null");
    if (!saved || typeof saved !== "object" || Array.isArray(saved)) return {};
    return Object.fromEntries(
      questions
        .filter((q) => !q.isSecret)
        .flatMap((q) => {
          const value = Reflect.get(saved, q.id);
          return typeof value === "string" ||
            (value &&
              typeof value === "object" &&
              typeof value.other === "string" &&
              Array.isArray(value.selected) &&
              value.selected.every((item: unknown) => typeof item === "string"))
            ? [[q.id, value]]
            : [];
        }),
    );
  } catch {
    return {};
  }
}
