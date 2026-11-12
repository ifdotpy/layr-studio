// @ts-check
// Stable bridge entrypoint; this check precedes every TypeScript import.
if (!process.features.typescript) {
  console.error(
    "Claude bridge requires Node.js with built-in TypeScript support enabled. See docs/typescript.md#how-typescript-runs.",
  );
  process.exit(1);
}
await import("./bridge.ts");
