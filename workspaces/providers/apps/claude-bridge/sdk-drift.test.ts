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
    file: "sdk-message-policy.ts",
    from: "| SDKConversationResetMessage;",
    to: "| SDKConversationResetMessage | { type: 'future_event' };",
  },
  {
    name: "a new system event",
    file: "sdk-message-policy.ts",
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
