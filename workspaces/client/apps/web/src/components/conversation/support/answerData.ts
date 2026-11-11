import type { components } from "../../../generated/api.ts";
import { QuestionDecision } from "../../../generated/api.ts";
import type { JsonValue } from "../../../types.ts";
import type { RequestView } from "../questions/RequestArea/RequestArea.tsx";
import {
  answerList,
  type AnswerValues,
} from "../questions/AnswerFields/AnswerFields.tsx";
import { elicitationSchema } from "./requestData.ts";
function stringValue(value: unknown): string | undefined {
  return typeof value === "string" ? value : undefined;
}

function isJsonValue(value: unknown): value is JsonValue {
  if (
    value === null ||
    typeof value === "string" ||
    typeof value === "number" ||
    typeof value === "boolean"
  )
    return true;
  if (Array.isArray(value)) return value.every(isJsonValue);
  return typeof value === "object" && Object.values(value).every(isJsonValue);
}

function parseJson(text: string): JsonValue {
  const value: unknown = JSON.parse(text);
  if (!isJsonValue(value)) throw new Error("Enter a valid JSON value.");
  return value;
}

export function questionAnswer(
  { request, questions }: RequestView,
  values: AnswerValues,
) {
  const schema = elicitationSchema(request);
  const properties = schema?.properties ?? {};
  const required = new Set(schema?.required ?? []);
  const kind = request.request.kind;
  if (kind === "question" || kind === "agent_question") {
    return {
      answers: Object.fromEntries(
        questions.map((q) => [q.id, { answers: answerList(q, values[q.id]) }]),
      ),
    } satisfies components["schemas"]["QuestionSelections"];
  } else if (kind === "elicitation") {
    const content: components["schemas"]["QuestionElicitation"]["content"] = {};
    for (const [key, value] of Object.entries(properties)) {
      const schemaProperty = value;
      const type = stringValue(schemaProperty.type) || "string";
      const enumValues = schemaProperty.items?.enum ?? [];
      const question = questions.find((entry) => entry.id === key);
      if (!question) throw new Error("The requested answer fields changed.");
      const v = answerList(question, values[key]);
      if (!required.has(key) && !v.some((answer) => answer.trim())) continue;
      content[key] =
        type === "boolean"
          ? v[0] === "true"
          : ["integer", "number"].includes(type)
            ? Number(v[0])
            : type === "array" && enumValues.length > 0
              ? v
              : ["object", "array"].includes(type)
                ? parseJson(v[0] ?? "")
                : v[0];
    }
    return {
      decision: QuestionDecision.accept,
      content,
    } satisfies components["schemas"]["QuestionElicitation"];
  } else {
    throw new Error("Unsupported question type.");
  }
}
export function savedAnswer(view: RequestView, values: AnswerValues) {
  return JSON.stringify(
    Object.fromEntries(
      view.questions
        .filter((question) => !question.isSecret)
        .map((question) => [question.id, values[question.id] ?? ""]),
    ),
  );
}
