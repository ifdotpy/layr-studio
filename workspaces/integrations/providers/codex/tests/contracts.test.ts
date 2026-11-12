import assert from "node:assert/strict";
import {
  readFileSync,
  mkdtempSync,
  rmSync,
  writeFileSync,
  mkdirSync,
} from "node:fs";
import { spawnSync } from "node:child_process";
import { tmpdir } from "node:os";
import path from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import test from "node:test";
import {
  renderCodexContracts,
  renderFullCodexDefinitions,
  codexDefinitions,
  codexDrift,
  checkCodexContracts,
  refreshCodexSchemas,
} from "../tooling/generate.ts";
import { codexContractSource } from "../vendor/source.ts";

const root = fileURLToPath(new URL("../../../../..", import.meta.url));

test("committed Rust and TypeScript consume the same vendor schema", () => {
  assert.doesNotThrow(() => checkCodexContracts(root));
});

test("a vendor refresh fails explicitly when the pinned CLI is absent", () => {
  const directory = mkdtempSync(path.join(tmpdir(), "studio-no-provider-"));
  try {
    assert.throws(
      () => refreshCodexSchemas(root, path.join(directory, "codex"), true),
      /ENOENT/,
    );
  } finally {
    rmSync(directory, { recursive: true, force: true });
  }
});

test("regeneration cannot relabel stale vendor schemas as a new pinned version", () => {
  assert.throws(
    () =>
      renderCodexContracts(root, (file) => {
        const text = readFileSync(file, "utf8");
        return file.endsWith("lifecycle.schema.json")
          ? text.replace(
              `codex-cli ${codexContractSource.version}`,
              "codex-cli future",
            )
          : text;
      }),
    /provenance differs/,
  );
});

test("the selected schema generates added variants and changed fields", () => {
  const render = renderCodexContracts(root, (file) => {
    const text = readFileSync(file, "utf8");
    if (file.endsWith("lifecycle.schema.json")) {
      const value = JSON.parse(text);
      value.definitions.TurnStatus.enum.push("futureStatus");
      return JSON.stringify(value);
    }
    return text;
  });
  const rust = render.find((entry) =>
    entry.path.endsWith("codex_generated.rs"),
  );
  assert.ok(rust?.text.includes("FutureStatus"));
  assert.ok(
    render
      .find((entry) => entry.path.endsWith("/TurnStatus.ts"))
      ?.text.includes('"futureStatus"'),
  );
});

test("freshness refuses a stale owner rather than compiling the old types", () => {
  assert.throws(
    () =>
      checkCodexContracts(
        root,
        (file) =>
          readFileSync(file, "utf8") +
          (file.endsWith("codex_generated.rs") ? "// stale\n" : ""),
      ),
    /Stale Codex contract/,
  );
});

for (const mutation of ["requiredDefault", "optionalNonNullable"] as const) {
  test(`generation refuses the unsupported ${mutation} presence codec`, () => {
    assert.throws(
      () =>
        renderCodexContracts(root, (file) => {
          const text = readFileSync(file, "utf8");
          if (!file.endsWith("ThreadTokenUsageUpdatedNotification.json"))
            return text;
          const schema = JSON.parse(text);
          if (mutation === "requiredDefault")
            schema.definitions.TokenUsageBreakdown.required.push(
              "cacheWriteInputTokens",
            );
          else
            schema.definitions.ThreadTokenUsage.properties.modelContextWindow.type =
              "integer";
          return JSON.stringify(schema);
        }),
      /Unsupported vendor presence codec/,
    );
  });
}

const fullDefinitions = () =>
  codexDefinitions(
    readFileSync(path.join(root, codexContractSource.schemaFile), "utf8"),
  );

test("full vendor generation covers every protocol method and same-source TypeScript", () => {
  const output = renderFullCodexDefinitions(fullDefinitions());
  const rust = output.find((entry) =>
    entry.path.endsWith("/src/generated/mod.rs"),
  )?.text;
  const ts = output.find((entry) => entry.path.endsWith("/protocol.ts"))?.text;
  assert.ok(rust && ts);
  for (const name of ["ClientRequest", "ServerRequest", "ServerNotification"]) {
    assert.ok(rust.includes(`pub enum ${name}`));
    assert.ok(ts.includes(`export type ${name}`));
  }
  // Both vendor spellings normalize to OpenaiForm; preserve separate variants.
  assert.ok(rust.includes("OpenaiFormVariant4"));
  assert.ok(ts.includes('"openai/form"'));
  assert.ok(ts.includes('"openaiForm"'));
});

test("a synthetic vendor method fails the actual exhaustive disposition at compile time", () => {
  const copy = mkdtempSync(path.join(tmpdir(), "studio-codex-disposition-"));
  try {
    mkdirSync(path.join(copy, "src"));
    writeFileSync(
      path.join(copy, "Cargo.toml"),
      '[package]\nname = "codex-disposition-witness"\nversion = "0.0.0"\nedition = "2024"\n',
    );
    const definitions = fullDefinitions();
    const disposition = readFileSync(
      path.join(
        root,
        "workspaces/runtime/packages/runtime-providers/src/codex_disposition.rs",
      ),
      "utf8",
    )
      .replace(
        "use studio_codex_protocol::",
        "use crate::studio_codex_protocol::",
      )
      .replaceAll("//!", "//");
    const kinds = readFileSync(
      path.join(
        root,
        "workspaces/runtime/packages/runtime-services/src/notification_dispatch/kinds.rs",
      ),
      "utf8",
    );
    const binding = kinds.slice(
      kinds.indexOf("fn projection("),
      kinds.indexOf("\nfn native_binding("),
    );
    assert.ok(binding.includes("H::TurnDiffUpdated =>"));
    const effects = readFileSync(
      path.join(
        root,
        "workspaces/runtime/packages/runtime-services/src/notification_dispatch/effects.rs",
      ),
      "utf8",
    );
    const dependency = effects.match(
      /pub enum NotificationDependency \{[\s\S]*?\n\}/,
    )?.[0];
    assert.ok(dependency);
    // Compile the actual branch selector. Only unrelated handler bodies are
    // stubs; neither an incidental method string nor a forwarding arm can fill it.
    const consumer = `
${dependency.replace(/^\s*#\[serde[^\n]*\n/gm, "")}
type Apply = fn(ApplyInput);
struct ApplyInput;
impl ApplyInput {
    fn write_plan_diff(self, _: &str) {} fn plan(self) {} fn reasoning(self) {}
    fn command_output(self) {} fn resolved(self) {} fn unload(self) {}
}
enum Projection { Apply(Apply), Buffered, AccountLimits, Requires(NotificationDependency) }
${binding}`;
    // The gate supplies its workspace target. This dependency-free witness must
    // use its own temporary target, so it never waits behind the workspace build.
    const env: NodeJS.ProcessEnv = {
      ...process.env,
      CARGO_HOME: path.join(copy, "cargo-home"),
    };
    delete env.CARGO_TARGET_DIR;
    const check = (policy = disposition, dispatcher = consumer) => {
      const generated = renderFullCodexDefinitions(definitions, true).find(
        (entry) => entry.path.endsWith("/src/generated/mod.rs"),
      )?.text;
      assert.ok(generated);
      const enums = [
        "ClientRequestMethod",
        "ServerRequestMethod",
        "ServerNotificationMethod",
        "ClientNotificationMethod",
      ]
        .map((name) => {
          const body = generated.match(
            new RegExp(`pub enum ${name} \\{[\\s\\S]*?\\n\\}`),
          )?.[0];
          assert.ok(body);
          return (
            "#[derive(Clone, Copy, Debug, Eq, PartialEq)]\n" +
            body.replace(/^\s*#\[serde[^\n]*\n/gm, "")
          );
        })
        .join("\n");
      writeFileSync(
        path.join(copy, "src/lib.rs"),
        `pub mod studio_codex_protocol {\n${enums}\n}\n${policy}\n${dispatcher}`,
      );
      const result = spawnSync(
        "cargo",
        ["check", "--offline", "--quiet", "-j", "4"],
        {
          cwd: copy,
          env,
          encoding: "utf8",
          timeout: 60_000,
        },
      );
      assert.equal(result.signal, null);
      assert.equal(result.error, undefined);
      return result;
    };
    const baseline = check();
    assert.equal(baseline.status, 0, baseline.stderr);
    const removed = check(
      disposition,
      consumer.replace(
        /\s*H::TurnDiffUpdated => Projection::Apply\([^\n]+\),/,
        "",
      ),
    );
    assert.notEqual(removed.status, 0);
    assert.match(removed.stderr, /non-exhaustive patterns.*TurnDiffUpdated/);
    const ignored = check(
      disposition
        .replace("            TurnDiffUpdated => TurnDiffUpdated,", "")
        .replace(
          "    ignored {\n        ThreadStarted",
          '    ignored {\n        TurnDiffUpdated => "mutation",\n        ThreadStarted',
        ),
    );
    assert.notEqual(ignored.status, 0);
    assert.match(ignored.stderr, /no variant.*TurnDiffUpdated/);
    for (const family of [
      "ServerNotification",
      "ClientNotification",
    ] as const) {
      const notification = definitions[family];
      assert.ok(
        notification &&
          typeof notification === "object" &&
          !Array.isArray(notification),
      );
      assert.ok("oneOf" in notification && Array.isArray(notification.oneOf));
      notification.oneOf.push({
        type: "object",
        properties: {
          method: { type: "string", enum: ["synthetic/compileFailure"] },
        },
        required: ["method"],
      });
      const mutation = check();
      assert.notEqual(mutation.status, 0);
      assert.match(mutation.stderr, /SyntheticCompileFailure.*not covered/);
      const method = `${family}Method`;
      const decided = check(
        family === "ServerNotification"
          ? disposition.replace(
              "    ignored {\n        ThreadStarted",
              '    ignored {\n        SyntheticCompileFailure => "Offline witness only.",\n        ThreadStarted',
            )
          : disposition.replace(
              `match method {\n        ${method}::`,
              `match method {\n        ${method}::SyntheticCompileFailure => Disposition::Ignored { reason: "Offline witness only." },\n        ${method}::`,
            ),
      );
      assert.equal(decided.status, 0, decided.stderr);
      notification.oneOf.pop();
    }
  } finally {
    rmSync(copy, { recursive: true, force: true });
  }
});

test("drift reports new methods, removed types, changed fields and result links", () => {
  const before = fullDefinitions();
  const after = fullDefinitions();
  const notification = after.ServerNotification;
  assert.ok(
    notification &&
      typeof notification === "object" &&
      !Array.isArray(notification),
  );
  assert.ok("oneOf" in notification && Array.isArray(notification.oneOf));
  notification.oneOf.push({
    type: "object",
    properties: {
      method: { type: "string", enum: ["synthetic/drift"] },
      params: { type: "object" },
    },
    required: ["method", "params"],
  });
  delete after.ThreadStatus;
  after.AbsolutePathBuf = { type: "integer", format: "int64" };
  const report = codexDrift(before, after, codexContractSource.responseTypes, {
    ...codexContractSource.responseTypes,
    initialize: "ChangedResponse",
  });
  assert.equal(report.changed, true);
  assert.ok(report.lines.includes("ServerNotification added: synthetic/drift"));
  assert.ok(report.lines.includes("types removed: ThreadStatus"));
  assert.ok(
    report.lines.some(
      (line) =>
        line.startsWith("types changed:") && line.includes("AbsolutePathBuf"),
    ),
  );
  assert.ok(report.lines.includes("request results changed: initialize"));
  assert.equal(
    codexDrift(
      before,
      before,
      codexContractSource.responseTypes,
      codexContractSource.responseTypes,
    ).changed,
    false,
  );
});

test("the drift recipe exits one on a breaking payload-field change", () => {
  const temporary = mkdtempSync(path.join(tmpdir(), "studio-drift-payload-"));
  try {
    const fixture = path.join(
      root,
      "workspaces/integrations/providers/codex/tests/fixtures/drift.fixture.ts",
    );
    const schemaFile = path.join(temporary, "schema.json");
    const schema = JSON.parse(
      readFileSync(path.join(root, codexContractSource.schemaFile), "utf8"),
    );
    writeFileSync(schemaFile, JSON.stringify(schema));
    const run = (args = ["provider-drift"]) => {
      const result = spawnSync("just", args, {
        cwd: root,
        env: {
          ...process.env,
          CODEX_BIN: fixture,
          STUDIO_TEST_CODEX_SCHEMA: schemaFile,
          NODE_OPTIONS: `${process.env.NODE_OPTIONS ?? ""} --import ${pathToFileURL(fixture).href}`,
        },
        encoding: "utf8",
        timeout: 30_000,
      });
      assert.equal(result.signal, null);
      assert.equal(result.error, undefined);
      return result;
    };
    const baseline = run();
    assert.equal(baseline.status, 0, baseline.stdout + baseline.stderr);
    const owner = readFileSync(
      path.join(
        root,
        "workspaces/integrations/providers/codex/vendor/source.ts",
      ),
      "utf8",
    );
    const review = run(["provider-update", "codex", "0.0.0-test"]);
    assert.equal(review.status, 0, review.stdout + review.stderr);
    assert.match(review.stdout, /--reviewed-sha [a-f0-9]{64}/);
    const wrongVersion = run(["provider-update", "codex", "0.0.1-test"]);
    assert.equal(wrongVersion.status, 1);
    assert.match(wrongVersion.stderr, /Expected codex-cli 0.0.1-test/);
    const wrongSha = run([
      "provider-update",
      "codex",
      "0.0.0-test",
      "--reviewed-sha",
      "0".repeat(64),
    ]);
    assert.equal(wrongSha.status, 1);
    assert.match(wrongSha.stderr, /Reviewed SHA differs/);
    assert.equal(
      readFileSync(
        path.join(
          root,
          "workspaces/integrations/providers/codex/vendor/source.ts",
        ),
        "utf8",
      ),
      owner,
    );
    schema.definitions.v2.AgentMessageDeltaNotification.properties.delta = {
      type: "integer",
      format: "int64",
    };
    writeFileSync(schemaFile, JSON.stringify(schema));
    const changed = run();
    assert.equal(changed.status, 1, changed.stdout + changed.stderr);
    assert.match(
      changed.stdout,
      /types changed: AgentMessageDeltaNotification/,
    );
  } finally {
    rmSync(temporary, { recursive: true, force: true });
  }
});

// Compile the owning decisions without dependencies, so this witness remains
// cheap and does not contend for the workspace Cargo lock.
test("new provider controls, policies and identities require exhaustive decisions", () => {
  const copy = mkdtempSync(path.join(tmpdir(), "studio-provider-decisions-"));
  try {
    const identity = readFileSync(
      path.join(
        root,
        "workspaces/runtime/packages/runtime-schema/src/provider_kind.rs",
      ),
      "utf8",
    );
    const adapter = readFileSync(
      path.join(
        root,
        "workspaces/runtime/packages/runtime-services/src/provider_adapter.rs",
      ),
      "utf8",
    );
    const enumBody = (source: string, name: string) => {
      const body = source.match(
        new RegExp(`pub enum ${name} \\{[\\s\\S]*?\\n\\}`),
      )?.[0];
      assert.ok(body, `${name} owner`);
      return body;
    };
    const controls = identity.slice(
      identity.indexOf("impl ProviderCapability {"),
      identity.indexOf("/// How the session numbers"),
    );
    const policies = adapter
      .slice(
        adapter.indexOf("impl ProviderFeature {"),
        adapter.indexOf("/// Provider-neutral request preparation"),
      )
      .replaceAll(
        "studio_runtime_schema::provider_kind::ProviderSupport",
        "ProviderSupport",
      );
    // ProviderSupport is already in the controls segment, between the capability
    // keys and the provider decisions. Only the identity and feature enums are added.
    const baseline = [
      "#[derive(Clone, Copy)]",
      enumBody(identity, "ProviderKind"),
      "#[derive(Clone, Copy)]",
      enumBody(identity, "ProviderCapability"),
      controls.replaceAll(", Serialize, JsonSchema", ""),
      enumBody(adapter, "ProviderFeature"),
      policies,
    ].join("\n");
    const check = (text: string) => {
      writeFileSync(path.join(copy, "lib.rs"), text);
      const result = spawnSync(
        "rustc",
        [
          "--edition=2024",
          "--crate-type=lib",
          "--emit=metadata",
          "-o",
          path.join(copy, "lib.rmeta"),
          path.join(copy, "lib.rs"),
        ],
        { encoding: "utf8", timeout: 60_000 },
      );
      assert.equal(result.error, undefined);
      assert.equal(result.signal, null);
      return result;
    };
    const valid = check(baseline);
    assert.equal(valid.status, 0, valid.stderr);
    for (const name of [
      "ProviderCapability",
      "ProviderFeature",
      "ProviderKind",
    ]) {
      const result = check(
        baseline.replace(
          `pub enum ${name} {`,
          `pub enum ${name} {\n    FutureWitness,`,
        ),
      );
      assert.notEqual(result.status, 0, name);
      assert.match(result.stderr, /non-exhaustive patterns/, name);
      assert.match(result.stderr, /FutureWitness/, name);
    }
  } finally {
    rmSync(copy, { recursive: true, force: true });
  }
});

test("both fast-filter copies include the actual binding witness", () => {
  const config = readFileSync(path.join(root, ".config/nextest.toml"), "utf8");
  const filters = [
    ...config.matchAll(/default-filter\s*=\s*'''([\s\S]*?)'''/g),
  ];
  assert.equal(filters.length, 2);
  for (const filter of filters)
    assert.ok(
      filter[1]?.includes(
        "test(=notification_dispatch::review_tests::handled_dispositions_reference_production_handlers)",
      ),
    );
});
