import {
  questionAnswer,
  savedAnswer as serializeAnswer,
} from "./answerData.ts";
import { observer } from "mobx-react-lite";
import { Button } from "@mantine/core";
import { errorText } from "../../../../api.ts";
import { elicitationSchema } from "../support/requestData.ts";
import AnswerFields, { answerList } from "../AnswerFields/AnswerFields.tsx";
import {
  requestRenderProbe,
  type RequestView,
} from "../RequestArea/RequestArea.tsx";
export function AnswerForm({ view }: { view: RequestView }) {
  const { request, disabled: sending } = view;
  const { post, close, notify, values, questions } = view;
  requestRenderProbe("answer", request.id);
  const elicitation = request.request.kind === "elicitation";
  const required = new Set(elicitationSchema(request)?.required);
  const ready =
    [...required].every((id) =>
      questions.some((question) => question.id === id),
    ) &&
    (elicitation || questions.length > 0) &&
    questions
      .filter((q) => !elicitation || required.has(q.id))
      .every((q) =>
        answerList(q, values[q.id]).some((answer) => answer.trim()),
      );
  const submit = (e: React.FormEvent) => {
    e.preventDefault();
    if (sending || !ready) return;
    try {
      void post(questionAnswer(view, values), serializeAnswer(view, values));
    } catch (e) {
      notify(errorText(e));
    }
  };
  return (
    <form
      className="request-answer-form"
      aria-label="Reply to the agent"
      onSubmit={submit}
      onKeyDown={(e) => {
        if (e.key === "Escape") {
          e.stopPropagation();
          if (!sending) close();
        }
      }}
    >
      <AnswerFields
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

export { savedAnswer } from "./answerData.ts";
