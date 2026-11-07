// Executed by the pure Rust generator's fixture test with its generated output.
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import {
  mkdtempSync,
  mkdirSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { join, resolve } from "node:path";
import { text } from "node:stream/consumers";
import ts from "typescript";

const root = resolve(import.meta.dirname, "../../../../..");
const web = join(root, "workspaces/client/apps/web");
const input: { source: string; document: object } = JSON.parse(
  await text(process.stdin),
);
const directory = mkdtempSync(join(web, ".observable-codegen-"));
function write(path: string, source: string) {
  const destination = join(directory, path);
  mkdirSync(resolve(destination, ".."), { recursive: true });
  writeFileSync(destination, source);
}
function declarations(path: string, names: readonly string[]) {
  const source = ts.createSourceFile(
    path,
    readFileSync(join(web, path), "utf8"),
    ts.ScriptTarget.Latest,
    true,
  );
  return source.statements
    .filter(
      (node) =>
        (ts.isInterfaceDeclaration(node) ||
          ts.isTypeAliasDeclaration(node) ||
          ts.isFunctionDeclaration(node)) &&
        node.name &&
        names.includes(node.name.text),
    )
    .map((node) => node.getText(source))
    .join("\n");
}
try {
  write("package.json", '{"type":"module"}');
  write("src/generated/observableResources.ts", input.source);
  const api = spawnSync(
    process.execPath,
    [
      join(
        root,
        "workspaces/tooling/apps/repository-checks/generate-api-types.ts",
      ),
      "--rust-owned",
    ],
    {
      input: JSON.stringify({
        openapi: "3.1.0",
        info: { title: "Observable fixture", version: "1" },
        ...input.document,
      }),
      encoding: "utf8",
    },
  );
  assert.equal(api.status, 0, api.stderr);
  write("src/generated/api.ts", api.stdout);
  write(
    "src/deepReadonly.ts",
    readFileSync(join(web, "src/deepReadonly.ts"), "utf8"),
  );
  write(
    "src/sync/subscriptions/contracts.ts",
    `
    import type { components } from '../../generated/api.ts';
    export type ResourceRef = components['schemas']['ResourceRef'];
    export type ResourceData<R extends ResourceRef> = components['schemas']['SubscriptionDataMap'][R['kind']];
    export type SubscriptionError = Error;
    ${declarations("src/sync/subscriptions/contracts.ts", ["subscriptionKey"])}
  `,
  );
  write(
    "src/store/index.ts",
    `
    import type { DeepReadonly } from '../deepReadonly.ts';
    import type { ResourceRef, ResourceData, SubscriptionError } from '../sync/subscriptions/contracts.ts';
    ${declarations("src/store/runtime.ts", ["ResourceStatus", "ResourceSlot", "ObservableResourceRuntime"])}
  `,
  );
  write(
    "witness.ts",
    `
import assert from 'node:assert/strict';
import { autorun } from 'mobx';
import { createObservableResources, createFixtureMirror, agentId, teamId, type AgentId, type TeamId, type List } from './src/generated/observableResources.ts';
import type { components } from './src/generated/api.ts';
import type { DeepReadonly } from './src/deepReadonly.ts';
import type { ObservableResourceRuntime } from './src/store/index.ts';

const a = agentId('agent'); const t = teamId('team');
const brandWitness = (agent: AgentId, team: TeamId) => [agent, team];
brandWitness(a, t);
// @ts-expect-error Resource identities must not cross brands.
brandWitness(t, a);
function readonlyWitness(value: List) {
  // @ts-expect-error Public data cannot be assigned.
  value.rows = [];
  const row = value.rows[0];
  if (row) {
    // @ts-expect-error Public nested fields cannot be assigned.
    row.nested.count = 3;
    // @ts-expect-error Public arrays cannot be mutated.
    row.tags.push('bad');
    // @ts-expect-error Model apply is not exposed through the facade.
    row.apply(row);
  }
}
void readonlyWitness;
type Row = components['schemas']['Row'];
type Dto = DeepReadonly<components['schemas']['List']>;
const opaque = { payload: 'transport' };
const row = (id: string, sortKey: string): Row => ({ id, sortKey, title: null, nested: {count: 1}, tags: ['one'], choice: { kind: 'count', count: 1 }, opaque, maybe: null, nil: null, optional: 'present' });
let latest: Dto = { rows: [row('b', '2'), row('a', '1')], plain: [1, 2] };
let publish: () => void = () => { throw new Error('slot missing'); };
let opens = 0;
const runtime: ObservableResourceRuntime = {
  slot(definition) {
    opens += 1;
    const data = definition.create(latest as DeepReadonly<import('./src/sync/subscriptions/contracts.ts').ResourceData<typeof definition.ref>>);
    publish = () => definition.apply(data, latest as DeepReadonly<import('./src/sync/subscriptions/contracts.ts').ResourceData<typeof definition.ref>>);
    return { current: {status: 'live', data: data as DeepReadonly<typeof data>} };
  }
};
const store = createObservableResources(runtime);
const slot = store.fixture(a, t);
assert.equal(slot, store.fixture(a, t)); assert.equal(opens, 1);
const state = slot.current; assert.equal(state.status, 'live');
if (state.status !== 'live') throw new Error('expected live');
const data = state.data;
assert.deepEqual(data.rows.map(row => row.id), ['a', 'b']);
const first = data.rows[0]; assert.ok(first);
const second = data.rows[1]; assert.ok(second);
const nested = first.nested; const tags = first.tags; const choice = first.choice;
let runs = 0; const stop = autorun(() => { data.rows.map(row => [row.id, row.sortKey, row.title, row.nested.count, row.tags.join(','), row.choice.kind, row.optional, row.opaque]); data.plain.join(','); runs += 1; });
const initialRuns = runs;
latest = { rows: [row('b', '2'), row('a', '1')], plain: [1, 2] }; publish();
assert.equal(runs, initialRuns, 'equal replay produces no reactions');
assert.equal(data.rows[0], first); assert.equal(first.nested, nested); assert.equal(first.tags, tags); assert.equal(first.choice, choice);
const withoutOptional = row('a', '3'); delete withoutOptional.optional;
latest = { rows: [{...withoutOptional, nested: {count: 2}, title: 'changed', choice: {kind:'text', text:'new'}, maybe: {value: 'present'}}, row('b', '2'), row('c', '0')], plain: [1, 3] }; publish();
assert.deepEqual(data.rows.map(row => row.id), ['c', 'b', 'a']);
assert.equal(data.rows[2], first); assert.equal(first.nested, nested); assert.equal(first.nested.count, 2);
assert.equal(first.optional, undefined); assert.equal(first.maybe?.value, 'present'); assert.equal(first.title, 'changed'); assert.notEqual(first.choice, choice); assert.equal(first.choice.kind, 'text');
latest = { rows: [{...withoutOptional, tags: ['two'], opaque: {payload: 'new'}}], plain: [] }; publish();
assert.deepEqual(data.rows.map(row => row.id), ['a']); assert.equal(first.title, null); assert.notEqual(first.tags, tags); assert.notEqual(first.opaque, opaque); assert.equal(first.maybe, null); assert.equal(first.nil, null);
latest = { rows: [row('a', '1'), row('a', '2')], plain: [] };
assert.throws(publish, /Duplicate identity/); assert.deepEqual(data.rows.map(row => row.id), ['a']);
stop();
const secondStore = createObservableResources(runtime); latest = {rows:[row('x','0'),row('x','1')],plain:[]};
assert.throws(() => secondStore.fixture(a,t), /Duplicate identity/);
const mirror = createFixtureMirror();
assert.equal(mirror.current.rows.length, 0);
mirror.apply({rows:[row('a','1'), row('b','2')],plain:[]});
const mirrorFirst=mirror.current.rows[0]; const mirrorSecond=mirror.current.rows[1];
assert.ok(mirrorFirst); assert.ok(mirrorSecond);
let mirrorRuns=0; const stopMirror=autorun(()=>{mirror.current.rows.map(row=>[row.title,row.nested.count]);mirrorRuns+=1;});
mirror.apply({rows:[row('a','1'),row('b','2')],plain:[]});assert.equal(mirrorRuns,1);
mirror.apply({rows:[{...row('a','1'),title:'updated'},row('b','2')],plain:[]});
assert.equal(mirror.current.rows[0],mirrorFirst);assert.equal(mirror.current.rows[1],mirrorSecond);assert.equal(mirrorFirst.title,'updated');assert.equal(mirrorRuns,2);stopMirror();
// @ts-expect-error A mirror's exposed snapshot remains readonly.
if (false) mirror.current.rows = [];
console.log('observable generator type and behavior witnesses passed');
`,
  );
  const parsed = ts.getParsedCommandLineOfConfigFile(
    join(web, "tsconfig.json"),
    {},
    {
      ...ts.sys,
      onUnRecoverableConfigFileDiagnostic: (diagnostic) => {
        throw new Error(
          ts.flattenDiagnosticMessageText(diagnostic.messageText, "\n"),
        );
      },
    },
  );
  assert.ok(parsed);
  const program = ts.createProgram([join(directory, "witness.ts")], {
    ...parsed.options,
    types: ["node"],
  });
  const diagnostics = ts.getPreEmitDiagnostics(program);
  assert.equal(
    diagnostics.length,
    0,
    ts.formatDiagnosticsWithColorAndContext(diagnostics, {
      getCurrentDirectory: () => root,
      getCanonicalFileName: (file) => file,
      getNewLine: () => "\n",
    }),
  );
  const result = spawnSync(process.execPath, [join(directory, "witness.ts")], {
    encoding: "utf8",
  });
  assert.equal(result.status, 0, result.stdout + result.stderr);
  process.stdout.write(result.stdout);
} finally {
  rmSync(directory, { recursive: true, force: true });
}
