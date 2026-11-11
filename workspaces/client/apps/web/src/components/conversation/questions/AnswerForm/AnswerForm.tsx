import { observer } from "mobx-react-lite";
import { Button } from "@mantine/core";
import AnswerFields from "../AnswerFields/AnswerFields.tsx";
import {
  requestRenderProbe,
  type RequestView,
} from "../RequestArea/RequestArea.tsx";
export function AnswerForm({ view }: { view: RequestView }) {
  const { request, disabled: sending } = view;
  const { close, values, questions } = view;
  requestRenderProbe("answer", request.id);
  const ready = view.answerReady;
  const submit = (e: React.FormEvent) => {
    e.preventDefault();
    view.submitAnswer();
  };
  return (
    <form
      className="request-answer-form"
      aria-label="Reply to the agent"
      onSubmit={submit}
      onKeyDown={(e) => {
        if (!sending)
          view.answerSelection.keyDown(e, questions, values, view.setValue);
        if (e.key === "Escape") {
          e.stopPropagation();
          if (!sending) close();
        }
      }}
    >
      <AnswerFields
        selection={view.answerSelection}
        questions={questions}
        values={values}
        disabled={sending}
        onChange={view.setValue}
      />
      <div className="request-answer-actions">
        <Button
          type="button"
          aria-label="Close reply"
          variant="subtle"
          disabled={sending}
          onClick={close}
        >
          Answer later
        </Button>
        <Button
          type="submit"
          aria-label="Send answer"
          variant="filled"
          disabled={sending || !ready}
          loading={sending}
        >
          Send
        </Button>
      </div>
    </form>
  );
}

export default observer(AnswerForm);

export { savedAnswer } from "../support/answerData.ts";
