import { observer } from "mobx-react-lite";
import ErrorDescription from "../ErrorDescription";
import type { JsonValue } from "../../types";
import { requestApprovalDetails } from "./requestData";
import { requestRenderProbe, type RequestView } from "./requestActions";
type JsonObject = Record<string, JsonValue>;
function isJsonObject(value: unknown): value is JsonObject {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}
function permissionRows(
  value: unknown,
  name = "Permission",
): [string, string][] {
  // The requested names, when the provider sent no details.
  if (Array.isArray(value) && value.every((entry) => typeof entry === "string"))
    return value.map((entry): [string, string] => [entry, "Requested"]);
  if (isJsonObject(value)) {
    return Object.entries(value).flatMap(([key, entry]) =>
      permissionRows(entry, name === "Permission" ? key : `${name} / ${key}`),
    );
  }
  if (Array.isArray(value))
    return value.flatMap((entry, index) =>
      permissionRows(entry, `${name} ${index + 1}`),
    );
  return [
    [
      name.replace(/([a-z])([A-Z])/g, "$1 $2").replaceAll("_", " "),
      value === true
        ? "Allowed"
        : value === false
          ? "Not allowed"
          : value == null
            ? "None"
            : String(value),
    ],
  ];
}

export default observer(function RequestDescription({
  view,
}: {
  view: RequestView;
}) {
  const r = view.request,
    question = view.question,
    approval = !question;
  const { reason, message, command, cwd, files, permissions, url, tool } =
    requestApprovalDetails(r);
  const questions = view.questions;
  requestRenderProbe("description", r.id);
  return (
    <>
      {" "}
      {(!view.open || view.blocked || !question) &&
        (questions[0]?.question || reason || message || !approval) && (
          <p className="request-prompt">
            {questions[0]?.question || (
              <ErrorDescription
                value={reason || message || "The agent needs a reply."}
                role="status"
              />
            )}
          </p>
        )}
      {questions[0]?.question &&
        (reason || message) &&
        (reason || message) !== questions[0].question && (
          <p>
            <ErrorDescription value={reason || message} role="status" />
          </p>
        )}
      {!view.open && questions.length > 1 && (
        <p>{questions.length} questions</p>
      )}
      {!view.stdinApproval && command !== undefined && (
        <pre className="request-command">{command}</pre>
      )}
      {files !== undefined && files.length > 0 && (
        <pre className="request-command">{files.join("\n")}</pre>
      )}
      {tool !== undefined && <p className="request-cwd">Tool {tool}</p>}
      {cwd && (
        <p className="request-cwd" title={cwd}>
          in {cwd}
        </p>
      )}
      {permissions !== undefined && (
        <div className="request-permissions">
          <dl>
            {permissionRows(permissions).map(([name, value]) => (
              <div key={name}>
                <dt>{name}</dt>
                <dd title={value}>{value}</dd>
              </div>
            ))}
          </dl>
          <details>
            <summary>Permission details</summary>
            <pre>{JSON.stringify(permissions, null, 2)}</pre>
          </details>
        </div>
      )}
      {url && /^https?:\/\//.test(url) && (
        <a href={url} target="_blank" rel="noreferrer">
          Open request
        </a>
      )}
    </>
  );
});
