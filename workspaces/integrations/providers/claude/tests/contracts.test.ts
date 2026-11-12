import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import test from "node:test";
import { checkClaudeRateContract } from "../tooling/generate.ts";
const root = fileURLToPath(new URL("../../../../..", import.meta.url));
test("the SDK-backed rate parser owns the committed Rust validation schema", async () => {
  await checkClaudeRateContract(root);
  await assert.rejects(
    () =>
      checkClaudeRateContract(root, (file) => {
        const schema = JSON.parse(readFileSync(file, "utf8"));
        schema.properties.status.enum.push("future");
        return JSON.stringify(schema);
      }),
    /Stale Claude rate contract/,
  );
});
test("an optional SDK field change fails declaration freshness offline", async () => {
  await assert.rejects(
    () =>
      checkClaudeRateContract(root, (file) => {
        const text = readFileSync(file, "utf8");
        return file.endsWith("sdk.d.ts")
          ? text.replace(
              "abortController?: AbortController;",
              "abortController?: never;",
            )
          : text;
      }),
    /Stale Claude rate provenance/,
  );
});
