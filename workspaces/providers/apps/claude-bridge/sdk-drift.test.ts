import { dispatchMessage } from "./sdk-message-policy.ts";
import assert from "node:assert/strict";
import { test } from "vitest";
import ts from "typescript";
import { sdkDiagnostics } from "./testing/sdk-typecheck.ts";

test("the actual consumers compile against the pinned SDK", () => {
  assert.deepEqual(
    sdkDiagnostics([
      "provider-lifecycle.ts",
      "features.ts",
      "usage.ts",
      "sdk-message-policy.ts",
      "sdk-dispositions.ts",
      "command-background.ts",
    ]),
    [],
  );
});

test("changing the SDK lifecycle field type breaks the actual parser", () => {
  const errors = sdkDiagnostics("provider-lifecycle.ts", (source) => {
    assert.ok(source.includes("is_error: boolean;"));
    return source.replaceAll("is_error: boolean;", "is_error: string;");
  });
  assert.ok(
    errors.some((error) =>
      error.file?.fileName.endsWith("/provider-lifecycle.ts"),
    ),
    "A trusted type predicate must not hide changed SDK field types",
  );
});

test("adding an SDK result subtype breaks the production outcome mapping", () => {
  const errors = sdkDiagnostics("provider-lifecycle.ts", (source) =>
    source.replace(
      "'error_during_execution' |",
      "'future_result' | 'error_during_execution' |",
    ),
  );
  assert.ok(
    errors.some((error) =>
      error.file?.fileName.endsWith("/provider-lifecycle.ts"),
    ),
  );
});

for (const { name, file, from, to } of [
  {
    name: "a new query option",
    file: "sdk-dispositions.ts",
    from: "export declare type Options = {",
    to: "export declare type Options = { futureOption?: boolean;",
  },
  {
    name: "a new query method",
    file: "sdk-dispositions.ts",
    from: "export declare interface Query extends AsyncGenerator<SDKMessage, void> {",
    to: "export declare interface Query extends AsyncGenerator<SDKMessage, void> { futureMethod(): void;",
  },
  {
    name: "a new top-level event",
    file: "bridge.ts",
    from: "| SDKConversationResetMessage;",
    to: "| SDKConversationResetMessage | { type: 'future_event' };",
  },
  {
    name: "a new system event",
    file: "bridge.ts",
    from: "| SDKConversationResetMessage;",
    to: "| SDKConversationResetMessage | { type: 'system'; subtype: 'future_system' };",
  },
  {
    name: "a new assistant error",
    file: "features.ts",
    from: "| 'cloud_credential_error';",
    to: "| 'cloud_credential_error' | 'future_error';",
  },
  {
    name: "an added model token category",
    file: "usage.ts",
    from: "export declare type ModelUsage = {",
    to: "export declare type ModelUsage = { futureTokens: number;",
  },
  {
    name: "a changed model token field",
    file: "usage.ts",
    from: "inputTokens: number;",
    to: "inputTokens: string;",
  },
  {
    name: "a new task patch status",
    file: "command-background.ts",
    from: "'killed' | 'paused';",
    to: "'killed' | 'paused' | 'future_status';",
  },
]) {
  test(`${name} fails the actual SDK consumer compilation`, () => {
    const errors = sdkDiagnostics(file, (source) => {
      assert.ok(source.includes(from), `Missing mutation source ${from}`);
      return source.replace(from, to);
    });
    assert.ok(
      errors.some((error) => error.file?.fileName.endsWith("/" + file)),
      ts.formatDiagnosticsWithColorAndContext(errors, {
        getCurrentDirectory: () => process.cwd(),
        getCanonicalFileName: (name) => name,
        getNewLine: () => "\n",
      }),
    );
  });
}

import { createSdkMessageDiagnostics } from "./sdk-message-policy.ts";
import { CLAUDE_SDK_VERSION } from "./sdk-dispositions.ts";
import { sdkExportDrift, sdkPackMetadata } from "./sdk-drift.ts";
import { readFileSync } from "node:fs";
import type { SDKMessage } from "@anthropic-ai/claude-agent-sdk";

test("runtime SDK drift is explicit and diagnosed once per version and kind", () => {
  const reports: string[] = [];
  const disposition = createSdkMessageDiagnostics((text) => reports.push(text));
  // @ts-expect-error A newer runtime can emit variants absent from pinned declarations.
  const future: SDKMessage = { type: "system", subtype: "future_system" };
  assert.deepEqual(disposition(future), {
    kind: "Unknown",
    provider: "claude",
    version: CLAUDE_SDK_VERSION,
    type: "system",
    subtype: "future_system",
  });
  disposition(future);
  // @ts-expect-error Unknown top-level types are a separate diagnostic kind.
  disposition({ type: "future_message" });
  assert.equal(reports.length, 2);
  assert.ok(reports.every((report) => report.includes(CLAUDE_SDK_VERSION)));
});

test("the disposition version matches the manifest pin", () => {
  const manifest: unknown = JSON.parse(
    readFileSync(new URL("package.json", import.meta.url), "utf8"),
  );
  assert.ok(
    manifest && typeof manifest === "object" && "dependencies" in manifest,
  );
  const dependencies = manifest.dependencies;
  assert.ok(
    dependencies &&
      typeof dependencies === "object" &&
      "@anthropic-ai/claude-agent-sdk" in dependencies,
  );
  assert.equal(
    dependencies["@anthropic-ai/claude-agent-sdk"],
    CLAUDE_SDK_VERSION,
  );
});

test("the drift report identifies added removed and changed published exports", () => {
  assert.deepEqual(
    sdkExportDrift(
      "export type A = string; export interface B {}",
      "export type A = number; export type C = string;",
    ),
    { added: ["C"], removed: ["B"], changed: ["A"] },
  );
});

test("SDK pack metadata accepts the installed npm named-package receipt", () => {
  const metadata = { filename: "claude-agent-sdk.tgz", version: "0.3.308" };
  assert.deepEqual(
    sdkPackMetadata({ "@anthropic-ai/claude-agent-sdk": metadata }),
    metadata,
  );
  assert.deepEqual(sdkPackMetadata([metadata]), metadata);
  assert.throws(() =>
    sdkPackMetadata([{ ...metadata, filename: "../outside.tgz" }]),
  );
});

for (const invocation of [
  ".throw(new Error('test'))",
  "[Symbol.asyncDispose]()",
]) {
  test("an ignored Query invocation is rejected: " + invocation, () => {
    const errors = sdkDiagnostics("bridge.ts", undefined, "sdk", (source) => {
      const call = "const sdkQuery = query({ prompt: prompt(), options });";
      assert.ok(source.includes(call));
      return source.replace(call, call + " await sdkQuery" + invocation + ";");
    });
    assert.ok(
      errors.some(
        (error) =>
          (error.code === 2339 || error.code === 7053) &&
          error.file?.fileName.endsWith("/bridge.ts"),
      ),
      "Ignored Query invocations must not compile",
    );
  });
}

test("removing retry dispatch while retaining its parser breaks compilation", () => {
  const errors = sdkDiagnostics("bridge.ts", undefined, "sdk", (source) => {
    const ast = ts.createSourceFile(
      "bridge.ts",
      source,
      ts.ScriptTarget.ESNext,
      true,
    );
    let property: ts.PropertyAssignment | undefined;
    function visit(node: ts.Node) {
      if (
        ts.isPropertyAssignment(node) &&
        node.name.getText(ast) === "api_retry"
      )
        property = node;
      ts.forEachChild(node, visit);
    }
    visit(ast);
    assert.ok(property, "Production retry registration is required");
    const end = property.getEnd();
    return (
      source.slice(0, property.getStart(ast)) +
      source.slice(source[end] === "," ? end + 1 : end)
    );
  });
  assert.ok(
    errors.some(
      (error) =>
        error.code === 1360 && error.file?.fileName.endsWith("/bridge.ts"),
    ),
    "Handled retry must require its actual callback",
  );
});

for (const change of ["remove", "add"]) {
  test(`the actual Options builder rejects policy drift: ${change}`, () => {
    const errors = sdkDiagnostics("bridge.ts", undefined, "sdk", (source) => {
      const field = 'supportedDialogKinds: ["resume_return"],';
      assert.ok(source.includes(field));
      return source.replace(
        field,
        change === "remove" ? "" : field + ' title: "test",',
      );
    });
    assert.ok(
      errors.some((error) =>
        error.file?.fileName.endsWith("/sdk-dispositions.ts"),
      ),
      "Options decisions must follow configured keys",
    );
  });
}

test("a text delta dispatches synchronously", () => {
  const message = {
    type: "stream_event",
    event: {
      type: "content_block_delta",
      index: 0,
      delta: { type: "text_delta", text: "hello" },
    },
  } satisfies Pick<SDKMessage & { type: "stream_event" }, "type" | "event">;
  let seen = false;
  const handlers = { stream_event: () => void (seen = true) };
  assert.equal(
    dispatchMessage(message, handlers, message.type, undefined, undefined),
    undefined,
  );
  assert.equal(seen, true);
});
