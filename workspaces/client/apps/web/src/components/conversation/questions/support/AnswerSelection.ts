import type { KeyboardEvent } from "react";
import { makeAutoObservable } from "mobx";
import {
  answerParts,
  type AnswerQuestion,
  type AnswerValues,
  type AnswerValue,
} from "../AnswerFields/AnswerFields.tsx";

/** Keyboard focus and option selection for an answer form. */
export class AnswerSelection {
  active: string | undefined;
  constructor(active?: string) {
    this.active = active;
    makeAutoObservable(this, {}, { autoBind: true });
  }
  activate(id: string) {
    this.active = id;
  }
  keyDown(
    event: KeyboardEvent,
    questions: AnswerQuestion[],
    values: AnswerValues,
    change: (id: string, value: AnswerValue) => void,
  ) {
    if (
      event.altKey ||
      event.ctrlKey ||
      event.metaKey ||
      event.repeat ||
      !/^[1-9]$/.test(event.key) ||
      (event.target instanceof HTMLElement &&
        event.target.closest(
          'textarea, input:not([type="checkbox"]):not([type="radio"]), [contenteditable="true"]',
        ))
    )
      return;
    const question = questions.find(
      (q) => q.id === (this.active ?? questions[0]?.id),
    );
    const option = question?.options?.[Number(event.key) - 1];
    if (question && option) {
      event.preventDefault();
      this.select(question, option.label, values, change);
    }
  }
  select(
    question: AnswerQuestion,
    label: string,
    values: AnswerValues,
    change: (id: string, value: AnswerValue) => void,
  ) {
    const value = answerParts(values[question.id]);
    change(
      question.id,
      question.multiSelect
        ? {
            ...value,
            selected: value.selected.includes(label)
              ? value.selected.filter((entry) => entry !== label)
              : [...value.selected, label],
          }
        : { selected: [label], other: "" },
    );
  }
}
