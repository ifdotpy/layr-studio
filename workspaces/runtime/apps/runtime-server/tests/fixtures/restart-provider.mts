#!/usr/bin/env node
// A scripted Codex app-server for the backend restart witness. It records
// every request in requests.jsonl, keeps turns running, holds command/exec
// until the test writes release-exec, and completes a turn of a thread when
// the test writes complete-<threadId>. It runs under the real supervisor, so
// it outlives the backend that started it.
import fs from "node:fs";
import type {
  ClientRequestResults,
  ServerNotification,
  ThreadResumeResponse,
} from "../../../../../integrations/providers/claude/bridge/codex-generated/protocol.ts";

import type { JsonValue } from "../../../../../client/apps/web/src/generated/api.ts";
import path from "node:path";
import readline from "node:readline";

// Requests come from the parent Rust witness; retain its JSON payload unchanged.
type FixtureRequest = Record<string, JsonValue> & { id?: JsonValue } & (
    | { method: "command/exec"; params: { processId: string } }
    | {
        method: "thread/resume" | "turn/start";
        params: { threadId: string };
      }
    | {
        method: "turn/interrupt";
        params: { threadId: string; turnId: string };
      }
    | {
        method:
          | "initialize"
          | "initialized"
          | "config/read"
          | "model/list"
          | "thread/backgroundTerminals/clean";
        params?: Record<string, JsonValue>;
      }
  );
function parseFixtureRequest(line: string) {
  return JSON.parse(line) as FixtureRequest;
}
function resumedThread(threadId: string): ThreadResumeResponse {
  return {
    thread: {
      id: threadId,
      sessionId: "fixture-session",
      preview: "",
      modelProvider: "openai",
      createdAt: 0,
      updatedAt: 0,
      cwd: process.cwd(),
      cliVersion: "fixture",
      ephemeral: false,
      projectId: null,
      source: "appServer",
      status: { type: "idle" },
      turns: [],
    },
    model: "fake-model",
    modelProvider: "openai",
    cwd: process.cwd(),
    approvalPolicy: "never",
    approvalsReviewer: "user",
    sandbox: { type: "dangerFullAccess" },
  };
}
const root = import.meta.dirname;
const log = path.join(root, "requests.jsonl");
const pendingExec = new Map<JsonValue, string>();
const runningTurns = new Map<string, string>();
fs.appendFileSync(path.join(root, "provider-pids"), `${process.pid}\n`);

function send(value: unknown) {
  process.stdout.write(`${JSON.stringify(value)}\n`);
}
function notify<M extends ServerNotification["method"]>(
  method: M,
  params: Extract<ServerNotification, { method: M }>["params"],
) {
  send({ method, params });
}
function exists(name: string) {
  return fs.existsSync(path.join(root, name));
}

function flushReleasedWork() {
  if (exists("release-exec")) {
    for (const [id, processId] of pendingExec) {
      pendingExec.delete(id);
      fs.appendFileSync(path.join(root, "exec-released"), `${processId}\n`);
      send({
        id,
        result: {
          exitCode: 3,
          stdout: "",
          stderr: "",
        } satisfies ClientRequestResults["command/exec"],
      });
    }
  }
  for (const [threadId, turnId] of runningTurns) {
    if (!exists(`complete-${threadId}`)) continue;
    runningTurns.delete(threadId);
    const itemId = `answer-${turnId}`;
    notify("item/completed", {
      completedAtMs: Date.now(),
      threadId,
      turnId,
      item: { id: itemId, type: "agentMessage", text: "fixture answer" },
    });
    notify("turn/completed", {
      threadId,
      turn: { id: turnId, status: "completed", items: [] },
    });
  }
}
// The parent releases work by creating marker files. Subscribe before reading
// requests and recheck after admission, so either ordering observes the marker.
fs.watch(root, flushReleasedWork);

readline.createInterface({ input: process.stdin }).on("line", (line) => {
  const request = parseFixtureRequest(line);
  // The trailing newline publishes a complete request to the Rust witness.
  fs.appendFileSync(log, `${line}\n`);
  if (!("id" in request)) return;
  const method = request.method;

  if (method === "command/exec") {
    pendingExec.set(request.id, request.params.processId);
    flushReleasedWork();
    return;
  }
  let result: unknown = {};
  if (method === "initialize")
    result = {
      codexHome: process.env.CODEX_HOME ?? root,
      platformFamily: "fixture",
      platformOs: "fixture",
      userAgent: "fixture",
    } satisfies ClientRequestResults["initialize"];
  else if (method === "thread/resume")
    result = resumedThread(request.params.threadId);
  else if (method === "turn/start") {
    const { threadId } = request.params;
    const turnId = `turn-${threadId}`;
    runningTurns.set(threadId, turnId);
    result = {
      turn: { id: turnId, status: "inProgress", items: [] },
    } satisfies ClientRequestResults["turn/start"];
    send({ id: request.id, result });
    notify("turn/started", {
      threadId,
      turn: { id: turnId, status: "inProgress", items: [] },
    });
    flushReleasedWork();
    return;
  } else if (method === "turn/interrupt") {
    const { threadId, turnId } = request.params;
    runningTurns.delete(threadId);
    send({
      id: request.id,
      result: {} satisfies ClientRequestResults["turn/interrupt"],
    });
    notify("turn/completed", {
      threadId,
      turn: { id: turnId, status: "interrupted", items: [] },
    });
    return;
  } else if (method === "config/read")
    result = {
      config: {},
      origins: {},
    } satisfies ClientRequestResults["config/read"];
  else if (method === "model/list")
    result = {
      data: [
        {
          id: "fake-model",
          model: "fake-model",
          displayName: "Fixture",
          description: "Scripted fixture model",
          hidden: false,
          isDefault: true,
          supportedReasoningEfforts: [],
          defaultReasoningEffort: "medium",
        },
      ],
    } satisfies ClientRequestResults["model/list"];
  send({ id: request.id, result });
});
