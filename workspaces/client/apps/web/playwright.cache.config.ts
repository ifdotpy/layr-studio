import { defineConfig } from "@playwright/test";
import { browserExecutablePath } from "./tests/browser-executable.ts";
export default defineConfig({
  testDir: ".",
  testMatch: "tests/cache/subscription-cache.spec.ts",
  workers: 1,
  use: {
    browserName: "chromium",
    headless: true,
    launchOptions: { executablePath: browserExecutablePath },
  },
});
