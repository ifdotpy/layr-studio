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
  checkClaudeRateContract,
} from "../generate-provider-contracts.ts";
import { codexContractSource } from "../provider-contract-sources.ts";

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
    // The gate supplies its workspace target. This dependency-free witness must
    // use its own temporary target, so it never waits behind the workspace build.
    const env: NodeJS.ProcessEnv = {
      ...process.env,
      CARGO_HOME: path.join(copy, "cargo-home"),
    };
    delete env.CARGO_TARGET_DIR;
    const check = () => {
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
          return body.replace(/^\s*#\[serde[^\n]*\n/gm, "");
        })
        .join("\n");
      writeFileSync(
        path.join(copy, "src/lib.rs"),
        `pub mod studio_codex_protocol {\n${enums}\n}\n${disposition}`,
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
      "workspaces/tooling/apps/repository-checks/tests/fixtures/provider-drift.fixture.ts",
    );
    const schemaFile = path.join(temporary, "schema.json");
    const schema = JSON.parse(
      readFileSync(path.join(root, codexContractSource.schemaFile), "utf8"),
    );
    writeFileSync(schemaFile, JSON.stringify(schema));
    const run = () => {
      const result = spawnSync("just", ["provider-drift"], {
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
