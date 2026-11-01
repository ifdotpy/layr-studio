import { required } from "./components/componentTestFixtures.ts";
// Compare UI coverage to the installed native protocol without model requests.
import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { mkdtempSync, readFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import {
  nativeErrorHints,
  nativeErrorView,
  nativeErrorKind,
  nativeThreadError,
} from "./nativeErrors.ts";
import { test } from "vitest";
import { KNOWN_CODEX_ERROR_KINDS } from "studio-claude-bridge/provider-errors.generated.ts";
import { knownCodexErrorKind } from "studio-claude-bridge/provider-errors.ts";

const expectedMessages = new Map([
  ["cyberPolicy", "Codex refused this turn under its cybersecurity policy."],
  ["tooManyDenials", "Codex stopped this turn after too many denied actions."],
  [
    "flexUnavailable",
    "The selected model's flexible processing tier is unavailable right now.",
  ],
]);

function assertKnownVariants(codes: string[]) {
  for (const code of codes) {
    for (const info of [code, { [code]: { httpStatusCode: 503 } }]) {
      const view = nativeErrorView({
        message: "Native message",
        codexErrorInfo: info,
        additionalDetails: "Cause",
      });
      assert.equal(
        view!.message,
        expectedMessages.get(code) ?? "Native message",
      );
      assert.equal(view.kind, code);
      assert.ok(view);
      assert.ok(view.hint);
      assert.ok(view.hint.length > 0);
      assert.ok(view.details.includes("Cause"));
    }
  }
}

test.skipIf(!process.env.CODEX_NATIVE_SCHEMA)(
  "installed Codex error schema matches renderer coverage",
  async () => {
    const dir = mkdtempSync(join(tmpdir(), "studio-error-schema-"));
    try {
      execFileSync(process.env.CODEX_BIN || "codex", [
        "app-server",
        "generate-json-schema",
        "--experimental",
        "--out",
        dir,
      ]);
      const schema: {
        definitions: {
          CodexErrorInfo: {
            anyOf: { enum?: string[]; properties?: Record<string, unknown> }[];
          };
        };
      } = JSON.parse(
        readFileSync(join(dir, "v2/ErrorNotification.json"), "utf8"),
      );
      const codes = schema.definitions.CodexErrorInfo.anyOf.flatMap(
        (v) => v.enum ?? Object.keys(v.properties ?? {}),
      );
      assert.deepEqual([...codes].sort(), Object.keys(nativeErrorHints).sort());
      assertKnownVariants(codes);
      console.log(`PASS: ${codes.length} installed native error variants`);
    } finally {
      rmSync(dir, { recursive: true, force: true });
    }
  },
);

test("committed vendor classifications exhaust the renderer hints", () => {
  assert.deepEqual(
    [...KNOWN_CODEX_ERROR_KINDS].sort(),
    Object.keys(nativeErrorHints).sort(),
  );
  for (const kind of KNOWN_CODEX_ERROR_KINDS) {
    assert.equal(knownCodexErrorKind(kind), kind);
    assert.equal(knownCodexErrorKind({ [kind]: {} }), kind);
  }
  for (const info of [
    "futureCode",
    { futureCode: { opaque: [1, null] } },
    ["serverOverloaded"],
  ])
    assert.equal(knownCodexErrorKind(info), undefined);
});

test("native error rendering reads normalized fields without recursion", () => {
  assertKnownVariants(Object.keys(nativeErrorHints));
  assert.equal(
    nativeErrorView({ message: "RPC rejected", code: "-32600" }).message,
    "RPC rejected",
  );
  assert.equal(nativeErrorView("ordinary failure").message, "ordinary failure");
  assert.ok(
    nativeErrorView({ message: "New variant", codexErrorInfo: "futureCode" })
      .hint,
  );
  assert.equal(
    nativeErrorKind({ data: { codexErrorInfo: "serverOverloaded" } }),
    "",
  );
  const capacity = nativeErrorView({
    message: "",
    codexErrorInfo: "serverOverloaded",
  });
  assert.equal(capacity.message, "Codex is currently experiencing high load.");
  assert.equal(capacity.severity, "warning");
  assert.equal(
    nativeErrorView({
      codexErrorInfo: { activeTurnNotSteerable: { turnKind: "compact" } },
    }).severity,
    "warning",
  );
  const policy = {
    message: "Policy error",
    codexErrorInfo: "misalignmentPolicyViolation",
  };
  assert.equal(nativeErrorView(policy).title, "Chat stopped as a precaution");
  assert.equal(
    nativeThreadError({
      threadId: "one",
      nativeThreadBlock: { threadId: "one", error: policy },
      error: "Connection lost",
    }),
    policy,
  );
  assert.equal(
    nativeThreadError({
      threadId: "two",
      nativeThreadBlock: { threadId: "one", error: policy },
    }),
    undefined,
  );
  const biological = nativeErrorView({
    message: "Restricted",
    code: "bio_policy",
    codexErrorInfo: "other",
  });
  assert.equal(biological.title, "This content can't be shown");
  assert.equal(biological.severity, "info");
  assert.equal(
    required(biological.links[0]).href,
    "https://chatgpt.com/r/b749fb02595e04c3007a54375f3f4374",
  );
  assert.equal(
    nativeErrorView({ message: "Log text mentions bio_policy only." }).links
      .length,
    0,
  );
  for (const [plan, link] of [
    ["pro", "https://chatgpt.com/cyber/"],
    [
      "enterprise",
      "https://openai.com/form/enterprise-trusted-access-for-cyber/",
    ],
    [undefined, "https://openai.com/form/enterprise-trusted-access-for-cyber/"],
  ] as const)
    assert.equal(
      required(
        nativeErrorView({ codexErrorInfo: "cyberPolicy" }, plan).links[0],
      ).href,
      link,
    );
  const cyber = nativeErrorView(
    { message: "Raw refusal detail", codexErrorInfo: "cyberPolicy" },
    "pro",
  );
  assert.equal(
    cyber.message,
    "Codex refused this turn under its cybersecurity policy.",
  );
  assert.equal(cyber.title, "Cybersecurity policy refusal");
  assert.match(cyber.hint, /narrower or rephrased task/);
  assert.ok(cyber.details.includes("Raw refusal detail"));
  // Encoded and nested provider errors arrive normalized by the server.
  assert.equal(
    nativeErrorView({ message: "Unavailable" }).message,
    "Unavailable",
  );
  assert.equal(
    nativeErrorView({ message: "Nested failure" }).message,
    "Nested failure",
  );
});

test("Claude causes use plain words and preserve the raw code", () => {
  const view = nativeErrorView({
    message: "API failed",
    codexErrorInfo: "badRequest",
    claudeFailure: {
      apiError: "tool_history_mismatch",
      apiErrorCode: "new_server_gate",
      apiErrorStatus: 400,
    },
  });
  assert.equal(view.title, "Tool history does not match");
  assert.equal(view.kind, "badRequest");
  assert.match(view.message, /tool calls and results/);
  assert.match(view.details, /new_server_gate/);
});

test("Claude token counts and unknown server codes remain useful", () => {
  const view = nativeErrorView({
    claudeFailure: {
      apiErrorCode: "future_server_gate",
      promptTokens: 210001,
      maxContextTokens: 200000,
    },
  });
  assert.equal(view.title, "Claude request failed");
  assert.match(view.message, /210001.*200000/);
  assert.match(view.details, /future_server_gate/);
});

test("Claude cause metadata preserves the native limit reset text", () => {
  const view = nativeErrorView({
    message: "Usage limit reached. Try again at 20:00 UTC",
    codexErrorInfo: "rateLimitExceeded",
    claudeFailure: { error: "rate_limit", apiError: "usage_limit_reached" },
  });
  assert.match(view.message, /Try again at 20:00 UTC/);
  assert.equal(view.kind, "rateLimitExceeded");
  assert.equal(view.title, "Claude usage limit reached");
});

test("ambiguous error objects stay unclassified and retain every raw field", () => {
  const error = {
    message: "Provider extension",
    codexErrorInfo: { serverOverloaded: {}, futureDiagnostic: { id: 42 } },
    vendorExtension: ["keep", { nested: true }],
  };
  const original = JSON.stringify(error);
  assert.equal(nativeErrorKind(error), "");
  assert.deepEqual(JSON.parse(nativeErrorView(error).details), error);
  assert.equal(JSON.stringify(error), original);
});
