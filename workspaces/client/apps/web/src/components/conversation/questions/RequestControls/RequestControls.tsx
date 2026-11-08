import { observer } from "mobx-react-lite";
import { Button } from "@mantine/core";
import {
  QuestionDecision,
  type components,
} from "../../../../generated/api.ts";
import { isDeferred } from "../../../../approvals.ts";
import {
  requestRenderProbe,
  type RequestView,
} from "../RequestArea/RequestArea.tsx";
export default observer(function RequestControls({
  view,
}: {
  view: RequestView;
}) {
  const r = view.request,
    question = view.question,
    approval = !question;
  const waiting = r.state.kind === "pending";
  const trigger = view.trigger;
  requestRenderProbe("controls", r.id);
  return (
    <div className="request-actions">
      {" "}
      {question && waiting && (
        <Button
          variant="subtle"
          className="request-delete"
          disabled={view.disabled}
          onClick={() => void view.defer(true)}
        >
          Delete question
        </Button>
      )}
      {view.blocked ? (
        <p>This chat stopped as a precaution. This request cannot resume it.</p>
      ) : !waiting ? null : question ? (
        <>
          {!view.open && (
            <Button
              variant="subtle"
              disabled={view.disabled}
              aria-label={isDeferred(r) ? "Restore" : "Defer"}
              onClick={() => void view.defer()}
            >
              {isDeferred(r) ? "Restore" : "Answer later"}
            </Button>
          )}
          <Button
            ref={trigger}
            data-answer={r.id}
            variant={view.open ? "subtle" : "filled"}
            {...(view.open ? { color: "gray" } : {})}
            disabled={view.disabled}
            aria-expanded={view.open}
            aria-controls={`answer-${r.id}`}
            onClick={() => {
              view.toggle();
            }}
          >
            {view.open ? "Hide" : "Answer"}
          </Button>
        </>
      ) : approval ? (
        <>
          <Button
            disabled={view.disabled}
            onClick={() =>
              void view.post({
                decision: QuestionDecision.decline,
              } satisfies components["schemas"]["QuestionApproval"])
            }
          >
            {view.stdinApproval ? "Decline input" : "Decline"}
          </Button>
          <Button
            variant="filled"
            disabled={view.disabled}
            onClick={() =>
              void view.post({
                decision: QuestionDecision.accept,
              } satisfies components["schemas"]["QuestionApproval"])
            }
          >
            {view.stdinApproval ? "Allow input" : "Approve"}
          </Button>
        </>
      ) : null}
    </div>
  );
});
