import { test } from "node:test";
import assert from "node:assert/strict";
import { operationStateMatches, isOperationOwner } from "../operation-state.ts";
import { readFileSync } from "node:fs";
import { scan } from "../ast-grep.ts";
import {
  checkRatchet,
  baselineGrowthErrors,
  readBaseline,
} from "../ratchet.ts";
import { OPERATION_STATE_RULE, OPERATION_OWNERS } from "../operation-state.ts";

const file = "workspaces/client/apps/web/src/app/conversation/RenameOwner.ts";
const matches = (text: string) => operationStateMatches(file, text);

test("operation owners reject writable mutation state, including aliased hooks", () => {
  for (const source of [
    "class Owner { pending = new Map(); }",
    "class Owner { requestId?: string; }",
    "const form = { saving: false };",
    "const form = { busy: computeStatus() };",
    'import { useState as state } from "react"; const [busy, setBusy] = state(false);',
    'import React from "react"; const { useRef: ref } = React; const sending = ref(false);',
    'import * as R from "react"; const [uncertain] = R.useState(false);',
    "this.request_id = value;",
    'this["pending"] = value;',
    'state.set("pending", value);',
  ])
    assert.ok(matches(source).length, source);
});

test("transport and identity boundary catches aliases and destructuring", () => {
  for (const source of [
    'import { post as write } from "./net/api.ts"; write("/api/rename", body);',
    'const { syncPost: send } = api; send("/api/rename", body);',
    'const write = api.post; write("/api/rename", body);',
    "const { run: dispatch } = command; dispatch(body);",
    'import { requestId as id } from "studio-claude-bridge/request-id.ts"; const request_id = id();',
    "const { randomUUID: id } = crypto; api.post(path, { request_id: id() });",
    "const { stableRequestId: id } = storage; requests.start(op.intent(key, { id: id() }));",
    "const { runInAction: action } = mobx; action(() => {});",
    "api.post(path, body).then(() => store.refetch());",
    'storage.setItem("studio-server-write:key", body);',
  ])
    assert.ok(matches(source).length, source);
});

test("identity aliases are diagnosed independently of their mutation transport", () => {
  for (const source of [
    "const { randomUUID: id } = crypto; api.post(path, { request_id: id() });",
    "const makeId = crypto.randomUUID; requests.start(op.intent(key, { id: makeId() }));",
    'import { requestId as id } from "studio-claude-bridge/request-id.ts"; const request_id = id();',
  ])
    assert.ok(
      matches(source).some((match) =>
        match.message.startsWith("Mutation identity generation"),
      ),
      source,
    );
});

test("registry projections, read state, validation and navigation identities remain permitted", () => {
  assert.deepEqual(
    matches(`
    class Owner {
      readonly requestId = entry.requestId;
      validationError = "";
      readLoading = false;
      get busy() { return this.request.view.busy; }
      navigate() { return { requestId: crypto.randomUUID() }; }
    }
    const { busy, requestId } = entry.view;
    const [open] = useState(false);
    const request_id = entry.requestId;
    const body = { request_id: entry.requestId };
    const read = { status: "pending", error: "" };
  `),
    [],
  );
});

test("chained Zod schemas remain validation state", () => {
  assert.deepEqual(
    matches(`
    const schema = z.object({
      pending: z.boolean().optional(),
      saving: z.boolean().nullable().default(false),
      uncertain: z.coerce.boolean().optional(),
    });
  `),
    [],
  );
});

test("parsed Zod values are writable operation state", () => {
  for (const method of ["parse", "safeParse"])
    assert.ok(
      matches(
        `const form = observable({ pending: z.boolean().${method}(false) });`,
      ).some((match) => match.message.startsWith("Stored operation state")),
    );
});

test("outbox key identifiers are classified by const literal values, never spelling", () => {
  for (const source of [
    'const stableRequestTimeoutKey = "ui:request-timeout"; storage.setItem(stableRequestTimeoutKey, "5000");',
    "storage.setItem(stableRequestUnknownKey, value);",
    'let key = "studio-project-request:key"; storage.setItem(key, body);',
  ])
    assert.deepEqual(matches(source), [], source);
  assert.ok(
    matches(
      'const key = "studio-project-request:key"; storage.setItem(key, body);',
    ).some((match) => match.message.startsWith("Direct request-outbox write")),
  );
});

test("navigation UUIDs without mutation calls are allowed in every function form", () => {
  for (const source of [
    "function navigate() { return { requestId: crypto.randomUUID() }; }",
    "const navigate = () => ({ requestId: crypto.randomUUID() });",
    "class Owner { navigate() { return { requestId: crypto.randomUUID() }; } }",
  ])
    assert.deepEqual(matches(source), [], source);
  assert.ok(
    matches(
      "function mutate() { const requestId = crypto.randomUUID(); requests.start(intent); }",
    ).some((match) => match.message.startsWith("Mutation identity generation")),
  );
});

test("literal legacy outbox namespaces cannot reintroduce independent request journals", () => {
  for (const prefix of [
    "studio-server-write:",
    "studio-pending-renames:",
    "studio-sidebar-rename:",
    "studio-project-request:",
    "stableRequest",
  ])
    assert.ok(
      matches(`storage.setItem(${JSON.stringify(prefix + "key")}, body);`).some(
        (match) => match.message.startsWith("Direct request-outbox write"),
      ),
      prefix,
    );
  assert.deepEqual(
    matches('storage.setItem("studio-ui-preference:key", value);'),
    [],
  );
});

test("scope includes migrated owners and new registry users, excluding fixtures", () => {
  assert.ok(isOperationOwner(file, ""));
  assert.ok(
    isOperationOwner(
      "workspaces/client/apps/web/src/New.ts",
      'import { x } from "./servers/requestOperations.ts";',
    ),
  );
  assert.equal(
    isOperationOwner(file.replace(".ts", ".test.ts"), "requestOperations"),
    false,
  );
  assert.equal(
    isOperationOwner(
      "workspaces/client/apps/web/src/Read.ts",
      "const pending = false;",
    ),
    false,
  );
});

test("actual scanner uses the existing shrinking fingerprint ratchet with exact paths and reasons", () => {
  const baseline = readBaseline();
  const found = scan(process.cwd(), OPERATION_OWNERS, "^web-");
  assert.deepEqual(
    checkRatchet(
      OPERATION_STATE_RULE,
      found[OPERATION_STATE_RULE] ?? [],
      baseline,
    ),
    [],
  );
  for (const entry of baseline[OPERATION_STATE_RULE] ?? []) {
    assert.ok(entry.reason?.trim());
    assert.ok(OPERATION_OWNERS.includes(entry.file));
    assert.ok(isOperationOwner(entry.file, readFileSync(entry.file, "utf8")));
  }
  const [entry] = baseline[OPERATION_STATE_RULE] ?? [];
  assert.ok(entry);
  assert.deepEqual(
    baselineGrowthErrors({ [OPERATION_STATE_RULE]: [] }, baseline),
    [],
  );
  assert.match(
    baselineGrowthErrors(
      { [OPERATION_STATE_RULE]: [{ ...entry, file: file + ".new.ts" }] },
      baseline,
    ).join("\n"),
    /only shrink/,
  );
});
