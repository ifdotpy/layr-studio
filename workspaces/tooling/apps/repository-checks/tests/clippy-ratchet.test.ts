import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { collect, scopedBaseline } from "../clippy-ratchet.ts";
import { checkRatchet } from "../ratchet.ts";

const file = "workspaces/runtime/packages/runtime-store/src/lib.rs";
function diagnostic(code: string, message: string, start = 0) {
  return JSON.stringify({
    reason: "compiler-message",
    message: {
      code: { code },
      message,
      spans: [
        {
          is_primary: true,
          file_name: file,
          byte_start: start,
          byte_end: start + 3,
        },
      ],
    },
  });
}
test("Clippy diagnostics reject new and fixed findings until the baseline shrinks", () => {
  const line = diagnostic("clippy::option_option", "nested option");
  const found = collect(`${line}\n${line}`);
  assert.ok(found.option_option);
  assert.equal(found.option_option.length, 1); // duplicate target diagnostics
  const baseline = { option_option: found.option_option };
  assert.deepEqual(
    checkRatchet("option_option", found.option_option, baseline),
    [],
  );
  assert.match(
    checkRatchet("option_option", found.option_option, {}).join("\n"),
    /new violation/,
  );
  assert.match(
    checkRatchet("option_option", [], baseline).join("\n"),
    /fixed violation/,
  );
  assert.deepEqual(
    checkRatchet("option_option", [], { option_option: [] }),
    [],
  );
});
test("conversion lints are classified and other disallowed methods still fail", () => {
  const output = diagnostic(
    "clippy::disallowed_methods",
    "use of a disallowed method `serde_json::from_value`",
  );
  assert.equal((collect(output).from_value ?? []).length, 1);
  assert.throws(
    () =>
      collect(
        diagnostic(
          "clippy::disallowed_methods",
          "use of a disallowed method `std::thread::sleep`",
        ),
      ),
    /std::thread::sleep/,
  );
});

test("ratchet Clippy config extends the HTTP policy", () => {
  const owner = readFileSync(
    "workspaces/runtime/packages/runtime-http/clippy.toml",
    "utf8",
  );
  const config = readFileSync(
    "workspaces/tooling/apps/repository-checks/clippy-ratchet/clippy.toml",
    "utf8",
  );
  const extras = `
  { path = "serde_json::from_value", reason = "prefer typed deserialization" },
  { path = "serde_json::to_value", reason = "prefer typed serialization" },`;
  assert.equal(
    config,
    owner.replace("disallowed-methods = [", `disallowed-methods = [${extras}`),
  );
});

test("native baseline entries remain checked only when native is included", () => {
  const native = {
    file: "workspaces/client/apps/native/src-tauri/src/close.rs",
    fingerprint: "native:0",
  };
  const runtime = { file, fingerprint: "runtime:0" };
  const baseline = { option_option: [native, runtime] };
  assert.deepEqual(scopedBaseline(baseline, true), {
    option_option: [runtime],
  });
  assert.deepEqual(scopedBaseline(baseline, false), baseline);
  assert.deepEqual(baseline.option_option, [native, runtime]);
});
