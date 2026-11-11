import { readFileSync } from "node:fs";
import { createRequire } from "node:module";
import path from "node:path";
import { z } from "zod";

const require = createRequire(import.meta.url);
const file = path.join(
  path.dirname(require.resolve("@anthropic-ai/claude-agent-sdk")),
  "package.json",
);
const value: unknown = JSON.parse(readFileSync(file, "utf8"));
// Installed package version, distinct from the declaration/disposition pin.
export const CLAUDE_RUNTIME_SDK_VERSION = z
  .object({ version: z.string() })
  .parse(value).version;
