import { observer } from "mobx-react-lite";
import { MessageCircleQuestion, ShieldQuestion } from "lucide-react";
import { useEffect } from "react";
import {
  approvalCreated,
  isAgentQuestion,
  isDeferred,
} from "../../../../approvals.ts";
import MessageDate from "../../transcript/MessageDate/MessageDate.tsx";
import ErrorDescription from "../../../shell/ErrorDescription/ErrorDescription.tsx";
import AnswerForm from "../AnswerForm/AnswerForm.tsx";
import SignInCard from "../SignInCard/SignInCard.tsx";
import RequestDescription from "../RequestDescription/RequestDescription.tsx";
import RequestControls from "../RequestControls/RequestControls.tsx";
import {
  useRequestView,
  requestRenderProbe,
  type RequestView,
  type RequestModel,
} from "../RequestArea/RequestArea.tsx";
const RequestOwner = observer(function RequestOwner({
  view,
}: {
  view: RequestView;
}) {
  requestRenderProbe("owner", view.request.id);
  return <strong>{view.owner?.name || "Codex"}</strong>;
});
const RequestStatus = observer(function RequestStatus({
  view,
}: {
  view: RequestView;
}) {
  const r = view.request,
    waiting = r.state.kind === "pending",
    question = view.question;
  const asynchronous = isAgentQuestion(r);
  const stateNote = {
    pending: "",
    answering: "Sending your answer",
    answered: "Answered",
    unknown: "The agent may not have got your answer",
    closed: "Closed",
  }[r.state.kind];
  requestRenderProbe("status", r.id);
  return (
    <span
      className={
        "request-status" + (question && waiting ? " request-status-hidden" : "")
      }
    >
      {!waiting
        ? stateNote
        : isDeferred(r)
          ? asynchronous
            ? "Deferred · Agent can continue"
            : "Deferred · Agent still waits"
          : asynchronous
            ? "Agent can continue"
            : question
              ? "Answer required"
              : view.stdinApproval
                ? "Terminal input approval required"
                : "Approval required"}
    </span>
  );
});
const RequestSubmitStatus = observer(function RequestSubmitStatus({
  view,
}: {
  view: RequestView;
}) {
  return (
    view.error && (
      <div className="studio-recovery-banner" role="alert">
        <p>Could not update this request.</p>
        <details>
          <summary>Details</summary>
          <ErrorDescription value={view.error} />
        </details>
      </div>
    )
  );
});
const InlineAnswer = observer(function InlineAnswer({
  view,
}: {
  view: RequestView;
}) {
  const open = view.open;
  useEffect(() => {
    const node = view.card.current;
    if (!open || !node) return;
    const transcript = node.closest<HTMLElement>("#messages"),
      requests = node.closest<HTMLElement>("#requests");
    if (!node.querySelector(".request-answer-form") || !transcript) return;
    const position = () => view.position(node);
    const observer = new ResizeObserver(position);
    observer.observe(transcript);
    if (requests) observer.observe(requests);
    position();
    const frame = requestAnimationFrame(position);
    return () => {
      cancelAnimationFrame(frame);
      observer.disconnect();
    };
  }, [view, open]);
  return (
    !view.blocked &&
    view.question &&
    open && (
      <div id={"answer-" + view.request.id} className="request-inline-answer">
        <AnswerForm key={view.draftKey} view={view} />
      </div>
    )
  );
});
const RequestSignIn = observer(function RequestSignIn({
  view,
}: {
  view: RequestView;
}) {
  return view.blocked ? (
    <p>This chat stopped as a precaution. This request cannot resume it.</p>
  ) : (
    view.browserOpen && <SignInCard view={view} />
  );
});
export const RequestCard = observer(function RequestCard({
  request,
  messageId,
  showDates = true,
}: {
  request: RequestModel;
  messageId?: string;
  showDates?: boolean | undefined;
}) {
  const view = useRequestView(request);
  const browserOpen = !!view.browserOpen;
  const Icon = view.question ? MessageCircleQuestion : ShieldQuestion;
  requestRenderProbe("card", request.id);
  return (
    <div
      className={
        "request request-card" +
        (browserOpen
          ? " request-sign-in"
          : isAgentQuestion(request)
            ? " request-async"
            : "")
      }
      ref={view.card}
      data-request={request.id}
      data-message={messageId}
      onFocusCapture={(event) => {
        const target = event.target;
        if (target instanceof HTMLElement)
          target
            .closest<HTMLElement>(".request-answer-option")
            ?.scrollIntoView({ block: "nearest" });
      }}
    >
      {browserOpen ? (
        <>
          {showDates && <MessageDate at={approvalCreated(request)} />}
          <RequestSignIn view={view} />
        </>
      ) : (
        <>
          <div className="request-summary">
            <Icon size={18} className="request-icon" />
            <div className="request-copy">
              <div className="request-heading">
                <RequestOwner view={view} />
                <RequestStatus view={view} />
              </div>
              {showDates && <MessageDate at={approvalCreated(request)} />}
              <RequestDescription view={view} />
            </div>
            <RequestControls view={view} />
          </div>
          <RequestSubmitStatus view={view} />
          <InlineAnswer view={view} />
        </>
      )}
    </div>
  );
});
/** Open the answer form of a pending request card, as its own Answer button does. */
export function openRequestAnswer(requestId: string): boolean {
  const trigger = [
    ...document.querySelectorAll<HTMLButtonElement>("#requests [data-answer]"),
  ].find((button) => button.dataset.answer === requestId);
  if (!trigger) return false;
  const later = trigger.closest("details");
  if (later) later.open = true;
  if (trigger.getAttribute("aria-expanded") === "true") {
    trigger.scrollIntoView({ block: "nearest" });
    trigger.focus();
  } else trigger.click();
  return true;
}
