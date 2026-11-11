# Prompt composer

`PromptComposer.tsx` owns the form subtree and reads the active chat's observable
draft in its observer leaf. `PromptInput.tsx` owns the native textarea, autocomplete, keyboard
handling, and caret-sensitive skill insertion. The draft hook in
["../../../../sync/drafts.ts"](../../sync/drafts.ts) owns draft persistence and
conflict handling. Send reads the current draft by chat ID so an immediate
Enter or a send delayed by an upload uses current text.

Textarea height is measured at most once per animation frame, including after
programmatic draft changes and width changes. CSS owns its minimum and maximum
height; long drafts scroll inside the textarea. Live text is subscribed to by
the transcript subtree so token deltas do not render the composer. Attachments,
dictation, recall, skill completion, caret placement, Enter, Tab, Shift+Enter,
and IME composition retain their input paths. Native edits publish the draft
without feeding it back through React's textarea value/defaultValue updates,
which mutate the backing text during composition. External draft changes write
the live value only when it differs and composition is inactive. During
composition an external draft is deferred until composition ends. A later
native edit drops that pending value and publishes the newer text, as ordinary
typing does. Only an explicit applied send reset takes precedence over later
composition input: `DeliveryOwner.draftResetVersions` owns its per-chat sequence.
The delivery owner still clears only a draft matching the captured send.
Conflict selection stays in `sync/drafts.ts`. Autocomplete never
changes focus or selection in a frame while composition is active.

Draft edits update the screen immediately, are written to the unsent input
buffer at once, and are sent by POST after the typing wait in
[`draftSyncTiming.ts`](../../sync/draftSyncTiming.ts). Stream snapshots and
changes restore saved drafts. An unconfirmed edit stays in the buffer until the
server confirms it, so a reload keeps it.

An incoming message displays as plain, wrapped text while its turn is active.
When the turn finishes, the completed message is parsed and shown as Markdown.

From `workspaces/client/apps/web/`, run:

```sh
pnpm run test:prompt-composer
pnpm run benchmark:prompt-composer
```

The input browser check exercises geometry, coalesced layout reads, input
handling, and autocomplete. The benchmark uses an isolated server and scripted
provider to create at least 300 completed turns containing Markdown and code.
It opens the actual SSE transcript, checks switching between two real chats,
and records input-to-second-animation-frame latency and render probes while
idle and while tokens stream at 4× CPU slowdown. It also checks that a token
without typing does not render the composer. It does not use a live model.

One local Chromium observation (301 completed turns) gave these timings:

| Phase                 | Before p50/p95  | After p50/p95   |
| --------------------- | --------------- | --------------- |
| Idle input            | 18.5 / 28.1 ms  | 21.7 / 29.9 ms  |
| Input while streaming | 29.9 / 158.9 ms | 28.6 / 140.3 ms |

These individual runs do not establish a latency improvement. In the same
observations, Conversation renders per streaming key fell from 0.68 to 0.36;
the token-only composer assertion changed from one render to zero. Phase-wide
render counts include asynchronous background work, so they do not by
themselves establish what caused each render.

## Effects and integration

The retained DOM effects in these two packages are:

- PromptInput's layout effect applies external textarea values without disturbing
  IME composition and schedules height measurement.
- PromptInput's effect observes textarea width and cancels pending resize frames.
- RequestCard's InlineAnswer effect measures and positions the answer using a
  ResizeObserver and animation frame.
- AnswerFields' effect installs the document keyboard input listener for numbered
  options while focus is inside the answer form.

SelectedConversationOwner creates RequestAreaOwner, starts its membership reaction,
and disposes it when the conversation closes or switches. RequestArea requires
that instance; Requests and answer cards read its observable list and views.
SignInCard reads the shared observable clock directly.

PromptDraft temporarily retains its marked useSyncExternalStore bridge until E6
makes the real draft reader observable. Its real-createDrafts witness covers
restored snapshots and programmatic draft changes through the production reader.
