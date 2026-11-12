#!/usr/bin/env node
// Offline vendor binary/source fixture for the actual just provider-drift recipe.
import { readFileSync, writeFileSync } from "node:fs";
import path from "node:path";
import { codexContractSource } from "../../vendor/source.ts";

if (process.argv.includes("--version")) {
  process.stdout.write("codex-cli 0.0.0-test\n");
  process.exit(0);
} else if (process.argv.includes("generate-json-schema")) {
  const out = process.argv[process.argv.indexOf("--out") + 1];
  const schema = process.env.STUDIO_TEST_CODEX_SCHEMA;
  if (!out || !schema) throw new Error("Missing offline Codex fixture input");
  writeFileSync(
    path.join(out, "codex_app_server_protocol.schemas.json"),
    readFileSync(schema),
  );
  process.exit(0);
} else {
  const vendor = Object.entries(codexContractSource.responseTypes)
    .map(
      ([method, response], index) =>
        `    Method${index} => "${method}" {\n        response: ${response},\n    }`,
    )
    .join("\n");
  globalThis.fetch = async () => new Response(vendor, { status: 200 });
}
