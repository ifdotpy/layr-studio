import {
  GLUE_MANIFEST,
  GLUE_MESSAGE,
  MAX_REACT_GLUE_EXCEPTIONS,
  REACT_GLUE_APIS,
  WEB_SOURCE,
  glueFiles,
  glueManifestErrors,
  glueOverrides,
  glueUses,
  isGlueSource,
} from "../react-glue.ts";
import ts from "typescript";
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import {
  mkdtempSync,
  mkdirSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";
import test from "node:test";

const root = fileURLToPath(new URL("../../../../../", import.meta.url));
const web = "workspaces/client/apps/web/src/";
const IMPORT_RULE = "eslint(no-restricted-imports)";
type Config = {
  overrides: { files: string[]; rules: Record<string, unknown> }[];
};
const fullConfig: Config = JSON.parse(
  readFileSync(path.join(root, ".oxlintrc.json"), "utf8"),
);

const config = {
  ...fullConfig,
  overrides: fullConfig.overrides.filter(
    (item) =>
      !JSON.stringify(item.rules["no-restricted-imports"] ?? null).includes(
        GLUE_MESSAGE,
      ),
  ),
};

function lint(cwd: string, configFile: string, target: string) {
  const result = spawnSync(
    path.join(root, "node_modules/.bin/oxlint"),
    [
      "--config",
      configFile,
      "--disable-nested-config",
      "--format",
      "json",
      target,
    ],
    { cwd, encoding: "utf8" },
  );
  assert.ok(result.status === 0 || result.status === 1, result.stderr);
  const report: { diagnostics: { code: string; filename: string }[] } =
    JSON.parse(result.stdout);
  return report.diagnostics.filter((item) =>
    item.code.includes("no-restricted-"),
  );
}

test("network exceptions only shrink and every remaining entry still violates", () => {
  // The final override is the migration list; transport/owner and test exemptions precede it.
  const exceptions = config.overrides.at(-1);
  assert.ok(exceptions);
  assert.deepEqual(exceptions.rules, { "no-restricted-imports": "off" });
  // Owner: maximum grandfathered network source files; only lower this ceiling.
  const MAX_NETWORK_EXCEPTIONS = 50;
  assert.ok(
    exceptions.files.length <= MAX_NETWORK_EXCEPTIONS,
    "network exception count grew",
  );
  assert.deepEqual(exceptions.files, [...new Set(exceptions.files)].sort());
  const reasons: Record<string, string> = JSON.parse(
    readFileSync(
      new URL("./network-boundary-exceptions.json", import.meta.url),
      "utf8",
    ),
  );
  assert.deepEqual(Object.keys(reasons), Object.keys(reasons).sort());
  assert.ok(Object.values(reasons).every((reason) => reason.trim()));
  assert.deepEqual(
    Object.keys(reasons).map((file) => web + file),
    exceptions.files,
    "config and exception manifest must agree",
  );
  if (exceptions.files.length === 0) return;
  const temporary = mkdtempSync(
    path.join(tmpdir(), "studio-network-exceptions-"),
  );
  try {
    const strict = path.join(temporary, ".oxlintrc.json");
    writeFileSync(
      strict,
      JSON.stringify({ ...config, overrides: config.overrides.slice(0, -1) }),
    );
    for (const file of exceptions.files) {
      const destination = path.join(temporary, file);
      mkdirSync(path.dirname(destination), { recursive: true });
      writeFileSync(destination, readFileSync(path.join(root, file)));
    }
    const actual = [
      ...new Set(
        lint(temporary, strict, web)
          .filter((item) => item.code === IMPORT_RULE)
          .map((item) => item.filename),
      ),
    ].sort();
    assert.deepEqual(
      actual,
      exceptions.files,
      "remove stale exceptions; new violations must move to owners",
    );
  } finally {
    rmSync(temporary, { recursive: true, force: true });
  }
});

test("React network boundary rejects direct transports and permits store/owner use", () => {
  const temporary = mkdtempSync(
    path.join(tmpdir(), "studio-network-boundary-"),
  );
  try {
    writeFileSync(
      path.join(temporary, ".oxlintrc.json"),
      JSON.stringify(fullConfig),
    );
    const imported = { [IMPORT_RULE]: 1 };
    const importedTwice = { [IMPORT_RULE]: 2 };
    const globalViolation = { "eslint(no-restricted-globals)": 1 };
    const beacon = { "eslint(no-restricted-properties)": 1 };
    const probes: [
      file: string,
      source: string,
      expected: Record<string, number>,
    ][] = [
      [
        "components/NetworkProbe.tsx",
        'import { get } from "../api"; get("/api/session");',
        imported,
      ],
      [
        "components/NormalizedProbe.tsx",
        'export { post } from "../components/../api.ts";',
        imported,
      ],
      [
        "components/useNetworkProbe.ts",
        'import { createSyncClient } from "../sync/client.ts"; void createSyncClient;',
        importedTwice,
      ],
      [
        "components/StreamProbe.tsx",
        'import { defaultSubscriptions } from "/src/sync/subscriptions/events"; void defaultSubscriptions;',
        imported,
      ],
      [
        "components/ResourceProbe.tsx",
        'export { createResourceEvents } from "../sync/resourceEvents";',
        imported,
      ],
      [
        "components/ServicesProbe.tsx",
        'export { useServerApi } from "../components/servers/ServerServicesProvider/ServerServicesProvider";',
        imported,
      ],
      [
        "components/ReaderProbe.tsx",
        'export { useServerModelReader } from "../components/servers/ServerServicesProvider/ServerServicesProvider";',
        imported,
      ],
      [
        "servers/FactoryProbe.tsx",
        'export { createServerViewServices } from "../components/servers/ServerServicesProvider/ServerServicesProvider";',
        imported,
      ],
      [
        "components/SidebarProbe.tsx",
        'export { useServerSidebarServices } from "../components/servers/ServerServicesProvider/ServerServicesProvider";',
        imported,
      ],
      [
        "components/BackendProbe.tsx",
        'export { localSidebarBackend } from "./sidebar/services";',
        imported,
      ],
      [
        "components/BackendHookProbe.tsx",
        'export { useSidebarBackend } from "./sidebar/services";',
        imported,
      ],
      [
        "components/GlobalProbe.tsx",
        'fetch("/"); new EventSource("/"); new XMLHttpRequest(); new WebSocket("wss://example.com"); window.fetch("/"); globalThis.fetch("/");',
        {
          "eslint(no-restricted-globals)": 6,
        },
      ],
      [
        "components/BeaconProbe.jsx",
        'navigator.sendBeacon("/"); window.navigator.sendBeacon("/");',
        {
          "eslint(no-restricted-properties)": 2,
        },
      ],
      ["components/LegacyProbe.js", 'fetch("/");', globalViolation],
      [
        "servers/NetworkProbe.tsx",
        'export { fetchServerSummary } from "./summary";',
        imported,
      ],
      [
        "servers/ServicesProbe.tsx",
        'export { useServerApi } from "../components/servers/ServerServicesProvider/ServerServicesProvider.tsx";',
        imported,
      ],
      [
        "components/Analytics.tsx",
        'fetch("/"); navigator.sendBeacon("/");',
        { ...globalViolation, ...beacon },
      ],
      [
        "store/nested/useNetworkProbe.ts",
        'export { get } from "../../api";',
        imported,
      ],
      ["sync/useNetworkProbe.tsx", 'fetch("/");', globalViolation],
      ["sync/useSyncedVisualState.ts", 'fetch("/");', globalViolation],
      [
        "components/StoreProbe.tsx",
        'export { createServerStore } from "../store/server.ts";',
        {},
      ],
      [
        "components/TypeProbe.tsx",
        'import type { ServerApi } from "../api"; export type { ServerApi };',
        {},
      ],
      [
        "components/EnumProbe.tsx",
        'export { AgentStatus } from "../generated/api.ts";',
        {},
      ],
      ["components/PureProbe.tsx", 'export { errorText } from "../api";', {}],
      ["store/networkProbe.ts", 'fetch("/"); navigator.sendBeacon("/");', {}],
      ["sync/networkProbe.ts", 'fetch("/");', {}],
      ["app/startup/StartupOwner.ts", 'fetch("/");', {}],
      [
        "components/NormalizedClientProbe.tsx",
        'export { createSyncClient } from "../sync/./client.ts";',
        importedTwice,
      ],
      [
        "components/NormalizedBackendProbe.tsx",
        'export { localSidebarServices } from "./sidebar/./services.tsx";',
        imported,
      ],
      [
        "components/AudioProbe.tsx",
        'export { useServerAudio } from "../components/servers/ServerServicesProvider/ServerServicesProvider";',
        imported,
      ],
      [
        "components/CombinedSidebarProbe.tsx",
        'export { useCombinedSidebarServices } from "../servers/combinedSidebarServices";',
        imported,
      ],
      [
        "components/ReaderBarrelProbe.tsx",
        'export { useModelCatalogReader } from "./agents/WorkerModelPicker";',
        imported,
      ],
      [
        "sync/useClientProbe.ts",
        'export { createSyncClient } from "./client";',
        imported,
      ],
      [
        "components/NativeCredentialsProbe.tsx",
        'import { NativeServerCredentials } from "../native/serverCredentials"; new NativeServerCredentials().fetch(server, request);',
        imported,
      ],
      [
        "components/NativeCredentialsExtensionProbe.tsx",
        'import { NativeServerCredentials } from "../native/serverCredentials.ts"; new NativeServerCredentials().fetch(server, request);',
        imported,
      ],
      [
        "components/NativeBridgeProbe.tsx",
        'import { nativeRequest, nativeHost } from "../native/client"; import { frameRequest } from "../native/frame"; void nativeRequest("servers.request", {}); void frameRequest("servers.request", {}); void nativeHost()?.core.invoke("servers.request", {});',
        {},
      ],
      ["components/DynamicProbe.tsx", 'void import("../api");', {}],
    ];
    for (const [file, source] of probes) {
      const destination = path.join(temporary, web, file);
      mkdirSync(path.dirname(destination), { recursive: true });
      writeFileSync(destination, source);
    }
    const violations = lint(temporary, ".oxlintrc.json", web);
    for (const [file, , expected] of probes) {
      const counts: Record<string, number> = {};
      for (const item of violations.filter(
        (item) => item.filename === web + file,
      ))
        counts[item.code] = (counts[item.code] ?? 0) + 1;
      assert.deepEqual(counts, expected, file);
    }
  } finally {
    rmSync(temporary, { recursive: true, force: true });
  }
});

/** Follow runtime edges, including barrels, aliases and literal dynamic imports. */
function runtimeImports(entry: string, base: string): string[] {
  const seen = new Set<string>();
  const visit = (file: string) => {
    if (seen.has(file)) return;
    seen.add(file);
    const source = ts.createSourceFile(
      file,
      readFileSync(file, "utf8"),
      ts.ScriptTarget.Latest,
      true,
    );
    const follow = (specifier: string) => {
      const resolved = ts.resolveModuleName(
        specifier,
        file,
        {
          moduleResolution: ts.ModuleResolutionKind.Bundler,
          paths: { "/src/*": [path.join(base, "*")] },
        },
        ts.sys,
      ).resolvedModule?.resolvedFileName;
      if (resolved && resolved.startsWith(base) && !resolved.endsWith(".d.ts"))
        visit(resolved);
    };
    const walk = (node: ts.Node) => {
      if (
        ts.isImportDeclaration(node) &&
        ts.isStringLiteral(node.moduleSpecifier)
      ) {
        const clause = node.importClause;
        if (
          !clause?.isTypeOnly &&
          !(
            clause?.namedBindings &&
            ts.isNamedImports(clause.namedBindings) &&
            clause.namedBindings.elements.every((item) => item.isTypeOnly)
          )
        )
          follow(node.moduleSpecifier.text);
      } else if (
        ts.isExportDeclaration(node) &&
        !node.isTypeOnly &&
        node.moduleSpecifier &&
        ts.isStringLiteral(node.moduleSpecifier)
      ) {
        if (
          !node.exportClause ||
          !ts.isNamedExports(node.exportClause) ||
          node.exportClause.elements.some((item) => !item.isTypeOnly)
        )
          follow(node.moduleSpecifier.text);
      } else if (
        ts.isCallExpression(node) &&
        node.expression.kind === ts.SyntaxKind.ImportKeyword &&
        node.arguments[0] &&
        ts.isStringLiteral(node.arguments[0])
      )
        follow(node.arguments[0].text);
      ts.forEachChild(node, walk);
    };
    walk(source);
  };
  visit(entry);
  return [...seen];
}

test("public API contracts have no runtime transport re-export", () => {
  const source = path.join(root, web);
  const graph = runtimeImports(path.join(source, "api.ts"), source);
  assert.ok(
    !graph.some((file) => file.includes("/servers/net/")),
    graph.join("\n"),
  );
  const compiled = ts.transpileModule(
    readFileSync(path.join(source, "api.ts"), "utf8"),
    {
      compilerOptions: { module: ts.ModuleKind.ESNext },
    },
  ).outputText;
  assert.ok(!compiled.includes("servers/net/api"), compiled);
});

test("compiler graph follows transport imports through a barrel, dynamic import and alias", () => {
  const temporary = mkdtempSync(path.join(tmpdir(), "studio-net-graph-"));
  try {
    mkdirSync(path.join(temporary, "servers/net"), { recursive: true });
    writeFileSync(
      path.join(temporary, "entry.ts"),
      'export { load } from "./barrel.ts";',
    );
    writeFileSync(
      path.join(temporary, "barrel.ts"),
      'export const load = () => import("/src/servers/net/api.ts");',
    );
    const transport = path.join(temporary, "servers/net/api.ts");
    writeFileSync(transport, "export const post = () => {};");
    assert.ok(
      runtimeImports(path.join(temporary, "entry.ts"), temporary).includes(
        transport,
      ),
    );
    writeFileSync(
      path.join(temporary, "entry.ts"),
      'export type { post } from "/src/servers/net/api.ts";',
    );
    assert.ok(
      !runtimeImports(path.join(temporary, "entry.ts"), temporary).includes(
        transport,
      ),
    );
  } finally {
    rmSync(temporary, { recursive: true, force: true });
  }
});

test("React glue imports are forbidden outside the shrink-only manifest", () => {
  const manifest: Record<string, string> = JSON.parse(
    readFileSync(path.join(root, GLUE_MANIFEST), "utf8"),
  );
  const entries = Object.keys(manifest);
  assert.deepEqual(entries, [...new Set(entries)].sort());
  assert.ok(entries.every(isGlueSource));
  assert.ok(
    Object.values(manifest).every(
      (reason) => reason.trim() && !/[\r\n]/.test(reason),
    ),
  );
  assert.deepEqual(
    fullConfig.overrides.slice(-2),
    glueOverrides(fullConfig, manifest),
    "regenerate Oxlint overrides: node workspaces/tooling/apps/repository-checks/react-glue.ts",
  );
  assert.deepEqual(glueManifestErrors(root, glueFiles(root)), []);
  const temporary = mkdtempSync(path.join(tmpdir(), "studio-react-glue-"));
  try {
    writeFileSync(
      path.join(temporary, ".oxlintrc.json"),
      JSON.stringify(fullConfig),
    );
    const targets = [
      "components/GlueProbe.tsx",
      "store/GlueProbe.ts",
      "sync/GlueProbe.ts",
      "app/GlueProbe.ts",
      "components/GlueProbe.test.tsx",
      "components/GlueProbe.stories.tsx",
      "fixtures/GlueProbe.tsx",
      "sync/eventHistory.testHelper.ts",
    ];
    for (const file of targets) {
      const destination = path.join(temporary, WEB_SOURCE, file);
      mkdirSync(path.dirname(destination), { recursive: true });
      writeFileSync(
        destination,
        `import { ${REACT_GLUE_APIS.map((name) => name + " as alias" + name).join(", ")} } from "react";`,
      );
    }
    const actual = lint(temporary, ".oxlintrc.json", WEB_SOURCE);
    for (const file of targets)
      assert.equal(
        actual.filter(
          (item) =>
            item.filename === WEB_SOURCE + file && item.code === IMPORT_RULE,
        ).length,
        isGlueSource(WEB_SOURCE + file) ? REACT_GLUE_APIS.length : 0,
        file,
      );
  } finally {
    rmSync(temporary, { recursive: true, force: true });
  }
});

test("listener check rejects callback sets and untyped subscribe/notify listener sets", () => {
  for (const source of [
    "const listeners = new Set<() => void>();",
    "class Owner { listeners = new Set<() => void>(); }",
    "const listeners = new Set(); function subscribe(cb) { listeners.add(cb); } function notify() { listeners.forEach(cb => cb()); }",
  ])
    assert.deepEqual(glueUses("probe.ts", source), ["listener pub-sub"]);
  assert.deepEqual(
    glueUses("probe.ts", 'import { useEffect as effect } from "react";'),
    ["useEffect"],
  );
  assert.deepEqual(
    glueUses(
      "probe.ts",
      'import type { useEffect } from "react"; const values = new Set<string>();',
    ),
    [],
  );
});

test("React glue manifest rejects one more exception even with a real violation", () => {
  const temporary = mkdtempSync(path.join(tmpdir(), "studio-glue-growth-"));
  try {
    const manifest: Record<string, string> = JSON.parse(
      readFileSync(path.join(root, GLUE_MANIFEST), "utf8"),
    );
    mkdirSync(path.dirname(path.join(temporary, GLUE_MANIFEST)), {
      recursive: true,
    });
    const writeManifest = () =>
      writeFileSync(
        path.join(temporary, GLUE_MANIFEST),
        JSON.stringify(manifest),
      );
    const writeViolation = (file: string) => {
      mkdirSync(path.dirname(path.join(temporary, file)), { recursive: true });
      writeFileSync(
        path.join(temporary, file),
        'import { useEffect } from "react";',
      );
    };
    for (const file of Object.keys(manifest)) writeViolation(file);
    writeManifest();
    assert.deepEqual(glueManifestErrors(temporary, glueFiles(temporary)), []);
    for (
      let index = Object.keys(manifest).length;
      index < MAX_REACT_GLUE_EXCEPTIONS;
      index++
    ) {
      const file = WEB_SOURCE + `components/GlueProbe${index}.tsx`;
      manifest[file] = "DOM integration.";
      writeViolation(file);
    }
    writeManifest();
    assert.deepEqual(glueManifestErrors(temporary, glueFiles(temporary)), []);
    const extra = WEB_SOURCE + "components/ExtraGlueProbe.tsx";
    manifest[extra] = "DOM integration.";
    writeViolation(extra);
    writeManifest();
    assert.deepEqual(glueManifestErrors(temporary, glueFiles(temporary)), [
      "React glue exception count grew",
    ]);
  } finally {
    rmSync(temporary, { recursive: true, force: true });
  }
});

test("React glue manifest rejects missing and stale entries", () => {
  const temporary = mkdtempSync(path.join(tmpdir(), "studio-glue-manifest-"));
  try {
    const file = WEB_SOURCE + "Probe.ts";
    mkdirSync(path.dirname(path.join(temporary, GLUE_MANIFEST)), {
      recursive: true,
    });
    mkdirSync(path.dirname(path.join(temporary, file)), { recursive: true });
    writeFileSync(path.join(temporary, GLUE_MANIFEST), "{}");
    writeFileSync(
      path.join(temporary, file),
      "const listeners = new Set<() => void>();",
    );
    assert.equal(glueManifestErrors(temporary, [file]).length, 1);
    writeFileSync(
      path.join(temporary, GLUE_MANIFEST),
      JSON.stringify({ [file]: "DOM input listeners." }),
    );
    assert.deepEqual(glueManifestErrors(temporary, [file]), []);
    writeFileSync(path.join(temporary, file), "export {};");
    assert.match(
      glueManifestErrors(temporary, [file])[0] ?? "",
      /remove stale/,
    );
    assert.match(glueManifestErrors(temporary, [])[0] ?? "", /remove stale/);
  } finally {
    rmSync(temporary, { recursive: true, force: true });
  }
});

test("hook counts follow React aliases and count calls instead of imports", async () => {
  const { hookCounts } = await import("../react-glue.ts");
  assert.deepEqual(
    hookCounts(
      "probe.tsx",
      `
    import React, { useMemo as state, useMemo, type useRef } from "react";
    import * as R from "react";
    state(1); state(2); React.useRef(null); R.useCallback(() => {});
    const text = "useMemo(0)";
  `,
    ),
    { useMemo: 2, useRef: 1, useCallback: 1 },
  );
});

test("hook ceiling update lowers counts and refuses new files and per-hook growth", async () => {
  const { hookCeilingErrors, lowerHookCeilings } =
    await import("../react-glue.ts");
  const file = WEB_SOURCE + "Probe.tsx";
  const baseline = { [file]: { useMemo: 2, useCallback: 1 } };
  const actual = { [file]: { useMemo: 1 } };
  assert.deepEqual(hookCeilingErrors(actual, baseline), []);
  assert.deepEqual(lowerHookCeilings(actual, baseline), actual);
  assert.deepEqual(lowerHookCeilings({}, baseline), {});
  for (const counts of [
    { [file]: { useMemo: 3 } },
    { [file]: { useRef: 1 } },
    { [WEB_SOURCE + "New.tsx"]: { useMemo: 1 } },
  ]) {
    assert.match(hookCeilingErrors(counts, baseline).join("\n"), /MobX/);
    assert.throws(() => lowerHookCeilings(counts, baseline), /MobX/);
  }
});

test("hook inventory excludes tests and permits the sanctioned Owned class boundary", async () => {
  const { hookInventory, hookManifestErrors, HOOK_MANIFEST, readHookCeilings } =
    await import("../react-glue.ts");
  const temporary = mkdtempSync(path.join(tmpdir(), "studio-hook-counts-"));
  try {
    const file = WEB_SOURCE + "stores/Owned.tsx";
    const testFile = WEB_SOURCE + "Probe.test.tsx";
    mkdirSync(path.dirname(path.join(temporary, file)), { recursive: true });
    mkdirSync(path.dirname(path.join(temporary, HOOK_MANIFEST)), {
      recursive: true,
    });
    writeFileSync(
      path.join(temporary, file),
      'import { Component } from "react"; export class Owned extends Component { store = this.props.create(); }',
    );
    writeFileSync(
      path.join(temporary, testFile),
      'import { useMemo } from "react"; useMemo(0);',
    );
    writeFileSync(path.join(temporary, HOOK_MANIFEST), "{}");
    assert.deepEqual(hookInventory(temporary, [file, testFile]), {});
    assert.deepEqual(hookManifestErrors(temporary, [file, testFile]), []);
    writeFileSync(
      path.join(temporary, HOOK_MANIFEST),
      JSON.stringify({ [file]: { useMemo: 1 } }),
    );
    assert.match(
      hookManifestErrors(temporary, [file], {}).join("\n"),
      /exceeds ceiling 0/,
    );
    for (const value of [
      null,
      [],
      { [file]: { useMemo: -1 } },
      { [file]: { useMemo: 1.2 } },
      { [file]: { other: 1 } },
    ])
      assert.throws(() => readHookCeilings(JSON.stringify(value)), /Malformed/);
  } finally {
    rmSync(temporary, { recursive: true, force: true });
  }
});

test("repository state hook counts stay within their per-file ceilings", async () => {
  const { hookManifestErrors } = await import("../react-glue.ts");
  assert.deepEqual(hookManifestErrors(root, glueFiles(root)), []);
});

test("hook counts cover named default imports and namespace destructuring", async () => {
  const { hookCounts } = await import("../react-glue.ts");
  assert.deepEqual(
    hookCounts(
      "probe.tsx",
      'import { default as React } from "react"; React.useCallback(() => {}); const {useMemo: memo, useRef} = React; memo(() => 1, []); useRef(null);',
    ),
    { useCallback: 1, useMemo: 1, useRef: 1 },
  );
});

test("trivial useState is allowed in new files under policy A", async () => {
  const { hookCounts, hookCeilingErrors } = await import("../react-glue.ts");
  const counts = hookCounts(
    "New.tsx",
    'import { useState } from "react"; const [open, setOpen] = useState(false);',
  );
  assert.deepEqual(counts, {});
  assert.deepEqual(
    hookCeilingErrors({ [WEB_SOURCE + "New.tsx"]: counts }, {}),
    [],
  );
});

test("namespace hook aliases are counted before their declaration", async () => {
  const { hookCounts } = await import("../react-glue.ts");
  assert.deepEqual(
    hookCounts(
      "Probe.tsx",
      `
    import * as React from "react";
    function Component() { return memo(() => 1, []); }
    const { useMemo: memo } = React;
  `,
    ),
    { useMemo: 1 },
  );
});
