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
const config: Config = JSON.parse(
  readFileSync(path.join(root, ".oxlintrc.json"), "utf8"),
);

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
  // Owner: this ceiling counts grandfathered source files, never diagnostics.
  const MAX_NETWORK_EXCEPTIONS = 87;
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
      JSON.stringify(config),
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
