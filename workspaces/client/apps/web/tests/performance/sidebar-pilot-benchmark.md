# Sidebar pilot comparison (#994)

Prepare from the current integrated pilot line (Node 22.18+ and installed web
packages, including generated WASM bindings). From the repository root:

```sh
node workspaces/client/apps/web/tests/performance/sidebar-pilot-benchmark.ts /tmp/studio-sidebar-pilot
```

This builds both fixture bundles, verifies the commit probes and deterministic
fixture/update sequence, and writes `variants.json`. It starts no browser, server,
update phases or measurement. The baseline uses the exact Git source
`947e17dfb3209bc90afac82a555d82a036a7240f` through an esbuild source overlay;
it makes no checkout or worktree. Both variants share installed dependencies,
fonts and the generated WASM artifact. The manifest records original production
source hashes and reports bytes per emitted file (raw, gzip and Brotli), including
JavaScript, CSS and assets. These are fixture bundle sizes, not application dist
sizes. Instrumentation is the same commit hook in both variants.

Only the pilot owner runs the following after the lead grants and announces a
measurement window. Budget: at most 10 minutes, one headless Chromium browser,
one page/context at a time, 1,000 fixture chats with 30 compact-view rows, no CPU
throttling, no synthetic load. No ordinary checks or user work run alongside it.
The runner closes contexts, the browser and its local static server on completion
or error; its ten-minute deadline closes them if the run stalls.

```sh
node workspaces/client/apps/web/tests/performance/sidebar-pilot-benchmark.ts /tmp/studio-sidebar-pilot --run
```

The browser first validates that the actual Sidebar mounts exactly 30 actual row
components. The 970 old chats remain in the catalog but the existing compact
filter hides them. Updates rotate over the 30 recent chats four times, changing
each title and alternating working/read status each sweep without changing sort
order or entering the attention lane. Each of 120 updates awaits two animation
frames. The equal-replay phase applies the final identical 1,000-chat snapshot
120 times, again sequentially awaiting two frames. Title and status DOM checks run
after every sample. `results.json` reports per phase:

- Layout-effect commit counts for the actual Sidebar, row view and each observed
  row leaf, with zeroes included for unchanged leaves. The baseline predates the
  leaf split, so it reports its row view and status leaf. These count committed
  function executions, not render attempts or ancestor subtree commits.
- Update-to-double-rAF p95 in milliseconds (nearest-rank, 120 samples).
- Final retained JS heap bytes after Chromium's explicit full GC, and emitted
  bundle byte sizes. Each variant uses a fresh browser context.

The integrated adapter applies the actual generated `createSidebarMirror`; the
baseline adapter updates agent snapshots and runs its actual `chatIndicators`.
The fixture isolates rendering/read-model application; it does not measure SSE,
server computation or transport. The conversation counter is explicitly named
`conversationGuard`: it is a memoized sibling, **not the actual Conversation**.
It only witnesses sibling isolation and cannot establish application conversation
render counts. Browser/DOM and measured evidence remain unverified until the
owner executes `--run` in the granted window.

Typecheck without a full gate:

```sh
workspaces/client/apps/web/node_modules/.bin/tsc --noEmit -p workspaces/client/apps/web/tests/tsconfig.json
```
