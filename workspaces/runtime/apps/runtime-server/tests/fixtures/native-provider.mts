#!/usr/bin/env node
import fs from "node:fs";
import type {
  ClientRequestResults,
  ServerNotification,
  ThreadResumeResponse,
} from "../../../../../providers/apps/claude-bridge/codex-generated/protocol.ts";

import type { JsonValue } from "../../../../../client/apps/web/src/generated/api.ts";
import net from "node:net";
import os from "node:os";
import path from "node:path";
import readline from "node:readline";

// Requests come from the parent Rust witness; retain its JSON payload unchanged.
type FixtureRequest = Record<string, JsonValue> & { id?: JsonValue } & (
    | {
        method: "thread/resume" | "turn/start";
        params: Record<string, JsonValue> & {
          threadId: string;
          clientInfo?: Record<string, JsonValue>;
        };
      }
    | {
        method: "initialize" | "initialized" | "model/list" | "fixture/barrier";
        params?: Record<string, JsonValue> & {
          clientInfo?: Record<string, JsonValue>;
        };
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
let turnCount = 0;
fs.writeFileSync(path.join(root, "startup-cwd"), process.cwd());
async function reportSocket(socketPath: string, value: JsonValue) {
  await new Promise<void>((resolve, reject) => {
    const socket = net.createConnection(socketPath);
    socket.once("error", reject);
    socket.once("connect", () => socket.end(`${JSON.stringify(value)}\n`));
    // Flush our report without waiting for the observer to finish its test.
    socket.once("finish", () => {
      socket.destroy();
      resolve();
    });
  });
}
async function report(name: string, value: JsonValue) {
  const socketPath = path.join(root, `${name}.sock`);
  if (fs.existsSync(socketPath)) await reportSocket(socketPath, value);
}
function increment(name: string) {
  const file = path.join(root, name);
  const count = fs.existsSync(file)
    ? Number(fs.readFileSync(file, "utf8")) + 1
    : 1;
  fs.writeFileSync(file, String(count));
  return count;
}
function notify<M extends ServerNotification["method"]>(
  method: M,
  params: Extract<ServerNotification, { method: M }>["params"],
) {
  console.log(JSON.stringify({ method, params }));
}
if (process.env.FIXTURE_CWD_SOCKET) {
  await reportSocket(process.env.FIXTURE_CWD_SOCKET, {
    pid: process.pid,
    cwd: process.cwd(),
  });
}
for await (const line of readline.createInterface({ input: process.stdin })) {
  const request = parseFixtureRequest(line);
  const method = request.method;
  const accountProvider =
    request.params?.clientInfo?.name === "codex_agents_canvas";
  if (method === "initialize" && accountProvider) {
    const count = increment("account-launch-count");
    await report("launch", { pid: process.pid, count });
    if (count === 1 && fs.existsSync(path.join(root, "crash-first-initialize")))
      process.exit(17);
  }
  if (!Object.hasOwn(request, "id")) continue;
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
    turnCount = increment("turn-count");
    await report("turn", {
      pid: process.pid,
      cwd: process.cwd(),
      count: turnCount,
      priority: os.getPriority(),
      request,
    });
    if (turnCount === 1 && fs.existsSync(path.join(root, "hang-first-turn"))) {
      await new Promise(() => {});
    }
    result = {
      turn: { id: "fixture-turn", status: "inProgress", items: [] },
    } satisfies ClientRequestResults["turn/start"];
  } else if (method === "fixture/barrier") {
    await reportSocket(path.join(root, "turn.sock"), {
      method,
      count: turnCount,
    });
    result = { count: turnCount };
  } else if (method === "model/list") {
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
  }
  console.log(JSON.stringify({ id: request.id, result }));
  if (
    method === "initialize" &&
    accountProvider &&
    fs.existsSync(path.join(root, "emit-generation-notification"))
  ) {
    console.log(
      JSON.stringify({
        method: "fixture/generation",
        params: { pid: process.pid },
      }),
    );
  }
  if (
    method === "turn/start" &&
    process.env.STUDIO_W7_P01_COMPLETE_TURN === "1"
  ) {
    const threadId = request.params.threadId,
      turnId = "fixture-turn",
      itemId = "fixture-assistant-item";
    notify("turn/started", {
      threadId,
      turn: { id: turnId, status: "inProgress", items: [] },
    });
    notify("item/started", {
      startedAtMs: Date.now(),
      threadId,
      turnId,
      item: { id: itemId, type: "agentMessage", text: "" },
    });
    notify("item/agentMessage/delta", {
      threadId,
      turnId,
      itemId,
      delta: "fixture assistant response",
    });
    notify("item/completed", {
      completedAtMs: Date.now(),
      threadId,
      turnId,
      item: {
        id: itemId,
        type: "agentMessage",
        text: "fixture assistant response",
      },
    });
    notify("turn/completed", {
      threadId,
      turn: { id: turnId, status: "completed", items: [] },
    });
  }
}
