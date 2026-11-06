// Manual UI responsiveness measurement on a throttled phone. It is not a test:
// numbers depend on the machine, so compare runs taken on the same machine.
// Usage: node scripts/ui-responsiveness.ts [--turns N] [--cpu RATE] [--runs N]
// Needs a built renderer (pnpm run build:bundle) and the Rust UI fixture
// (CODEX_STUDIO_RUNTIME_SERVER, as for the browser tests).
import { spawn } from "node:child_process";
import { mkdtempSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { parseArgs } from "node:util";
import { chromium, type CDPSession, type Page } from "playwright";
import { browserExecutablePath } from "../tests/browser-executable.ts";

const { values } = parseArgs({
  options: {
    turns: { type: "string", default: "30" },
    cpu: { type: "string", default: "4" },
    runs: { type: "string", default: "3" },
    profile: { type: "string", default: "" },
    trace: { type: "string", default: "" },
  },
});
const turns = Number(values.turns);
const web = join(import.meta.dirname, "..");
// A reply with the usual parts of an answer: prose, a list, code and a table.
const reply = (index: number) =>
  [
    `Turn ${index}: the release check found two items to review before the merge.`,
    "",
    "- The migration keeps the previous index until the backfill ends.",
    "- The client keeps its cached projection when the schema hash matches.",
    "",
    "```ts\nexport function settle(rows: Row[]) {\n  return rows.filter((row) => row.done).map((row) => row.id);\n}\n```",
    "",
    "| Area | Result |\n| :--- | :--- |\n| Delivery | Passed |\n| Limits | Passed |",
  ].join("\n");
const stream = Array.from({ length: 60 }, (_, index) => ({
  delta: `word${index} `,
}));
const scripts = [
  ...Array.from({ length: turns }, (_, index) => ({
    prompt: `Perf turn ${index + 1} end`,
    steps: [{ say: reply(index + 1) }],
  })),
  { prompt: "Perf stream", steps: [...stream, { say: reply(0) }] },
];

const fixture = spawn(
  process.execPath,
  [
    "tests/fixtures/rust-ui-fixture.ts",
    mkdtempSync(join(tmpdir(), "ui-perf-")),
    "--workers",
    "0",
  ],
  {
    cwd: web,
    stdio: ["pipe", "pipe", "ignore"],
    env: { ...process.env, FIXTURE_SCRIPT: JSON.stringify(scripts) },
  },
);
const port = await new Promise<number>((resolve, reject) => {
  fixture.stdout.once("data", (chunk) => resolve(Number(String(chunk).trim())));
  fixture.once("exit", () => reject(new Error("fixture exited")));
});
const origin = `http://127.0.0.1:${port}`;
const browser = await chromium.launch({
  executablePath: browserExecutablePath,
});

/** Frame and long-task recorder installed in the page. */
const recorder = () => {
  const state = { frames: [] as number[], long: [] as number[], on: false };
  new PerformanceObserver((list) => {
    if (state.on)
      for (const entry of list.getEntries()) state.long.push(entry.duration);
  }).observe({ type: "longtask", buffered: false });
  let last = 0;
  const tick = (now: number) => {
    if (state.on && last) state.frames.push(now - last);
    last = now;
    requestAnimationFrame(tick);
  };
  requestAnimationFrame(tick);
  Object.assign(window, { __perf: state });
};
type Sample = { frames: number[]; long: number[] };
declare global {
  interface Window {
    /** The page recorder state of this measurement script. */
    __perf: Sample & { on: boolean };
  }
}
const profiled = new Set(values.profile.split(",").filter(Boolean));
const traced = new Set(values.trace.split(",").filter(Boolean));
/** Main-thread time per trace event name (style, layout, paint, script). */
async function traceTotals(session: CDPSession, run: () => Promise<void>) {
  const events: { name: string; dur?: number; ph: string; tid: number }[] = [];
  session.on("Tracing.dataCollected", ({ value }) =>
    events.push(...(value as never[])),
  );
  const done = new Promise<void>((resolve) =>
    session.once("Tracing.tracingComplete", () => resolve()),
  );
  await session.send("Tracing.start", {
    categories: "devtools.timeline,disabled-by-default-devtools.timeline",
    transferMode: "ReportEvents",
  });
  await run();
  await session.send("Tracing.end");
  await done;
  const totals = new Map<string, number>();
  for (const event of events)
    if (event.ph === "X" && event.dur)
      totals.set(event.name, (totals.get(event.name) ?? 0) + event.dur / 1000);
  return [...totals].sort((a, b) => b[1] - a[1]).slice(0, 14);
}
let profileSession: CDPSession | undefined;
/** Self time per function, summed over the profile, largest first. */
function topSelfTime(profile: {
  nodes: {
    id: number;
    callFrame: { functionName: string; url: string; lineNumber: number };
  }[];
  samples: number[];
  timeDeltas: number[];
}) {
  const byNode = new Map(profile.nodes.map((node) => [node.id, node]));
  const parent = new Map<number, number>();
  for (const node of profile.nodes as { id: number; children?: number[] }[])
    for (const child of node.children ?? []) parent.set(child, node.id);
  const label = (id: number) => {
    const frame = byNode.get(id)!.callFrame;
    return `${frame.functionName || "(anonymous)"} ${frame.url.split("/").pop()}:${frame.lineNumber + 1}`;
  };
  const self = new Map<string, number>();
  const total = new Map<string, number>();
  profile.samples.forEach((id, index) => {
    const ms = (profile.timeDeltas[index] ?? 0) / 1000;
    self.set(label(id), (self.get(label(id)) ?? 0) + ms);
    // Inclusive time: each function on the sampled stack, once.
    const seen = new Set<string>();
    for (
      let node: number | undefined = id;
      node !== undefined;
      node = parent.get(node)
    ) {
      const name = label(node);
      if (seen.has(name) || name.startsWith("(root)")) continue;
      seen.add(name);
      total.set(name, (total.get(name) ?? 0) + ms);
    }
  });
  const top = (map: Map<string, number>) =>
    [...map].sort((a, b) => b[1] - a[1]).slice(0, 200);
  return { self: top(self), total: top(total) };
}
let currentName = "";
async function measure(
  page: Page,
  action: () => Promise<void>,
  name = currentName,
) {
  const profiling = profiled.has(name) && profileSession;
  if (profiling) {
    await profileSession!.send("Profiler.enable");
    await profileSession!.send("Profiler.start");
  }
  await page.evaluate(() => {
    const state = window.__perf;
    state.frames = [];
    state.long = [];
    state.on = true;
  });
  const start = Date.now();
  if (traced.has(name) && profileSession) {
    const totals = await traceTotals(profileSession, action);
    console.error(`trace of ${name}:`);
    for (const [event, time] of totals)
      console.error(`  ${time.toFixed(0).padStart(6)} ms  ${event}`);
  } else await action();
  const ms = Date.now() - start;
  if (profiling) {
    const { profile } = await profileSession!.send("Profiler.stop");
    const { self, total } = topSelfTime(profile as never);
    console.error(`profile of ${name}, self time:`);
    for (const [fn, time] of self)
      console.error(`  ${time.toFixed(0).padStart(6)} ms  ${fn}`);
    console.error(`profile of ${name}, total time:`);
    for (const [fn, time] of total)
      console.error(`  ${time.toFixed(0).padStart(6)} ms  ${fn}`);
  }
  const sample = await page.evaluate(() => {
    const state = window.__perf;
    state.on = false;
    return { frames: state.frames, long: state.long };
  });
  const slow = sample.frames.filter((frame) => frame > 20);
  return {
    ms,
    frames: sample.frames.length,
    slow: slow.length,
    worst: Math.round(Math.max(0, ...sample.frames)),
    longTasks: sample.long.length,
    blocked: Math.round(
      sample.long.reduce(
        (sum, duration) => sum + Math.max(0, duration - 50),
        0,
      ),
    ),
  };
}
const settle = (page: Page, ms = 400) =>
  page.evaluate((ms) => new Promise((resolve) => setTimeout(resolve, ms)), ms);

async function phone() {
  const context = await browser.newContext({
    viewport: { width: 390, height: 844 },
    hasTouch: true,
    isMobile: true,
    deviceScaleFactor: 3,
  });
  await context.addInitScript(recorder);
  const page = await context.newPage();
  const cdp: CDPSession = await context.newCDPSession(page);
  return { context, page, cdp };
}

// Setup: send every scripted turn once, without throttling.
console.error(`setup: ${turns} turns`);
{
  const { context, page } = await phone();
  await page.goto(origin);
  await page.locator("[data-chat]", { hasText: "Release lead" }).click();
  const input = page.locator("#message");
  for (let index = 1; index <= turns; index++) {
    await input.fill(`Perf turn ${index} end`);
    await page.getByRole("button", { name: "Send message" }).click();
    await page
      .getByText(`Turn ${index}: the release check`)
      .first()
      .waitFor({ timeout: 30_000 });
  }
  await context.close();
}

const results: Record<
  string,
  ReturnType<typeof measure> extends Promise<infer R> ? R[] : never
> = {};
const record = (name: string, value: Awaited<ReturnType<typeof measure>>) => {
  console.error(`${name}: ${value.ms} ms`);
  (results[name] ??= []).push(value);
};

for (let run = 0; run < Number(values.runs); run++) {
  const { context, page, cdp } = await phone();
  profileSession = cdp;
  await page.goto(origin);
  await page.locator("[data-chat]").first().waitFor();
  await settle(page, 1500);
  await cdp.send("Emulation.setCPUThrottlingRate", {
    rate: Number(values.cpu),
  });

  console.error(`run ${run + 1}`);
  currentName = "reload to chat list";
  record(
    "reload to chat list",
    await measure(page, async () => {
      await page.reload();
      await page
        .locator("[data-chat]", { hasText: "Release lead" })
        .first()
        .waitFor();
    }),
  );
  await settle(page, 1000);
  currentName = "open long chat";
  record(
    "open long chat",
    await measure(page, async () => {
      await page
        .locator("[data-chat]", { hasText: "Release lead" })
        .first()
        .click();
      // Later runs end with the streamed reply, so wait for the latest reply.
      await page
        .getByText(/^Turn \d+: the release check/)
        .last()
        .waitFor();
    }),
  );
  await settle(page, 1000);
  currentName = "touch scroll up and down";
  record(
    "touch scroll up and down",
    await measure(page, async () => {
      for (const distance of [-2400, 2400])
        await cdp.send("Input.synthesizeScrollGesture", {
          x: 195,
          y: 420,
          yDistance: -distance,
          speed: 1600,
          gestureSourceType: "touch",
        });
    }),
  );
  await settle(page);
  currentName = "model picker open, roles, close";
  record(
    "model picker open, roles, close",
    await measure(page, async () => {
      await page
        .getByRole("button", { name: /^Execution settings/ })
        .first()
        .tap();
      await page.locator(".setup-sheet .mantine-Drawer-content").waitFor();
      for (const role of ["Workers", "Review", "Lead"])
        await page
          .locator(".setup-sheet .mantine-SegmentedControl-label")
          .filter({ hasText: new RegExp(`^${role}$`) })
          .tap();
      await page.keyboard.press("Escape");
      await page
        .locator(".setup-sheet .mantine-Drawer-content")
        .waitFor({ state: "hidden" });
    }),
  );
  await settle(page);
  currentName = "type 30 characters";
  record(
    "type 30 characters",
    await measure(page, async () => {
      await page.locator("#message").tap();
      await page.keyboard.type("Check the release notes again.", { delay: 60 });
    }),
  );
  await page.locator("#message").fill("");
  await settle(page);
  currentName = "streamed reply";
  record(
    "streamed reply",
    await measure(page, async () => {
      await page.locator("#message").fill("Perf stream");
      await page.getByRole("button", { name: "Send message" }).tap();
      await page
        .getByText("Turn 0: the release check")
        .first()
        .waitFor({ timeout: 30_000 });
    }),
  );
  await context.close();
}

const median = (values: number[]) =>
  [...values].sort((a, b) => a - b)[Math.floor(values.length / 2)]!;
console.log(
  `turns=${turns} cpu=${values.cpu}x runs=${values.runs} (median per scenario)`,
);
for (const [name, samples] of Object.entries(results)) {
  const pick = (key: keyof (typeof samples)[number]) =>
    median(samples.map((sample) => sample[key]));
  console.log(
    `${name.padEnd(34)} ${String(pick("ms")).padStart(6)} ms  slow frames ${String(pick("slow")).padStart(3)}/${String(pick("frames")).padEnd(4)} worst ${String(pick("worst")).padStart(4)} ms  long tasks ${pick("longTasks")} blocked ${pick("blocked")} ms`,
  );
}
await browser.close();
fixture.kill("SIGTERM");
