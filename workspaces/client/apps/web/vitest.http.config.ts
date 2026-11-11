import base from "./vitest.unit.config.ts";

// These witnesses start the Rust draft server and keep its startup deadline.
export const HTTP_FIXTURE_TESTS = [
  "src/renameRequest.test.ts",
  "src/sync/client.entityServer.test.ts",
  "src/sync/client.terminalServer.test.ts",
];

export default {
  ...base,
  test: { ...base.test, name: "http", include: HTTP_FIXTURE_TESTS },
};
