import { WorkspaceResourceState as ResourceState } from "../WorkspaceResourceState/WorkspaceResourceState.tsx";
import {
  date,
  type PanelContext,
} from "../WorkspaceSection/WorkspaceSection.tsx";
import { Button, Modal, Textarea } from "@mantine/core";
import { action } from "mobx";
import { observer } from "mobx-react-lite";
import { requestId as randomUUID } from "studio-claude-bridge/request-id.ts";
import ErrorDescription from "../ErrorDescription/ErrorDescription.tsx";
import {
  section,
  WorkspaceSection as Empty,
} from "../WorkspaceSection/WorkspaceSection.tsx";

export const WorkspaceChanges = section(
  observer(function WorkspaceChanges(c: PanelContext<never>) {
    const state = c.model.changeState;
    const ui = c.panel;
    const { comment, busy: saving } = ui;
    const { report, files, annotations, rows } = c.model;
    return (
      <>
        <ResourceState state={state} />
        {state.data && !report ? (
          <Empty>Chat changes require the updated server.</Empty>
        ) : report?.git === false ? (
          <Empty>
            <ErrorDescription
              value={report.error || "Could not read reported changes."}
            />
          </Empty>
        ) : (
          <>
            <div className="workspace-toolbar">
              <span className="workspace-muted">
                {files.length} reported {files.length === 1 ? "file" : "files"}
                {report?.reportedAt && <> · {date(report.reportedAt)}</>}
              </span>
            </div>
            {report?.truncated && (
              <p className="workspace-muted">
                Showing the first 300,000 characters of the reported diff.
              </p>
            )}
            <div className="workspace-file-list">
              {files.map((file) => (
                <Button
                  key={file.path}
                  size="xs"
                  variant="subtle"
                  onClick={() =>
                    c.preview({ agent: c.selected!.id, path: file.path })
                  }
                >
                  <code>{file.status}</code>&nbsp; {file.path}
                </Button>
              ))}
            </div>
            {report?.diff ? (
              <div
                className="workspace-diff"
                role="region"
                aria-label="Changes diff"
              >
                {rows.map((row, index) => (
                  <div
                    className={`workspace-diff-line ${row.headerPath ? "file-header" : !row.commentable && !row.text.startsWith("@@") ? "patch-metadata" : ""} ${row.text.startsWith("+") ? "addition" : row.text.startsWith("-") ? "deletion" : row.text.startsWith("@@") ? "hunk" : ""}`}
                    key={index}
                  >
                    {row.headerPath ? (
                      <>
                        <code className="workspace-diff-status">
                          {files.find((file) => file.path === row.headerPath)
                            ?.status || "M"}
                        </code>
                        <span>{row.headerPath}</span>
                      </>
                    ) : (
                      <>
                        <button
                          type="button"
                          disabled={!row.commentable}
                          aria-label={
                            row.commentable
                              ? `Comment on ${row.path} line ${row.line}`
                              : undefined
                          }
                          title={
                            row.commentable
                              ? `Comment on line ${row.line}`
                              : undefined
                          }
                          onClick={action(
                            () =>
                              (ui.comment = {
                                path: row.path,
                                line: row.line,
                                ...(typeof report?.turnId === "string"
                                  ? { turnId: report.turnId }
                                  : {}),
                                text: "",
                                id: randomUUID(),
                              }),
                          )}
                        >
                          {row.commentable ? row.line : ""}
                        </button>
                        <code>{row.text || " "}</code>
                      </>
                    )}
                  </div>
                ))}
              </div>
            ) : (
              state.data !== null && (
                <Empty>
                  {files.length
                    ? "Choose a file to inspect its contents. No tracked text diff is available."
                    : "No file changes."}
                </Empty>
              )
            )}
          </>
        )}
        {!!annotations.length && (
          <section className="workspace-result">
            <h3>Comments sent to this agent</h3>
            {annotations.map((entry) => (
              <article key={entry.id}>
                <small>
                  {entry.path}:{entry.line}
                </small>
                <p className="workspace-prose">{entry.text}</p>
              </article>
            ))}
          </section>
        )}
        <Modal
          opened={!!comment}
          onClose={action(() => {
            if (!ui.busy) ui.comment = null;
          })}
          title={comment ? `${comment.path}:${comment.line}` : "Line comment"}
        >
          {comment && (
            <form
              className="workspace-form"
              onSubmit={(e) => {
                e.preventDefault();
                void ui.saveComment(c);
              }}
            >
              <Textarea
                label="Comment to the agent"
                required
                minRows={4}
                value={comment.text}
                onChange={action(
                  (e) => (ui.comment = { ...comment, text: e.target.value }),
                )}
              />
              <Button variant="filled" type="submit" loading={saving}>
                Send comment
              </Button>
            </form>
          )}
        </Modal>
      </>
    );
  }),
);
