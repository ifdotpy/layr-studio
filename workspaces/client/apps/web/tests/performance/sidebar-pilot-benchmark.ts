// Manual pilot comparison. Preparation is the default; only --run starts Chromium.
import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import { createServer } from "node:http";
import { isAbsolute, join, relative, resolve } from "node:path";
import { gzipSync, brotliCompressSync } from "node:zlib";
import { build, transform } from "esbuild";
import { chromium, type Browser } from "playwright";
import { browserExecutablePath } from "../browser-executable.ts";
import { required } from "./performance-fixture.ts";
import { pilotAgents, pilotSize, pilotUpdate } from "./sidebar-pilot-data.ts";

const base = "947e17dfb3209bc90afac82a555d82a036a7240f";
const web = resolve(import.meta.dirname, "../..");
const repo = execFileSync("git", ["rev-parse", "--show-toplevel"], {
  cwd: web,
  encoding: "utf8",
}).trim();
const args = process.argv.slice(2);
const run = args.includes("--run");
const output = args.find((arg) => arg !== "--run");
if (
  !output ||
  !isAbsolute(output) ||
  !relative(repo, output).startsWith("..") ||
  args.length !== (run ? 2 : 1)
)
  throw new Error(
    "Usage: node tests/performance/sidebar-pilot-benchmark.ts ABSOLUTE_OUTPUT_OUTSIDE_REPO [--run]",
  );
await mkdir(output, { recursive: true });
const currentState = execFileSync("layr", ["rev-parse", "head"], {
  cwd: repo,
  encoding: "utf8",
}).trim();
const baselineSource = (path: string) =>
  execFileSync("git", ["show", `${base}:${relative(repo, path)}`], {
    cwd: repo,
    encoding: "utf8",
    maxBuffer: 16 * 1024 * 1024,
  });
const source = (path: string, baseline: boolean) =>
  baseline ? baselineSource(path) : readFile(path, "utf8");
const entry = join(import.meta.dirname, "./sidebar-pilot.fixture.tsx");
const variants = [];
for (const label of ["base", "integrated"]) {
  const baseline = label === "base";
  const probes = new Set<string>();
  const sources: Record<string, string> = {};
  const bundled = await build({
    entryPoints: [entry],
    absWorkingDir: web,
    bundle: true,
    write: false,
    minify: true,
    jsx: "automatic",
    format: "esm",
    platform: "browser",
    outfile: join(output, label, "fixture.js"),
    loader: { ".woff2": "file", ".wasm": "file" },
    define: { "process.env.NODE_ENV": '"production"' },
    plugins: [
      {
        name: "sidebar-pilot-sources",
        setup(builder) {
          builder.onResolve({ filter: /\?worker&url$/ }, (args) => ({
            path: resolve(
              args.resolveDir,
              args.path.replace(/\?worker&url$/, ""),
            ),
            namespace: "pilot-worker",
          }));
          builder.onLoad(
            { filter: /.*/, namespace: "pilot-worker" },
            async (args) => {
              const contents = await source(args.path, baseline);
              sources[relative(repo, args.path)] = createHash("sha256")
                .update(contents)
                .digest("hex");
              const script = await transform(contents, {
                loader: "ts",
                minify: true,
              });
              return {
                contents: `export default ${JSON.stringify("data:text/javascript;base64," + Buffer.from(script.code).toString("base64"))}`,
                loader: "js",
              };
            },
          );
          builder.onResolve({ filter: /^\/src\// }, (args) => ({
            path: join(web, args.path),
          }));
          builder.onLoad(
            { filter: /sidebar-pilot-model\.ts$/ },
            async (args) => {
              let contents = await readFile(
                baseline
                  ? join(import.meta.dirname, "./sidebar-pilot-base.ts")
                  : args.path,
                "utf8",
              );
              if (baseline)
                contents = contents.replace(
                  /declare const baselineChatIndicators:[\s\S]*?;/,
                  'import { chatIndicators as baselineChatIndicators } from "/src/components/sidebar/chat-status/support/chatActivity.ts";',
                );
              return { contents, loader: "ts" };
            },
          );
          builder.onLoad({ filter: /\/src\/.*\.(tsx?|css)$/ }, async (args) => {
            if (!args.path.startsWith(join(web, "src/"))) return undefined;
            let contents = await source(args.path, baseline);
            sources[relative(repo, args.path)] = createHash("sha256")
              .update(contents)
              .digest("hex");
            let instrumented = false;
            for (const component of ["sidebar", "sidebar-row"]) {
              const expression = new RegExp(
                `reportPromptComposerRender\\("${component}"(?:, row.id)?\\);`,
                "g",
              );
              contents = contents.replace(expression, () => {
                probes.add(component);
                instrumented = true;
                return `pilotCommit(() => window.sidebarPilot.count("${component}"));`;
              });
            }
            if (
              /\/(chat-row\/.*|agents\/ChatStatus|sidebar\/LayrCopyProgress)\.tsx$/.test(
                args.path,
              )
            ) {
              contents = contents.replace(
                /(observer\(function (\w+)\([\s\S]*?\) \{)/g,
                (header: string, _match: string, name: string) => {
                  probes.add(name);
                  instrumented = true;
                  return `${header}\npilotCommit(() => window.sidebarPilot.count("${name}"));`;
                },
              );
              // The baseline status leaf predates observer wrapping.
              contents = contents.replace(
                /(export default function ChatStatus\([\s\S]*?\) \{)/,
                (header) => {
                  probes.add("ChatStatus");
                  instrumented = true;
                  return `${header}\npilotCommit(() => window.sidebarPilot.count("ChatStatus"));`;
                },
              );
              contents = contents.replace(
                /(function RowActionTarget\([\s\S]*?\) \{)/,
                (header) => {
                  probes.add("RowActionTarget");
                  instrumented = true;
                  return `${header}\npilotCommit(() => window.sidebarPilot.count("RowActionTarget"));`;
                },
              );
            }
            if (instrumented)
              contents =
                'import { useLayoutEffect as pilotCommit } from "react";\n' +
                contents;
            return {
              contents,
              loader: args.path.endsWith(".tsx")
                ? "tsx"
                : args.path.endsWith(".css")
                  ? "css"
                  : "ts",
            };
          });
        },
      },
    ],
  });
  assert(
    probes.has("sidebar") &&
      probes.has("sidebar-row") &&
      probes.has("ChatStatus"),
    "Missing actual component commit probes",
  );
  const bytes: Record<
    string,
    { uncompressed: number; gzip: number; brotli: number }
  > = {};
  for (const file of bundled.outputFiles) {
    const name = relative(join(output, label), file.path);
    await mkdir(join(output, label), { recursive: true });
    await writeFile(file.path, file.contents);
    bytes[name] = {
      uncompressed: file.contents.length,
      gzip: gzipSync(file.contents).length,
      brotli: brotliCompressSync(file.contents).length,
    };
  }
  variants.push({
    label,
    revision: baseline ? base : currentState,
    probes: [...probes].sort(),
    bytes,
    sources,
  });
}
// Deterministic preparation witnesses: size, retained compact membership and sequence.
let agents = pilotAgents();
assert.equal(agents.length, pilotSize.chats);
assert.equal(
  agents.filter((agent) => (agent.updated || 0) > 1).length,
  pilotSize.visible,
);
const initial = agents;
for (let i = 0; i < pilotSize.updates; i++) agents = pilotUpdate(agents, i);
assert.equal(
  agents.filter((agent, i) => agent !== initial[i]).length,
  pilotSize.visible,
);
assert.equal(agents[0]?.name, "Updated 90");
assert.equal(agents[29]?.name, "Updated 119");
await writeFile(
  join(output, "variants.json"),
  JSON.stringify({ base, currentState, fixture: pilotSize, variants }, null, 2),
);
console.log(
  JSON.stringify(
    {
      mode: run ? "run" : "dry-run",
      fixture: pilotSize,
      variants: variants.map(({ sources: _sources, ...variant }) => variant),
    },
    null,
    2,
  ),
);
if (!run) {
  console.log(
    "PASS: both bundles and fixture sequence validated; measured phases refused without --run; Chromium was not started.",
  );
} else {
  console.log(
    "Manual measurement: at most 10 minutes, one headless Chromium page at a time, no CPU throttling or synthetic load. Run only in the lead-granted window.",
  );
  const server = createServer(async (request, response) => {
    const path = new URL(required(request.url), "http://fixture").pathname;
    const match =
      /^\/(base|integrated)\/([\w.-]+\.(?:js|css|wasm|woff2))$/.exec(path);
    if (match) {
      response.setHeader(
        "Content-Type",
        path.endsWith(".wasm")
          ? "application/wasm"
          : path.endsWith(".woff2")
            ? "font/woff2"
            : path.endsWith(".css")
              ? "text/css"
              : "text/javascript",
      );
      response.end(
        await readFile(join(output, required(match[1]), required(match[2]))),
      );
    } else if (/^\/(base|integrated)\/$/.test(path)) {
      response.setHeader("Content-Type", "text/html");
      response.end(
        '<div id="root"></div><link rel="stylesheet" href="fixture.css"><script type="module" src="fixture.js"></script>',
      );
    } else {
      response.statusCode = 404;
      response.end();
    }
  });
  await new Promise<void>((resolve) => server.listen(0, "127.0.0.1", resolve));
  const address = server.address();
  assert(address && typeof address === "object");
  let browser: Browser | undefined;
  const deadline = setTimeout(
    () => {
      void browser?.close();
      server.closeAllConnections();
      server.close();
    },
    10 * 60 * 1000,
  );
  try {
    browser = await chromium.launch({
      headless: true,
      executablePath: browserExecutablePath,
    });
    const results = [];
    for (const variant of variants) {
      // A new context isolates caches and retained heaps; its page closes before the next opens.
      const context = await browser.newContext({
        viewport: { width: 1280, height: 2400 },
      });
      try {
        const page = await context.newPage();
        const errors: string[] = [];
        page.on("pageerror", (error) => errors.push(error.message));
        await page.route("**/api/**", (route) => route.abort());
        await page.goto(`http://127.0.0.1:${address.port}/${variant.label}/`);
        await page.waitForFunction(() => window.sidebarPilot?.ready);
        const fixture = await page.evaluate(() =>
          window.sidebarPilot.validate(),
        );
        const zeroCounts = Object.fromEntries(
          variant.probes.map((name) => [name, 0]),
        );
        const changed = await page.evaluate(() =>
          window.sidebarPilot.phase(false),
        );
        changed.commits = { ...zeroCounts, ...changed.commits };
        const equalReplay = await page.evaluate(() =>
          window.sidebarPilot.phase(true),
        );
        equalReplay.commits = { ...zeroCounts, ...equalReplay.commits };
        const cdp = await context.newCDPSession(page);
        await cdp.send("HeapProfiler.collectGarbage");
        const heap = await cdp.send("Runtime.getHeapUsage");
        assert.deepEqual(errors, []);
        const result = {
          label: variant.label,
          revision: variant.revision,
          fixture,
          conversation:
            "memoized sibling guard; actual Conversation is not mounted",
          changed,
          equalReplay,
          retainedJsHeapBytes: heap.usedSize,
          bundleBytes: variant.bytes,
        };
        results.push(result);
        console.log(JSON.stringify(result));
      } finally {
        await context.close();
      }
    }
    await writeFile(
      join(output, "results.json"),
      JSON.stringify(results, null, 2),
    );
  } finally {
    clearTimeout(deadline);
    await browser?.close();
    server.closeAllConnections();
    await new Promise<void>((resolve) => server.close(() => resolve()));
  }
}
