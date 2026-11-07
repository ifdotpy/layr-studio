import { questionAnswer, savedAnswer as serializeAnswer } from "./answerData";
import { useEffect, useState } from "react";
import { observer } from "mobx-react-lite";
import { Button } from "@mantine/core";
import { errorText } from "../../api";
import { useServerStorage } from "../../servers/ServerServicesContext";
import { useUnsentText } from "../../unsent/useUnsentText";
import { requestDraftsForStorage } from "./requestDrafts";
import { elicitationSchema } from "./requestData";
import AnswerFields, {
  answerList,
  type AnswerValue,
  type AnswerValues,
} from "./AnswerFields";
import { requestRenderProbe, type RequestView } from "./requestActions";
export function AnswerForm({ view }: { view: RequestView }) {
  const request = view.request,
    draftKey = view.draftKey,
    sending = view.disabled;
  const { post, close, notify } = view;
  const answerDrafts = requestDraftsForStorage(useServerStorage());
  requestRenderProbe("answer", request.id);
  const [savedAnswer, saveAnswer] = useUnsentText("answer", draftKey);

  const [values, setValues] = useState<AnswerValues>(
    () => answerDrafts.get(draftKey) || {},
  );
  const questions = view.questions;
  useEffect(() => {
    if (!savedAnswer) return;
    try {
      const saved: unknown = JSON.parse(savedAnswer);
      if (!saved || typeof saved !== "object" || Array.isArray(saved)) return;
      const restored: AnswerValues = {};
      for (const question of questions) {
        if (question.isSecret || !(question.id in saved)) continue;
        const value = Reflect.get(saved, question.id);
        if (typeof value === "string") restored[question.id] = value;
        else if (
          value &&
          typeof value === "object" &&
          "other" in value &&
          typeof value.other === "string" &&
          "selected" in value &&
          Array.isArray(value.selected) &&
          value.selected.every((item: unknown) => typeof item === "string")
        )
          restored[question.id] = {
            other: value.other,
            selected: value.selected,
          };
      }
      setValues((current) => ({ ...current, ...restored }));
    } catch {
      /* Ignore a damaged answer. */
    }
  }, [savedAnswer]);
  const kind = request.request.kind;
  const schema = elicitationSchema(request);
  const elicitation = kind === "elicitation";
  const required = new Set(elicitation ? (schema?.required ?? []) : []);
  const missingRequired = [...required].some(
    (id) => !questions.some((question) => question.id === id),
  );
  const setValue = (id: string, value: AnswerValue) => {
    const next = { ...values, [id]: value };
    answerDrafts.set(draftKey, next);
    setValues(next);
    saveAnswer(serializeAnswer(view, next));
  };
  const ready =
    !missingRequired &&
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
        onChange={setValue}
      />
      <div className="request-answer-actions">
        <Button
          type="button"
          aria-label="Close reply"
          variant="subtle"
          disabled={sending}
          onClick={() => {
            close();
          }}
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
