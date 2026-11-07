import type { components } from "../../generated/api";
import { QuestionDecision } from "../../generated/api";
import type { JsonValue } from "../../types";
import type { RequestModel } from "./requestActions";
import { answerList, type AnswerValues } from "./AnswerFields";
import { requestQuestions, elicitationSchema } from "./requestData";
type QuestionCardAnswer = components["schemas"]["QuestionCardAnswer"];
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

export function questionAnswer(request: RequestModel, values: AnswerValues) {
  const questions = requestQuestions(request),
    schema = elicitationSchema(request);
  const properties = schema?.properties ?? {};
  const required = new Set(schema?.required ?? []);
  const kind = request.request.kind;
  let answer: QuestionCardAnswer;
  if (kind === "question" || kind === "agent_question") {
    answer = {
      method:
        kind === "question"
          ? "item/tool/requestUserInput"
          : "agent/asyncQuestion",
      data: {
        answers: Object.fromEntries(
          questions.map((q) => [
            q.id,
            { answers: answerList(q, values[q.id]) },
          ]),
        ),
      },
    };
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
    answer = {
      method: "mcpServer/elicitation/request",
      data: { decision: QuestionDecision.accept, content },
    };
  } else {
    throw new Error("Unsupported question type.");
  }

  return answer.data;
}
export function savedAnswer(request: RequestModel, values: AnswerValues) {
  return JSON.stringify(
    Object.fromEntries(
      requestQuestions(request)
        .filter((question) => !question.isSecret)
        .map((question) => [question.id, values[question.id] ?? ""]),
    ),
  );
}
