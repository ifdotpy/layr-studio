import { Window } from "happy-dom";
import { spawn } from "node:child_process";
import { once } from "node:events";
import { mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath, URL as NodeURL } from "node:url";
import { createInterface } from "node:readline";
import { z } from "zod";
import { requestId as randomUUID } from "studio-claude-bridge/request-id.ts";
import {
  API_SCHEMA_HASH,
  API_SCHEMA_HASH_HEADER,
} from "./generated/apiSchema.ts";
import {
  errorPayload,
  NetworkTimeoutError,
  type PostBody,
  type PostResult,
} from "./api.ts";
// @vitest-environment happy-dom
import { autorun } from "mobx";
import { transientTestFixture } from "./sync/subscriptions/transientTestFixture.ts";
import { agentId, requestId } from "./generated/observableResources.ts";
import { resourceData } from "./store/runtime.ts";
import { afterEach, expect, it, vi } from "vitest";
import { renameOperation } from "./renameRequest.ts";
import { createRequestRegistry } from "./servers/requestOperations.ts";
import { postDouble } from "./servers/postDouble.ts";
afterEach(() => {
  vi.useRealTimers();
  vi.unstubAllGlobals();
});
it("a pending rename sends one POST and waits for the entity stream", async () => {
  vi.useFakeTimers();
  const f = transientTestFixture();
  const post = vi
    .fn<(body: PostBody<"/api/rename">) => Promise<PostResult<"/api/rename">>>()
    .mockResolvedValue({ id: "agent", status: "pending" });
  const registry = createRequestRegistry(
    localStorage,
    { post: postDouble({ "/api/rename": post }) },
    () => f.store,
    vi.fn(),
  );
  try {
    const entry = registry.start(
      renameOperation.intent(["workspace", "rename", "agent"], { id: "agent" }),
    );
    await entry.completion;
    await vi.advanceTimersByTimeAsync(90_000);
    expect(entry.state).toBe("confirmed");
    expect(post).toHaveBeenCalledTimes(1);
  } finally {
    registry.dispose();
    f.close();
    localStorage.clear();
  }
});
it("a lost response preserves uncertainty without disturbing the entity stream", async () => {
  const f = transientTestFixture();
  const post = vi
    .fn<(body: PostBody<"/api/rename">) => Promise<PostResult<"/api/rename">>>()
    .mockRejectedValue(new TypeError("response lost"));
  const registry = createRequestRegistry(
    localStorage,
    { post: postDouble({ "/api/rename": post }) },
    () => f.store,
    vi.fn(),
  );
  try {
    const entry = registry.start(
      renameOperation.intent(["workspace", "rename", "agent"], { id: "agent" }),
    );
    await entry.completion;
    expect(entry.state).toBe("unknown");
    expect(post).toHaveBeenCalledTimes(1);
  } finally {
    registry.dispose();
    f.close();
    localStorage.clear();
  }
});

it("delivers request-scoped rename failure through the generated facade without HTTP", async () => {
  vi.useFakeTimers();
  const f = transientTestFixture();
  const slot = f.store.renameProgress(agentId("agent"), requestId("request"));
  const stop = autorun(() => {
    void slot.current;
  });
  try {
    await vi.advanceTimersByTimeAsync(100);
    const ref = {
      kind: "rename-progress",
      agentId: "agent",
      requestId: "request",
    } as const;
    const value = {
      id: "agent",
      request_id: "request",
      status: "failed",
      error: "No title",
    } as const;
    f.emit(ref, {
      epoch: "one",
      revision: 1,
      progress: { resource: ref, value },
    });
    await vi.advanceTimersByTimeAsync(0);
    expect(resourceData(slot.current)).toEqual(value);
  } finally {
    stop();
    f.close();
  }
});

// Runs the actual manual handler and request-results replay store in disposable state.
it("manual HTTP success, lost reply replay and definitive rejection follow the real server contract", async () => {
  const directory = await mkdtemp(join(tmpdir(), "studio-rename-contract-"));
  const binary =
    process.env.STUDIO_DRAFT_SERVER_BINARY ??
    fileURLToPath(
      new NodeURL(
        "../../../../../target/debug/examples/draft-server",
        import.meta.url,
      ),
    );
  const server = spawn(binary, [directory, "--entity-live"], {
    stdio: ["pipe", "pipe", "inherit"],
  });
  const lines = createInterface({ input: server.stdout! });
  const f = transientTestFixture();
  const proof = vi.spyOn(f.store, "renameProgress");
  let registry: ReturnType<typeof createRequestRegistry> | undefined;
  try {
    const [line] = await Promise.race([
      once(lines, "line", { signal: AbortSignal.timeout(30_000) }),
      once(server, "exit").then(([code]) => {
        throw new Error(`Rename fixture exited: ${code}`);
      }),
    ]);
    const ready = z
      .object({ port: z.number(), workspaceId: z.string() })
      .parse(JSON.parse(line));
    lines.close();
    const origin = `http://127.0.0.1:${ready.port}`;
    const browser = new Window({ url: origin });
    const headers = {
      "content-type": "application/json",
      "x-canvas-token": "draft-witness",
      "x-canvas-workspace": ready.workspaceId,
      [API_SCHEMA_HASH_HEADER]: API_SCHEMA_HASH,
    };
    const seed = await browser.fetch(`${origin}/fixture/entity`, {
      method: "POST",
      headers,
      body: JSON.stringify({
        agent: {
          id: "lead",
          rootId: "lead",
          name: "Before",
          compactions: 0,
          accountKey: "default",
          provider: "codex",
          epoch: 0,
          status: "idle",
          isLead: true,
          autoWake: true,
          cwd: "/project",
        },
      }),
    });
    expect(seed.ok).toBe(true);
    const responseSchema = z.object({
      id: z.string(),
      name: z.string(),
      request_id: z.string(),
      status: z.literal("applied"),
    });
    const replies: PostResult<"/api/rename">[] = [];
    let loseReply = false;
    const post = vi.fn(
      async (
        body: PostBody<"/api/rename">,
      ): Promise<PostResult<"/api/rename">> => {
        const response = await browser.fetch(`${origin}/api/rename`, {
          method: "POST",
          headers: { ...headers, "X-Studio-Request-Id": body.request_id || "" },
          body: JSON.stringify(body),
        });
        const value: unknown = await response.json();
        if (!response.ok) throw errorPayload(value, response.status);
        const reply = responseSchema.parse(value);
        replies.push(reply);
        if (loseReply) {
          loseReply = false;
          throw new NetworkTimeoutError();
        }
        return reply;
      },
    );
    registry = createRequestRegistry(
      localStorage,
      { post: postDouble({ "/api/rename": post }) },
      () => f.store,
      vi.fn(),
    );
    const key = [ready.workspaceId, "rename", "lead"] as const;
    const first = registry.start(
      renameOperation.intent(key, { id: "lead", name: "First" }),
    );
    await first.completion;
    expect(replies[0]).toMatchObject({
      id: "lead",
      name: "First",
      request_id: first.requestId,
      status: "applied",
    });
    expect(first.state).toBe("applied");
    expect(registry.get(first.requestId)).toBeUndefined();
    loseReply = true;
    const lost = registry.start(
      renameOperation.intent(key, { id: "lead", name: "Saved" }),
    );
    await lost.completion;
    expect(lost.state).toBe("unknown");
    // A later entity edit must not replace the exact reply stored for the lost request.
    await post({
      id: "lead",
      name: "Later",
      request_id: randomUUID(),
    });
    await registry.retry(lost.requestId);
    expect(lost.state).toBe("applied");
    expect(post.mock.calls[3]).toEqual(post.mock.calls[1]);
    expect(replies[3]).toEqual(replies[1]);
    expect(registry.get(lost.requestId)).toBeUndefined();
    const rejected = registry.start(
      renameOperation.intent(key, { id: "missing", name: "No agent" }),
    );
    await rejected.completion;
    expect(rejected.state).toBe("failed");
    expect(registry.get(rejected.requestId)).toBeUndefined();
    expect(proof).not.toHaveBeenCalled();
  } finally {
    registry?.dispose();
    f.close();
    lines.close();
    localStorage.clear();
    if (server.exitCode === null && server.signalCode === null) {
      const exited = once(server, "exit", {
        signal: AbortSignal.timeout(10_000),
      });
      server.stdin?.end();
      try {
        await exited;
      } catch {
        const killed = once(server, "exit");
        server.kill("SIGKILL");
        await killed;
      }
    }
    await rm(directory, { recursive: true, force: true });
  }
}, 30_000);
