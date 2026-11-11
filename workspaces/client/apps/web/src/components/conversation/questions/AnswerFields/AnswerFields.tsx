import { observer } from "mobx-react-lite";
import { Owned } from "../../../ui/Owned/Owned.tsx";
import { AnswerSelection } from "../support/AnswerSelection.ts";
import { Checkbox, TextInput, Textarea, UnstyledButton } from "@mantine/core";

export type AnswerQuestion = {
  id: string;
  question: string;
  isSecret?: boolean;
  isOther?: boolean;
  multiSelect?: boolean;
  options?: { label: string; description?: string }[] | undefined;
};

export type AnswerValue = string | { selected: string[]; other: string };
export type AnswerValues = Record<string, AnswerValue>;

export function answerParts(value: AnswerValue | undefined) {
  return typeof value === "string"
    ? { selected: [], other: value }
    : value || { selected: [], other: "" };
}

export function answerList(
  question: AnswerQuestion,
  value: AnswerValue | undefined,
) {
  const { selected, other: draftOther } = answerParts(value);
  const other = question.options?.length && !question.isOther ? "" : draftOther;
  return question.multiSelect
    ? [...selected, ...(other.trim() ? [other.trim()] : [])]
    : [other.trim() || selected[0] || ""];
}

export type AnswerFieldsProps = {
  selection?: AnswerSelection;
  questions: AnswerQuestion[];
  values: AnswerValues;
  disabled?: boolean;
  onChange: (id: string, value: AnswerValue) => void;
};

export default function AnswerFields(props: AnswerFieldsProps) {
  if (props.selection)
    return <AnswerFieldsView {...props} selection={props.selection} />;
  return (
    <Owned create={() => new AnswerSelection(props.questions[0]?.id)}>
      {(selection) => <AnswerFieldsView {...props} selection={selection} />}
    </Owned>
  );
}
const AnswerFieldsView = observer(function AnswerFieldsView({
  selection,
  questions,
  values,
  disabled = false,
  onChange,
}: AnswerFieldsProps & { selection: AnswerSelection }) {
  const select = (question: AnswerQuestion, label: string) =>
    selection.select(question, label, values, onChange);
  return (
    <fieldset disabled={disabled} className="request-answer-fields">
      {questions.map((question, index) => {
        const value = answerParts(values[question.id]);
        return (
          <div
            className="answer-field"
            key={question.id}
            onFocusCapture={() => selection.activate(question.id)}
            onPointerDown={() => selection.activate(question.id)}
          >
            <p className="request-question-label">
              {questions.length > 1 && <span>{index + 1}. </span>}
              {question.question}
            </p>
            {question.options?.length ? (
              <div
                className="request-answer-options"
                role="group"
                aria-label={`${question.question} options`}
              >
                {question.options.map((option, optionIndex) => {
                  const copy = (
                    <span className="request-option-copy">
                      {optionIndex < 9 && (
                        <span
                          className="request-option-number"
                          aria-hidden="true"
                        >
                          {optionIndex + 1}
                        </span>
                      )}
                      <span>{option.label}</span>
                    </span>
                  );
                  return question.multiSelect ? (
                    <Checkbox
                      key={option.label}
                      className="request-answer-option request-answer-checkbox"
                      label={copy}
                      aria-label={option.label}
                      autoFocus={index === 0 && optionIndex === 0}
                      description={option.description}
                      checked={value.selected.includes(option.label)}
                      onChange={() => select(question, option.label)}
                    />
                  ) : (
                    <UnstyledButton
                      key={option.label}
                      type="button"
                      className="request-answer-option"
                      aria-pressed={value.selected.includes(option.label)}
                      aria-label={option.label}
                      autoFocus={index === 0 && optionIndex === 0}
                      onClick={() => select(question, option.label)}
                    >
                      {copy}
                      {option.description && (
                        <small>{option.description}</small>
                      )}
                    </UnstyledButton>
                  );
                })}
              </div>
            ) : null}
            {(!question.options?.length || question.isOther) &&
              (question.isSecret ? (
                <TextInput
                  aria-label={question.question}
                  autoFocus={index === 0 && !question.options?.length}
                  placeholder={question.options?.length ? "Other" : undefined}
                  type="password"
                  value={value.other}
                  onChange={(event) =>
                    onChange(question.id, event.target.value)
                  }
                />
              ) : (
                <Textarea
                  aria-label={question.question}
                  autoFocus={index === 0 && !question.options?.length}
                  value={value.other}
                  autosize
                  minRows={2}
                  maxRows={8}
                  onChange={(event) =>
                    onChange(question.id, {
                      selected: question.multiSelect ? value.selected : [],
                      other: event.target.value,
                    })
                  }
                  placeholder={
                    question.options?.length ? "Other" : "Your answer"
                  }
                />
              ))}
          </div>
        );
      })}
    </fieldset>
  );
});
