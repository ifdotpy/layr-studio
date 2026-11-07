import { approval, browserOpen } from "../../approvalFixtures";
import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import Requests, { RequestArea } from "./Requests";
import { requestFixture } from "./requestFixtures";
import type { RequestView } from "./requestActions";
import type { Approval } from "../../approvals";
import { required } from "../componentTestFixtures";

const controls = vi.hoisted(() => ({
  cards: [] as Array<{ post(body: Record<string, unknown>): Promise<boolean> }>,
  post: vi.fn(),
  answer: vi.fn(),
  storage: {},
  store: null as ReturnType<typeof requestFixture>["store"] | null,
  write: vi.fn(() => null),
}));
vi.mock("../../servers/ServerServicesContext", () => ({
  useServerStore: () => controls.store,
  useServerApi: () => ({
    post: controls.post,
    command: { answer: controls.answer },
  }),
  useServerServices: () => ({ owner: { id: "remote", label: "Build server" } }),
  useServerLocalDrafts: () => ({ writeLocalDraft: controls.write }),
  useServerStorage: () => controls.storage,
}));
vi.mock("../../api", () => ({
  ApiError: class extends Error {},
  errorText: (error: unknown) =>
    String(error instanceof Error ? error.message : error),
}));
vi.mock("./SignInCard", () => ({
  default: ({ view }: { view: RequestView }) => {
    controls.cards.push(view);
    return null;
  },
}));

const requestId = "633882fe-ad7e-468d-bf47-4252b89fb2db";
const link = "https://identity.example.test/authorize?client_id=fixture";
const forward = {
  server: "remote",
  bridgeId: "a".repeat(32),
  host: "localhost" as const,
  port: 8250,
  path: "/callback",
  expiresAt: 1_900_000_000,
  token: "fixture-only",
};

function card() {
  const notify = vi.fn();
  const refresh = vi.fn(async () => {});
  const request: Approval = approval({
    id: requestId,
    agent: "agent",
    request: browserOpen(link, {
      bridgeId: forward.bridgeId,
      host: "identity.example.test",
      source: "host",
      redirect: { host: "localhost", port: 8250, path: "/callback" },
      expiresAt: forward.expiresAt,
    }),
  });
  controls.store = requestFixture([request]).store;
  renderToStaticMarkup(
    createElement(
      RequestArea,
      { scope: "remote", notify, refresh },
      createElement(Requests, { all: true }),
    ),
  );
  expect(controls.cards).toHaveLength(1);
  return { post: required(controls.cards[0]).post, notify, refresh };
}

beforeEach(() => {
  controls.cards.length = 0;
  controls.post.mockReset();
  controls.answer.mockReset();
  controls.write.mockClear();
  vi.stubGlobal("window", {});
});
afterEach(() => {
  controls.store?.dispose();
  vi.unstubAllGlobals();
});

it("the actual request card prepares native sign-in before a slow approval and opens once", async () => {
  let elapsed = 0;
  const trace: string[] = [];
  const invoke = vi.fn(async (_command, { request }) => {
    const method = request.method;
    if (method === "desktop.prepareSignIn") {
      trace.push("prepare");
      expect(elapsed).toBe(0);
      expect(request.params).toEqual({
        value: { url: link, requestId },
        signInOwner: "remote",
      });
      return { id: request.id, result: "prepared-intent" };
    }
    if (method === "desktop.openSignIn") {
      trace.push("open");
      if (elapsed > 1200 && !request.params.value?.preparation)
        throw new Error("Use a desktop button or keyboard action.");
      expect(request.params).toEqual({
        value: {
          url: link,
          forward,
          preparation: "prepared-intent",
          requestId,
        },
        signInOwner: "remote",
      });
      return { id: request.id, result: true };
    }
    expect(method).toBe("desktop.cancelSignIn");
    trace.push("cancel");
    return { id: request.id, result: false };
  });
  vi.stubGlobal("__TAURI__", { core: { invoke } });
  controls.answer.mockImplementation(async () => {
    trace.push("approve");
    elapsed = 2000;
    return {
      kind: "answer",
      result: { status: "answered", openUrl: link, forward },
    };
  });
  const { post, notify, refresh } = card();
  const first = post({ decision: "accept", window: "desktop" });
  expect(await post({ decision: "accept", window: "desktop" })).toBe(false);
  expect(await first).toBe(true);
  expect(notify).not.toHaveBeenCalled();
  expect(trace).toEqual(["prepare", "approve", "open", "cancel"]);
  expect(controls.answer).toHaveBeenCalledExactlyOnceWith(requestId, {
    decision: "accept",
    window: "desktop",
  });
  expect(refresh).toHaveBeenCalledOnce();
  expect(controls.write).not.toHaveBeenCalled();
});

it("does not approve the card when native preparation lacks a user action", async () => {
  const invoke = vi.fn(async () => {
    throw new Error("Use a desktop button or keyboard action.");
  });
  vi.stubGlobal("__TAURI__", { core: { invoke } });
  const { post, notify } = card();
  expect(await post({ decision: "accept" })).toBe(false);
  expect(controls.answer).not.toHaveBeenCalled();
  expect(invoke).toHaveBeenCalledOnce();
  expect(notify).toHaveBeenCalledExactlyOnceWith(
    "Use a desktop button or keyboard action.",
  );
});

it("cancels a prepared intent after an uncertain approval without opening or retrying", async () => {
  const methods: string[] = [];
  const invoke = vi.fn(async (_command, { request }) => {
    methods.push(request.method);
    return {
      id: request.id,
      result:
        request.method === "desktop.prepareSignIn" ? "prepared-intent" : true,
    };
  });
  vi.stubGlobal("__TAURI__", { core: { invoke } });
  controls.answer.mockRejectedValue(new Error("Approval outcome is unknown."));
  const { post, notify } = card();
  expect(await post({ decision: "accept" })).toBe(false);
  expect(controls.answer).toHaveBeenCalledOnce();
  expect(methods).toEqual(["desktop.prepareSignIn", "desktop.cancelSignIn"]);
  expect(notify).toHaveBeenCalledExactlyOnceWith(
    "Approval outcome is unknown.",
  );
});

it("does not prepare or open a browser for a declined sign-in", async () => {
  const invoke = vi.fn();
  vi.stubGlobal("__TAURI__", { core: { invoke } });
  controls.answer.mockResolvedValue({
    kind: "answer",
    result: { status: "answered" },
  });
  const { post } = card();
  expect(await post({ decision: "decline" })).toBe(true);
  expect(invoke).not.toHaveBeenCalled();
});
