#!/usr/bin/env node
// Layout-shift matrix: a manual diagnostic, outside the test tiers.
//
// A graph node is one layout configuration of the UI (sidebar, right panel,
// terminals, draft) on one device profile. Each node gets the edges of a full
// agent turn, driven through the fixture provider without user input, and
// toggle edges to its neighbor configurations. Every edge records layout-shift
// entries with their sources and the moves of landmark controls (header,
// composer, footer, sidebar rows, the first visible message).
//
// Usage: node scripts/layout-shift-matrix.ts [--out DIR] [--profile NAME,...]
//   [--chromium PATH]. Needs `pnpm run build` (dist) and a debug
//   studio-runtime-server (CODEX_STUDIO_RUNTIME_SERVER or the newest cargo
//   build). Writes DIR/<profile>.json and DIR/index.html.
import { spawn, type ChildProcess } from "node:child_process";
import { mkdirSync, mkdtempSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { parseArgs } from "node:util";
import { chromium } from "playwright";

type Profile = {
  name: string;
  width: number;
  height: number;
  touch: boolean;
  /** Visual viewport height while the on-screen keyboard is open, in px. */
  keyboard?: number;
  /** Safe insets the native iOS shell supplies, which show the edge rail, in px. */
  shell?: { right: number; bottom: number };
};
type Config = {
  sidebar: string;
  panel: string;
  terminals: string;
  draft: string;
};
type Rect = [number, number, number, number];
type Source = {
  node: string;
  dx: number;
  dy: number;
  dw: number;
  dh: number;
  input: boolean;
};
type Edge = {
  node: string;
  action: string;
  byUser: boolean;
  error: string | null;
  cls: number;
  inputCls: number;
  sources: Source[];
  moved: { name: string; delta: number[] }[];
};
type ShiftEntry = {
  edge: string;
  value: number;
  input: boolean;
  sources: Source[];
};
declare global {
  interface Window {
    layoutShifts: ShiftEntry[];
    layoutEdge: string;
    layoutKeyboard?: (open: boolean) => void;
  }
}

export const profiles: Profile[] = [
  { name: "desktop-1440", width: 1440, height: 900, touch: false },
  { name: "desktop-1024", width: 1024, height: 768, touch: false },
  { name: "phone-390", width: 390, height: 844, touch: true },
  {
    name: "phone-390-keyboard",
    width: 390,
    height: 844,
    touch: true,
    keyboard: 508,
  },
  { name: "duo-open", width: 867, height: 669, touch: true },
  {
    name: "duo-open-keyboard",
    width: 867,
    height: 669,
    touch: true,
    keyboard: 323,
  },
  {
    name: "duo-shell",
    width: 951,
    height: 669,
    touch: true,
    shell: { right: 84, bottom: 21 },
  },
];
const web = resolve(fileURLToPath(import.meta.url), "../..");
const sleep = (ms: number) => new Promise((done) => setTimeout(done, ms));

/** Starts the Rust UI fixture with one held prompt and returns its origin. */
async function startFixture(): Promise<{
  fixture: ChildProcess;
  origin: string;
}> {
  const root = mkdtempSync(
    join(process.env.TMPDIR || tmpdir(), "layout-matrix-"),
  );
  const fixture = spawn(
    process.execPath,
    ["tests/fixtures/rust-ui-fixture.ts", root, "--workers", "3"],
    {
      cwd: web,
      stdio: ["pipe", "pipe", "pipe"],
      env: {
        ...process.env,
        FIXTURE_SCRIPT: JSON.stringify([
          { prompt: "Probe turn", steps: [{ hold: true }] },
        ]),
      },
    },
  );
  let log = "";
  fixture.stderr?.on("data", (chunk) => (log += chunk));
  const port = await new Promise<number>((done, fail) => {
    fixture.stdout?.once("data", (chunk) => done(Number(String(chunk).trim())));
    fixture.once("exit", () => fail(new Error(log.slice(-2000))));
  });
  return { fixture, origin: `http://127.0.0.1:${port}` };
}

/** Records layout shifts per edge and, for keyboard profiles, owns a fake visual viewport. */
function installProbe(keyboard: number | null) {
  window.layoutShifts = [];
  window.layoutEdge = "load";
  const describe = (node: Node | null) => {
    if (!(node instanceof Element)) return String(node?.nodeName);
    const parent = node.parentElement;
    const owner = parent
      ? parent.id
        ? `#${parent.id}`
        : parent.classList[0]
          ? `.${parent.classList[0]}`
          : parent.tagName.toLowerCase()
      : "";
    const classes = [...node.classList]
      .slice(0, 2)
      .map((name) => `.${name}`)
      .join("");
    return `${owner} > ${node.tagName.toLowerCase()}${node.id ? `#${node.id}` : ""}${classes}`;
  };
  new PerformanceObserver((list) => {
    for (const entry of list.getEntries() as (PerformanceEntry & {
      value: number;
      hadRecentInput: boolean;
      sources?: {
        node: Node | null;
        previousRect: DOMRectReadOnly;
        currentRect: DOMRectReadOnly;
      }[];
    })[])
      window.layoutShifts.push({
        edge: window.layoutEdge,
        value: entry.value,
        input: entry.hadRecentInput,
        sources: (entry.sources || []).map((source) => ({
          node: describe(source.node),
          dx: Math.round(source.currentRect.x - source.previousRect.x),
          dy: Math.round(source.currentRect.y - source.previousRect.y),
          dw: Math.round(source.currentRect.width - source.previousRect.width),
          dh: Math.round(
            source.currentRect.height - source.previousRect.height,
          ),
          input: entry.hadRecentInput,
        })),
      });
  }).observe({ type: "layout-shift", buffered: true });
  if (keyboard === null) return;
  const full = window.innerHeight;
  const viewport = Object.assign(new EventTarget(), {
    width: window.innerWidth,
    height: full,
    offsetTop: 0,
    pageTop: 0,
    offsetLeft: 0,
    pageLeft: 0,
    scale: 1,
  });
  let inner = full;
  Object.defineProperty(window, "innerHeight", {
    configurable: true,
    get: () => inner,
  });
  Object.defineProperty(window, "visualViewport", {
    configurable: true,
    value: viewport,
  });
  window.layoutKeyboard = (open) => {
    viewport.height = open ? keyboard : full;
    inner = viewport.height;
    viewport.dispatchEvent(new Event("resize"));
    window.dispatchEvent(new Event("resize"));
  };
}

function landmarks(): Record<string, Rect> {
  const out: Record<string, Rect> = {};
  const add = (name: string, node: Element | null | undefined) => {
    if (!node) return;
    const r = node.getBoundingClientRect();
    if (!r.width || r.bottom < 0 || r.top > innerHeight) return;
    out[name] = [r.x, r.y, r.width, r.height].map(Math.round) as Rect;
  };
  const label = (node: Element, index: number) =>
    node.getAttribute("aria-label")?.slice(0, 24) ||
    node.textContent?.trim().slice(0, 14) ||
    String(index);
  document
    .querySelectorAll(
      ".workspace-header button, .workspace-header .workspace-mode-badge",
    )
    .forEach((node, i) => add(`header:${label(node, i)}`, node));
  add("composer", document.querySelector("#composer"));
  add("input", document.querySelector("#message"));
  document
    .querySelectorAll("#composer button")
    .forEach((node, i) => add(`composer:${label(node, i)}`, node));
  document
    .querySelectorAll(".usage-footer > *")
    .forEach((node, i) => add(`footer:${label(node, i)}`, node));
  document
    .querySelectorAll(".sidebar-row")
    .forEach((node, i) => add(`sidebar:${label(node, i)}`, node));
  document
    .querySelectorAll(".edge-rail button")
    .forEach((node, i) => add(`rail:${label(node, i)}`, node));
  const messages = document.querySelector("#messages");
  if (messages) {
    const top = messages.getBoundingClientRect().top;
    add(
      "first-visible-message",
      [...messages.querySelectorAll(".message")].find(
        (m) => m.getBoundingClientRect().bottom > top + 4,
      ),
    );
  }
  return out;
}

async function runProfile(profile: Profile, chromiumPath: string | undefined) {
  const { fixture, origin } = await startFixture();
  const browser = await chromium.launch(
    chromiumPath ? { executablePath: chromiumPath } : {},
  );
  try {
    const context = await browser.newContext({
      viewport: { width: profile.width, height: profile.height },
      hasTouch: profile.touch,
      isMobile: profile.touch && profile.width < 760,
    });
    const page = await context.newPage();
    await page.addInitScript(installProbe, profile.keyboard ?? null);
    await page.goto(origin);
    if (profile.shell)
      await page.addStyleTag({
        content: `:root{--native-safe-top:0px!important;--native-safe-left:0px!important;--native-safe-right:${profile.shell.right}px!important;--native-safe-bottom:${profile.shell.bottom}px!important}`,
      });
    await page.waitForSelector("[data-chat]", { timeout: 30_000 });
    await sleep(1500);
    const lead = await page.evaluate(
      () =>
        [...document.querySelectorAll("[data-chat]")]
          .find((node) => node.textContent?.includes("Release lead"))
          ?.getAttribute("data-chat") ?? "",
    );
    const control = (steps: unknown[]) =>
      fixture.stdin?.write(
        JSON.stringify({ method: "fixture/provider", agent: lead, steps }) +
          "\n",
      );
    const edges: Edge[] = [];
    const measure = async (
      node: string,
      action: string,
      byUser: boolean,
      run: () => Promise<unknown>,
      settle = 1000,
    ) => {
      const edge = `${node}::${action}`;
      await page.evaluate((name) => (window.layoutEdge = name), edge);
      const before = await page.evaluate(landmarks);
      let error: string | null = null;
      try {
        await run();
      } catch (failure) {
        error = String(failure).split("\n")[0]!.slice(0, 160);
      }
      await sleep(settle);
      const after = await page.evaluate(landmarks);
      await page.evaluate(() => (window.layoutEdge = "layout"));
      const moved = Object.keys(before)
        .filter(
          (name) =>
            after[name] &&
            before[name]!.some((v, i) => Math.abs(v - after[name]![i]!) > 1),
        )
        .map((name) => ({
          name,
          delta: before[name]!.map((v, i) => after[name]![i]! - v),
        }));
      edges.push({
        node,
        action,
        byUser,
        error,
        cls: 0,
        inputCls: 0,
        sources: [],
        moved,
      });
    };
    const phoneLike = profile.touch;
    const visible = (selector: string) =>
      page.evaluate((s) => {
        const node = document.querySelector(s);
        if (!node) return false;
        const r = node.getBoundingClientRect();
        return r.width > 0 && r.height > 0;
      }, selector);
    const button = (name: string) =>
      page.getByRole("button", { name, exact: true }).first();
    const state = async (): Promise<Config> => ({
      sidebar: phoneLike
        ? "-"
        : (await visible(".sidebar-frame"))
          ? "open"
          : "closed",
      panel: (await visible("#team"))
        ? "team"
        : (await page.evaluate(() =>
              [
                ...document.querySelectorAll(
                  ".workspace-panel-header, .mantine-Drawer-title",
                ),
              ].some(
                (node) =>
                  node.getBoundingClientRect().width > 0 &&
                  /Messages/.test(node.textContent || ""),
              ),
            ))
          ? "messages"
          : "none",
      terminals: phoneLike
        ? "-"
        : (await page.getByRole("button", { name: "Hide terminals" }).count())
          ? "open"
          : "closed",
      draft: (await page
        .locator("#message")
        .inputValue()
        .catch(() => ""))
        ? "draft"
        : "empty",
    });
    const name = (config: Config) =>
      Object.entries(config)
        .filter(([, value]) => value !== "-")
        .map(([key, value]) => `${key}=${value}`)
        .join(" ");
    const setLayout = async (target: Config) => {
      for (let attempt = 0; attempt < 2; attempt++) {
        const now = await state();
        if (!phoneLike && now.sidebar !== target.sidebar) {
          await page.keyboard.press("Meta+b");
          await sleep(500);
        }
        if (now.panel !== target.panel) {
          if (now.panel === "team")
            await page
              .locator("#team-close, .mantine-Drawer-close")
              .first()
              .click()
              .catch(() => {});
          if (now.panel === "messages")
            await page
              .locator(
                ".workspace-panel-header button:last-child, .mantine-Drawer-close",
              )
              .first()
              .click()
              .catch(() => {});
          await sleep(400);
          if (target.panel !== "none")
            await button(target.panel === "team" ? "Team" : "Messages")
              .click()
              .catch(() => {});
          await sleep(500);
        }
        if (!phoneLike && now.terminals !== target.terminals) {
          await button(
            target.terminals === "open" ? "Show terminals" : "Hide terminals",
          )
            .click()
            .catch(() => {});
          await sleep(500);
        }
        if (now.draft !== target.draft) {
          await page
            .locator("#message")
            .fill(
              target.draft === "draft"
                ? "A draft long enough to wrap the composer onto a second line on narrow layouts"
                : "",
            );
          await sleep(300);
        }
      }
      return state();
    };
    const configs: Config[] = [];
    for (const sidebar of phoneLike ? ["-"] : ["open", "closed"])
      for (const panel of ["none", "team", "messages"])
        for (const terminals of phoneLike ? ["-"] : ["closed", "open"])
          for (const draft of ["empty", "draft"])
            configs.push({ sidebar, panel, terminals, draft });
    await page.locator(`[data-chat="${lead}"]`).click();
    await sleep(1500);
    let turn = 0;
    for (const config of configs) {
      const reached = await setLayout(config);
      const node = name(config);
      if (name(reached) !== node) {
        edges.push({
          node,
          action: "reach",
          byUser: true,
          error: `reached ${name(reached)}`,
          cls: 0,
          inputCls: 0,
          sources: [],
          moved: [],
        });
        continue;
      }
      if (profile.keyboard) {
        await page.locator("#message").focus();
        await page.evaluate(() => window.layoutKeyboard?.(true));
        await sleep(600);
      }
      const draft = await page.locator("#message").inputValue();
      const fill = async (text: string) => {
        await page.locator("#message").fill(text);
        await page.locator("#send").click();
        if (draft) await page.locator("#message").fill(draft);
      };
      await measure(
        node,
        "send turn",
        true,
        () => fill(`Probe turn ${++turn}`),
        1500,
      );
      await measure(node, "agent reasoning", false, async () => {
        const id = `r${turn}`;
        control([
          {
            notify: {
              method: "item/started",
              params: {
                item: { id, type: "reasoning", summary: [], content: [] },
              },
            },
          },
        ]);
        for (let i = 0; i < 4; i++) {
          control([
            {
              notify: {
                method: "item/reasoning/summaryTextDelta",
                params: { itemId: id, delta: ` step ${i}.`, summaryIndex: 0 },
              },
            },
          ]);
          await sleep(150);
        }
        control([
          {
            notify: {
              method: "item/completed",
              params: {
                item: { id, type: "reasoning", summary: ["done"], content: [] },
              },
            },
          },
        ]);
      });
      await measure(node, "agent command", false, async () => {
        const item = {
          id: `c${turn}`,
          type: "commandExecution",
          command: "pnpm test --filter web",
          cwd: "/tmp",
          status: "inProgress",
          commandActions: [],
          aggregatedOutput: "",
        };
        control([{ notify: { method: "item/started", params: { item } } }]);
        await sleep(400);
        control([
          {
            notify: {
              method: "item/completed",
              params: {
                item: {
                  ...item,
                  status: "completed",
                  exitCode: 0,
                  aggregatedOutput: "ok\n",
                  durationMs: 500,
                },
              },
            },
          },
        ]);
      });
      await measure(node, "agent plan and usage", false, async () => {
        const usage = {
          totalTokens: 9000 + turn * 100,
          inputTokens: 8000,
          cachedInputTokens: 0,
          outputTokens: 1000 + turn * 100,
          reasoningOutputTokens: 0,
        };
        control([
          {
            notify: {
              method: "turn/plan/updated",
              params: {
                explanation: null,
                plan: [
                  { step: "Read", status: "completed" },
                  { step: "Fix", status: "inProgress" },
                ],
              },
            },
          },
          {
            notify: {
              method: "thread/tokenUsage/updated",
              params: {
                tokenUsage: {
                  total: usage,
                  last: usage,
                  modelContextWindow: 200000,
                },
              },
            },
          },
        ]);
      });
      await measure(node, "agent stream", false, async () => {
        for (let i = 0; i < 10; i++) {
          control([
            {
              item: `s${turn}`,
              delta: `${i && i % 4 === 0 ? "\n\n" : " "}token${i} lorem ipsum dolor`,
              append: i > 0,
            },
          ]);
          await sleep(110);
        }
      });
      await measure(node, "queue message", true, () =>
        fill("Queued follow up"),
      );
      await measure(
        node,
        "agent complete",
        false,
        async () =>
          control([{ say: "Done. Short final answer." }, { complete: true }]),
        2000,
      );
      await measure(
        node,
        "queued reply",
        false,
        async () => control([{ say: "Follow up done." }, { complete: true }]),
        2000,
      );
      if (profile.keyboard) {
        await page.evaluate(() => window.layoutKeyboard?.(false));
        await sleep(400);
      }
    }
    const base: Config = {
      sidebar: phoneLike ? "-" : "open",
      panel: "none",
      terminals: phoneLike ? "-" : "closed",
      draft: "empty",
    };
    const toggles: [keyof Config, string][] = phoneLike
      ? [
          ["panel", "team"],
          ["panel", "messages"],
          ["draft", "draft"],
        ]
      : [
          ["sidebar", "closed"],
          ["panel", "team"],
          ["panel", "messages"],
          ["terminals", "open"],
          ["draft", "draft"],
        ];
    for (const [key, value] of toggles) {
      await setLayout(base);
      await measure(
        name(base),
        `set ${key}=${value}`,
        true,
        () => setLayout({ ...base, [key]: value }),
        1200,
      );
    }
    const shifts = await page.evaluate(() => window.layoutShifts);
    for (const edge of edges) {
      const own = shifts.filter(
        (entry) => entry.edge === `${edge.node}::${edge.action}`,
      );
      edge.cls = +own
        .filter((entry) => !entry.input)
        .reduce((sum, entry) => sum + entry.value, 0)
        .toFixed(4);
      edge.inputCls = +own
        .filter((entry) => entry.input)
        .reduce((sum, entry) => sum + entry.value, 0)
        .toFixed(4);
      edge.sources = own.flatMap((entry) => entry.sources).slice(0, 12);
    }
    return {
      profile,
      edges,
      load: shifts.filter((entry) => entry.edge === "load"),
    };
  } finally {
    await browser.close();
    fixture.kill("SIGTERM");
  }
}

type Result = Awaited<ReturnType<typeof runProfile>>;
const escape = (text: string) =>
  text.replace(
    /[&<>"]/g,
    (c) => `&${{ "&": "amp", "<": "lt", ">": "gt", '"': "quot" }[c]};`,
  );

/** One table per profile: rows are graph nodes, columns are edges, color is the worst signal. */
function report(results: Result[]) {
  const cell = (edge: Edge | undefined) => {
    if (!edge) return "<td></td>";
    const controls = edge.moved.filter(
      (m) => m.name !== "first-visible-message",
    );
    const level = edge.error
      ? "err"
      : edge.cls > 0.01 || (!edge.byUser && controls.length)
        ? "bad"
        : edge.cls > 0.001 || controls.length
          ? "warn"
          : "ok";
    const detail = [
      `CLS ${edge.cls} (input ${edge.inputCls})`,
      ...edge.moved.map((m) => `moved ${m.name} [${m.delta.join(", ")}]`),
      ...edge.sources.map(
        (s) =>
          `${s.input ? "input " : ""}${s.node} dx=${s.dx} dy=${s.dy} dw=${s.dw} dh=${s.dh}`,
      ),
      edge.error ?? "",
    ].filter(Boolean);
    return `<td class="${level}" title="${escape(detail.join("\n"))}">${edge.cls ? edge.cls.toFixed(3) : controls.length ? controls.length + "↔" : "·"}</td>`;
  };
  const sections = results.map(({ profile, edges }) => {
    const actions = [...new Set(edges.map((e) => e.action))].filter(
      (a) => a !== "reach",
    );
    const nodes = [...new Set(edges.map((e) => e.node))];
    const rows = nodes
      .map((node) => {
        const own = edges.filter((e) => e.node === node);
        const unreachable = own.find((e) => e.action === "reach");
        return `<tr><th>${escape(node)}</th>${unreachable ? `<td colspan="${actions.length}" class="err">${escape(unreachable.error ?? "")}</td>` : actions.map((a) => cell(own.find((e) => e.action === a))).join("")}</tr>`;
      })
      .join("");
    return `<h2>${profile.name} <small>${profile.width}×${profile.height}${profile.touch ? " touch" : ""}${profile.keyboard ? ` keyboard ${profile.keyboard}px` : ""}</small></h2><div class="scroll"><table><tr><th>node</th>${actions.map((a) => `<th>${escape(a)}</th>`).join("")}</tr>${rows}</table></div>`;
  });
  return `<!doctype html><meta charset="utf-8"><title>Layout shift matrix</title><style>
:root{color-scheme:light dark;--ok:#1d8a3f33;--warn:#c9730044;--bad:#ee000055;--err:#7d7d7d44}
body{font:13px/1.4 system-ui,sans-serif;margin:16px;background:Canvas;color:CanvasText}
.scroll{overflow-x:auto}table{border-collapse:collapse}th,td{border:1px solid #8884;padding:4px 6px;white-space:nowrap}
th{font-weight:500;text-align:left}td{text-align:center;font-variant-numeric:tabular-nums}
.ok{background:var(--ok)}.warn{background:var(--warn)}.bad{background:var(--bad)}.err{background:var(--err)}
</style><h1>Layout shift matrix</h1><p>Rows are UI configurations (graph nodes); columns are edges. A cell shows the layout-shift score without recent input, or the count of moved controls. Hover a cell for sources. Red: shift above 0.01, or a control moved by an agent event.</p>${sections.join("")}`;
}

const { values } = parseArgs({
  options: {
    out: { type: "string" },
    profile: { type: "string" },
    chromium: { type: "string" },
  },
});
const out = resolve(
  values.out || join(process.env.TMPDIR || tmpdir(), "layout-shift-matrix"),
);
mkdirSync(out, { recursive: true });
const selected = values.profile
  ? profiles.filter((p) => values.profile!.split(",").includes(p.name))
  : profiles;
const results: Result[] = [];
for (const profile of selected) {
  const result = await runProfile(profile, values.chromium);
  writeFileSync(
    join(out, `${profile.name}.json`),
    JSON.stringify(result, null, 1),
  );
  results.push(result);
  const shifting = result.edges.filter(
    (e) =>
      e.cls > 0.001 ||
      (!e.byUser && e.moved.some((m) => m.name !== "first-visible-message")),
  );
  console.log(
    `${profile.name}: ${result.edges.length} edges, ${shifting.length} shifting`,
  );
  for (const e of shifting)
    console.log(
      `  ${e.node} | ${e.action} cls=${e.cls} ${e.moved.map((m) => m.name).join(", ")}`,
    );
}
writeFileSync(join(out, "index.html"), report(results));
console.log(`report: ${join(out, "index.html")}`);
