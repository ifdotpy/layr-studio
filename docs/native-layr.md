# Native Layr workspaces

The `layr-native` workspace mode runs agents on the host.
The chat selector calls this mode **Layr**.
**WT** uses Git worktrees.

## Store and installation

Studio defaults to the host store at `<Studio state>/layr-native/store`. The built-in default resolves at use time.
An operator can select an existing host store with `native-layr configure --store-root`.
Studio saves this setting in `<Studio state>/layr-native/settings.json`.
The general Studio settings can select the same default. Project settings can select a store override, which takes precedence over the global default and built-in store. Each prepared chat records its store in `layrStoreRoot`.
A root switch changes the destination for new independent chats. Existing chats and their descendants retain their store and line.
The agent's `PATH` selects the managed Git guard, which refuses Git writes in native lines.
`NATIVE_LAYR_DIRECTORY` in `runtime-store/src/path.rs` owns the native metadata directory.
`STORE_DIRECTORY` in `layr_native.rs` owns the store subdirectory.
The state directory must stay outside project folders.
New imports use the SHA-256 of the canonical source folder path as the project ID.
Studio reuses an existing project when its Git origin and commit history match the source folder.
An explicit mapping selects a project when more than one project matches.
Studio refuses an origin or history mismatch.
Readiness records and operation receipts are separate for each store. The default store retains its original receipt paths.

Install Layr separately and make `layr` available on the server's `PATH`.
For a systemd service, install it in a directory on the unit's `Environment` PATH or add its installation directory to that PATH, which may omit user-local binary directories.
Studio uses that executable and does not require a particular Layr version for now.
Use `studio-runtime-server native-layr paths --state /path/to/studio-state` to inspect the store and daemon paths.

The host must use APFS on macOS or btrfs on Linux.
The new chat selector reads the tagged `nativeLayr` capability, default workspace mode,
and unavailable message from `/api/ui-summary`. The server defaults to WT unless the
native capability is ready.
The probe requires a btrfs store and a listening root daemon at `<Studio state>/layr-native/layr.sock`.
On macOS, commands run as the Studio user with `LAYR_ROOT` set to the owned store.
`LAYR_DIRECT=1` selects the macOS backend without another service.
On Linux, Layr requires a root daemon for btrfs operations.
Studio sets `LAYR_DIRECT=0` and `LAYR_SOCKET=<Studio state>/layr-native/layr.sock` for Linux commands and agent shells.
The daemon checks the Studio user's identity for each command.
It uses the same Studio-owned store.
Studio does not modify mounts or install the root service.

Install the host binary first.
On Linux, start its daemon with the paths from `native-layr paths`:

```sh
sudo env LAYR_ROOT=/path/to/studio-state/layr-native/store \
  LAYR_SOCKET=/path/to/studio-state/layr-native/layr.sock \
  /path/to/installed/layr daemon
```

On Linux, install Bubblewrap for reviewer checks. If you use a systemd unit for the daemon,
set its `ExecStart` to the installed Layr binary. Restart the daemon after every Layr upgrade
so it runs the updated binary.

The daemon must run before a native chat or migration on Linux.
macOS does not require this step.

## Select a shared store

Use an existing store outside the source project folder.
The commands below change only Studio settings. They do not move or delete either store. A CLI change takes effect for new chats immediately; open clients display it after their stream reconnects or the server restarts.

```sh
studio-runtime-server native-layr configure --state /path/to/studio-state \
  --store-root /Users/igor/Projects/layr-store
studio-runtime-server native-layr map --state /path/to/studio-state \
  --project studio /Users/igor/Projects/codex-agents
studio-runtime-server native-layr paths --state /path/to/studio-state
```

The selected directory must exist, use a supported copy-on-write volume, and have a compatible store and service. The server checks the directory against registered project folders; the CLI does not open the Studio database and can only check a source folder supplied to it. On Linux, start a separate root daemon for each selected root at its corresponding socket. A refusal reports the store, socket, and binary paths. Studio does not start or stop the service.

The mapping verifies the Git origin and at least one common ancestor commit.
GitHub origins match when the GitHub repository IDs match, including a repository rename.
Studio reads these IDs with `gh api`. The host needs `gh`, network access, and GitHub authentication.
An unsuccessful lookup refuses the mapping.
Other origins must have equal normalized URLs.
The mapping record retains both origin URLs and the GitHub repository ID.
The source and project can have different folder names.
An existing project supplies its saved main line and caches. Studio does not import another copy.
New implementer lines receive the caches from that main line.
The original source folder stays separate from main.

Studio uses saved caches on main's named head.
If that head has no cache record, Studio checks up to 200 recent history records for main's newest automatic state with caches.
Studio uses that state only when `layr diff --name-only main STATE` returns no paths.
It never selects a cache state from another line or changes the tracked content of main.
Otherwise, the prepared chat records `layrCacheWarning` with the main state ID and an operator command.
For a one-time cache setup, build in the existing store's main folder.
Keep tracked content unchanged, including the dependency lockfile.
Then run `layr -C /path/to/store/projects/studio/lines/main save --caches` on the protected main line.
Layr attaches these caches to the unchanged main head.
If tracked content changes, Layr saves caches on a separate automatic state instead.
Those caches do not automatically belong to named main.

Studio requires a compatible Layr store format before it prepares a line.
`layr store-version --json` reports the store format, package version, and build identity.
An incompatible store returns an error that names the root and both versions.
Layr 0.36 reports an unmarked store as `unmarked`, with no store identity and
`compatible: true`. The next normal Layr command that opens the store writes its
marker. Studio's readiness check only inspects the store; it does not write the marker.
Studio refuses incompatible stores. Use a compatible Layr binary for the store,
or select a compatible store with `native-layr configure --store-root`.

To select the original default for new chats, run:

```sh
studio-runtime-server native-layr configure --state /path/to/studio-state --default-store
```

An earlier import in the default store retains its project ID, receipts, lines, and data.
Legacy default-store projects reuse their ready import receipt, which records the original source path.
New external or named project mappings always verify the origin and shared Git history.
Chats created before a switch retain their recorded root, including records created before this setting existed.
Their descendants use that same root. A new independent chat uses the currently selected root.
The prepared state includes `layrStoreRoot` and `layrProjectId` for that chat.
Unmerged work in the old root stays there until you merge or move it.
Each store must pass the version check when Studio prepares its next native line.
On Linux, the configured daemon must serve the selected root and compatible format.
Each selected root has its own daemon socket under the Studio native metadata folder.
Use the socket from `native-layr paths` when you start that root's daemon.

## First import

The native import omits Git-ignored data, including ignored `.env` files.

Studio selects all paths and rejects case or Unicode collisions before it writes imported entries.
It sorts path references and checks sibling names once per directory.
The validator retains tables only for the current directory and its parents.
It makes a staging tree outside the checkout with APFS clones on macOS or btrfs reflinks on Linux.
It runs `layr init <project> --from <folder>`.
It then permits the owner to save directly in protected main.
No tar archive is created.

Build output becomes a warm Layr cache outside history.
The shared build-folder owner is `vm_preparation::regenerable_path`.
It covers `node_modules`, `.venv`, `venv`, `__pycache__`, `.pytest_cache`, `.mypy_cache`, `.ruff_cache`, `.turbo`, and `.next`.
A `target` folder becomes a cache only beside `Cargo.toml`.
Studio omits cache content from the initial state, registers each cache, clones its files, and saves the warm caches.
New lines receive these caches without a rebuild.

Layr imports the source Git history and creates its own root `.git` bridge. A clean
source tree has no additional import commit. Submodules are not supported in
native Layr import yet; Studio refuses them before writing imported entries.
Nested `.git` directories enter history. An invalid Git marker remains plain
versioned data, with its bytes preserved. The `plainGitMetadata` report lists
each path and the reason. Studio does not read its repository status.
Git pointers, common directories, and object alternates inside the whole project are admitted.
This boundary also applies when one imported worktree refers to another repository in the project.
Stores outside the project cause refusal. The report lists their paths.
An operator can select `native-layr migrate --without-git <project>` to import that project as a plain folder.
This option omits only the top-level `.git`, without a backup. Nested repositories retain normal admission.
The report and import receipt record this choice. The source folder remains unchanged.
The option cannot change a completed import that retained Git metadata.

Studio excludes registered linked worktree folders from main.
Ordinary files and folders under `.worktrees` remain versioned data.
The `worktreeData` report lists them.
The migration command imports their files into separate lines.
It omits their Git-ignored data and retains warm build caches.
The `cachePathChanges` report lists paths whose role differs between main and an imported line.
On APFS, a line can keep versioned files at a path that is a cache in main.
Main retains its cache. The imported line reports `not a cache: has versioned files`.
On btrfs, this case still causes refusal because the inherited cache is a subvolume.
It needs a future command that makes a cache folder plain. Native Linux has structural checks only on this Mac.
Use `--skip-worktree <path>` to omit an operator-selected temporary worktree. The report records the skipped path.
The original worktree folder remains unchanged.

The free-space preflight counts metadata, cross-volume copies, Git fetch output, and Git import allocations.
Reports separate these costs as `metadataBytes`, `transientMetadataBytes`, `crossVolumeBytes`, `gitTransferBytes`, and `gitHeadBytes`.
The metadata allowance is 2,862 bytes per entry, including directories.
It is twice the larger measured peak per entry, rounded up to a byte.
The APFS cache fixture has 4,096 files and 4,161 entries.
Its net free-space delta is 3,297,280 bytes. Its peak is 4,079,616 bytes, or 980.44 bytes per entry.
Every source and main cache clone ID matches.
The current migration fixture has 4,096 versioned files, 4,096 cache files, and 8,517 source entries.
A new line costs 3,092,480 bytes net and 3,960,832 bytes at peak.
A migrated line costs 11,362,304 bytes net and 12,185,600 bytes at peak.
These are 363.09 and 1,334.07 net bytes per entry, respectively.
The migrated-line peak is 1,430.74 bytes per entry. The allowance is twice this value, rounded up.
The temporary line allowance is 931 bytes per main entry, twice its measured peak per entry.
Layr 0.2.2 uses the APFS whole-directory clone call when hard links and nested layers do not require a fallback.
The measured fixture has neither condition. The directory clone still needs metadata for its new tree.
The measured bytes per entry do not show a file-by-file fallback.
The estimate charges only the part of that temporary allowance above the final line's metadata allowance.
For a nested repository, each line also retains the other project folders from main.
Their entries receive the same metadata allowance and appear as `inheritedEntries` and `inheritedMetadataBytes`.
The report separates the actual main counts as `mainSize` and the original source counts as `sourceSize`.
If native main exists, its saved main state determines each line clone cost.
APFS thin layers are counted conservatively, including their base entries, without creating a complete tree.
The estimate also includes the largest local saved cache tree for each cache path.
It counts this tree even when the main working cache is empty or absent.
Same-volume clones add no payload bytes to this estimate.
The estimate conservatively allows for every HEAD blob and copies into overlapping source files.
A margin retains at least 64 MiB or ten percent of the estimate, whichever is larger.
A same-volume clone failure causes refusal, without a byte-copy fallback.
Cross-volume files require a byte copy and sufficient space before import.
Keep the source quiet during import. Studio checks its identities before and after the clones.
Source Git reads set `GIT_NO_LAZY_FETCH=1` and `GIT_OPTIONAL_LOCKS=0`.
Missing partial-clone objects cause an unknown-size refusal, without fetching into the source.
Clears use directory descriptors and refuse symlink ancestors. They unlink links without visiting their targets.
Submodule config bytes are read through a validated descriptor and edited in a private temporary file.
Studio writes the result through the destination directory descriptor. A replacement link cannot redirect the edit.
Attribute copies use descriptors or no-follow link calls on APFS and btrfs.
Full-tree snapshots preserve empty directories, directory modes, and file mode changes in each imported head.
The source repository root mode also applies to each staged repository root.

Known Layr limits, last checked on 0.5.1:

- `caller_git` calls `command_as`, which clears `GIT_NO_LAZY_FETCH` and `GIT_OPTIONAL_LOCKS`.
  Studio refuses source object links and alternates that could reconnect a staged repository to the original object store.
- Layr `copy_entry` does not preserve symlink attributes or Linux file attributes.
  Studio preserves them in staging, but the final Layr import can lose them.
- The APFS thin snapshot comparison ignores attributes and change time (`ctime`).
  A directory attribute change alone can be absent from the saved head.
  A successful metadata fixture does not establish support for this case.

Layr includes Studio's earlier fixes for private directory metadata and warm cache copies.
It keeps private directories writable until metadata updates finish.
The final state and line retain source directory and file modes, including `0555` and `0444`.
The source modes stay unchanged.
Metadata errors name the affected path and operation.

An explicitly failed first import can retry with the same request ID.
On APFS, the retry removes only that import's private partial project before it imports the current source again.
The removal uses directory descriptors and never follows source links.
A ready project cannot be removed by this recovery.
Pending and unknown receipts stay held for inspection.
On btrfs, automatic removal of a partial project is unavailable.
The error names the private project that needs daemon-owned subvolume removal.
For a pending import on btrfs, first confirm the daemon has finished using the retained staging folder; do not retry or delete that folder while the outcome is unknown. Inspect the private project and import receipt, then have an operator either complete the remaining preparation and record the verified result, or remove the partial project with daemon-owned subvolume removal before an explicit retry. Preserve the staging folder until that inspection is complete.

Capacity checks run before the pending receipt. A shortage can retry with the same request after capacity increases.

Later chats use the existing main line.
They do not copy changes from the original folder.

The native main line is the source of truth for these chats.
The original Mac folder remains separate.
To export a normal folder or send reviewed work to a Git remote, run from main:

```sh
layr export /path/to/new-copy
layr git push origin main
```

A push still requires the user's authorization.

## Agents and tasks

The managed Git guard permits writes targeting repositories or explicit config files outside native Layr lines.

The lead uses `projects/<project>/lines/main` in the host store.
An implementer uses `agent-<SHA-256 of agent ID>`.
Studio creates this line from the requested `base_ref` revision, or from main when none is specified.
Workspace readiness and cleanup use the existing runtime hooks.
The agent record has `workspaceMode: layr-native`, `executionMode: native`, and `layrReady: true` after preparation.
`layrProjectId`, `layrLine`, `layrStateId`, and `cwd` identify the association.

Use a named state for task submission:

```sh
layr add path/to/file
layr save -m 'Complete the task'
layr rev-parse head
```

Submit the full state ID.
Studio rejects a prefix or a state that is not the worker's named head.
Acceptance runs `layr merge <line> --expect <state>` in main.
A changed worker head prevents acceptance.
Lead and reviewer results do not cause a merge.

At turn end, Studio runs `layr overlay repair <overlay>` for each folder overlay, then `layr save`.
An unchanged line without automatic states resolves to its current named head.
This automatic state preserves unfinished work separately from the named head.
Archive saves remaining work and creates `pins/studio/archive/<SHA-256 of agent ID>` at that exact state.
It then calls `layr line delete -D <line>`.
The archive receipt records `archiveRef` and `stateId`.
The pin has no expiry and keeps unmerged automatic states through garbage collection.
Studio never removes archive pins automatically.
After review or explicit discard, an operator can remove a pin with `layr ref delete <archiveRef>`.
Later garbage collection can then apply the normal retention rules.
Archive of a lead retains main.

Each filesystem effect has a durable receipt below `layr-native/receipts`.
A repeated request returns its saved result.
A request with an unknown outcome stays held for inspection.
Studio does not repeat an uncertain merge, import, save, or deletion.
An explicitly failed quiet-point save can retry with the same request ID.

## Warm caches

Mark cache folders in `.layrattributes` and ignore them in Git:

```text
node_modules regenerable
```

Build in main, then run:

```sh
layr save --caches
```

A new worker line receives the saved cache folders from main.
Existing worker lines keep their own folders.
Layr decides whether a folder contains versioned files and can become a cache.
Studio does not choose build commands or automatically publish caches.

## Git tools in native lines

The managed `git` executable uses Layr for compatible read requests in native lines.
The `read_route` function in `layr_native/git-guard.sh` owns the command table.
It converts Git `HEAD` revisions to Layr `head` revisions before the path separator.
Arguments after `--` keep their original bytes.
The wrapper preserves the command's output and exit status. It does not retry failed reads.

Layr 0.2.1 returns state IDs for commit identities and supports porcelain v1 only.
The proxy uses Layr only for a small set of verified argument forms.
Other reads use the real Git executable against the Layr-managed `.git` directory.
This fallback also serves read commands that Layr does not implement, including `archive`, `describe` and `cat-file`.
Git commit IDs therefore match the exported head, including `git rev-parse HEAD` and build stamps.
The fallback disables optional Git index refreshes.
Git owns path quoting for porcelain and `ls-files` without `-z`, all grep forms, and unverified history formats.
The only proxied history format is `log -1 --format=%ct`.
Flagged revision queries use Git unless the complete form has a verified Layr equivalent.

The managed Git index describes the exported head. It does not follow `layr add`.
The proxy refuses Git `diff --cached`, `diff --staged`, and porcelain v2 instead of reporting that index as Layr's stage.
Use `layr diff --cached` and `layr status --porcelain` for these reads.
Other real Git index reads describe the exported head, including `ls-files` without `-z`.

Git writes remain refused with a Layr command hint.
Layr also keeps the `.git` folder of a line read only (mode 0555), so a real Git write fails with `Permission denied`.
Layr opens the folder around its own writes. Studio does the same when it installs the commit hook and when an import moves `.git` aside, and it restores the mode.
Read commands with `--output` also fail because that option writes a file; `archive -o` is refused for the same reason.
Outside native lines, the wrapper passes the original arguments to real Git.
Explicit Git configuration and repository selectors use the real Git read path.

Tools that use libgit2 or gix read `.git` directly and do not call this wrapper.
They require the Layr-managed `.git` directory and its Git object store.

The guard prevents agent mistakes. It is not a security boundary.
It resolves a relative `GIT_DIR` environment value before any later `-C` option.
Git can resolve that value against a different final directory.
The guard does not check every `GIT_*` destination, including `GIT_INDEX_FILE`.
Git can run configured helpers, including `diff.external` and filesystem monitors.
These helpers can write files or run other commands.

## Search tools in native lines

When the installed Layr serves tree tools (`layr tool` without a tool prints its usage), the
managed bin folder also holds links named `rg` and `find` to the Layr binary. Agents' `rg` and
`find` calls in a native line are then answered from Layr's in-memory index of the line, with
the real tools' output; anything Layr cannot answer the same way runs the real tool (Layr
`docs/layrfs.md`). With an older Layr the links are removed. Preparing a line starts its index
build (`layr fs warm`), so the first searches of an agent are already served.

Set `LAYR_FS_OFF=1` in the server's environment to remove the links and always use the real
tools. `layr fs status` lists the indexed lines and the memory they use.

The `tool_index` module owns this behavior.

## Virtual agent lines (experimental)

A virtual line is a LayrFS mount of the line's changes over the tree of its start state (Layr
`docs/views.md`). A new virtual line costs one mount, not a clone of the whole tree. Layr offers
it behind `LAYR_EXPERIMENTAL_VIEWS=1`, on macOS with the LayrFS file system extension.

The setting "Virtual agent lines (experimental)" in the Layr settings is off by default. Studio
offers it only when the server can make virtual lines: macOS, an existing store, and a Layr
build in which `layr line show` without a line prints a usage that names `--virtual`. The
setting is `virtualAgentLines` in `layr-native/settings.json` and in
`/api/native-layr/settings`; a request with `virtualAgentLines` and without `storeRoot` changes
only this setting.

When the setting is on, preparation makes each new worker line with
`layr line new <line> <start> --view virtual`. Studio sets `LAYR_EXPERIMENTAL_VIEWS=1` only for
the commands that need it (`line new --view virtual` and `line show`). Leads and main stay
folders. Assigned reviewers keep folder lines, because they read through `layr try` and Git,
which a virtual line refuses. A line keeps its view when the setting changes later. If Layr
refuses the virtual line (for example, the extension is off), the server log gets a warning and
the agent gets a folder line.

Layr refuses most commands inside a virtual line. Studio uses only the commands that Layr
supports there, and the `virtual_lines` module owns these choices:

- **Is the line virtual?** Layr's local view record
  `projects/<project>/meta/vlines/*/view.json` names the line. It stays after a restart.
- **Preparation.** `line new --view virtual`, then `line show <line> --virtual` from main: it
  mounts the line again after a restart and does nothing for a mounted line. Then `fs warm`, as
  for a folder line. No Git hook: the line has no `.git` folder.
- **Turn save.** `rev-parse head` only, without a receipt. A folder line repairs its cache
  overlays and runs `save`, but a virtual line refuses `layr overlay` and `layr save` without
  `-m`. Its changes stay in its upper folder, which survives a restart.
- **Evidence.** `rev-parse`, as for a folder line.
- **Merge into main.** `merge <line> --expect <state>` from main, as for a folder line. The line
  has no caches, so Layr merges none.
- **Removal.** `status --json`; with changes, `save -m "Studio: unsaved work of the removed
  agent"`; archive `head` (a folder line archives `head@{0}`); then `line delete -D` from main.
  The commands in the line run with `LAYR_TRASH_BUDGET_MS=0`, so Layr starts no trash process
  whose working folder is the mount.

Agents in a virtual line work as in a folder line, with these limits from Layr:

- `layr status`, `layr diff` and `layr save -m` work. `layr add` is refused, and not necessary:
  a save takes every change.
- `layr git push`, `restore`, `reset`, `undo`, `overlay` and the other commands are refused. The
  lead merges the agent's saved state into main as before.
- There is no `.git` folder, so Git reads fail. Use `layr log`, `layr show` and `layr diff`.
- Build caches (`target/`, `node_modules`) are written into the upper folder and are not shared
  with other lines.
- `rg` and `find` go through the tool links (previous section): Layr answers them from the
  lower tree and the upper folder directly, not through the mount (ifdotpy/layr#432). Other bulk reads
  through the mount are slow.
- `line delete` must unmount the line. A process whose working folder is in the line (an agent
  shell, a server, or the trash process of a `layr save -m` that the agent ran) makes the
  removal fail with "Resource busy", and the cleanup of the agent is held. Stop that process,
  then delete the line with `layr line delete -D <line>` in main.

## Reviewers

A native reviewer uses its private scratch folder as its working directory.
Its `layrReadCwd` points to main for trusted state reads.
When spawning a reviewer, the lead can provide `review_state` as the full state ID
to review. Studio validates it in the project and includes it in the reviewer's
first input, with examples for reading that exact state through `layr_read`.
A reviewer can submit its task without a revision; implementer results still
require their exact state ID.
It owns no Layr line.
Codex permits writes only in the private reviewer scratch folder.
Claude uses the same scratch working directory and a sandbox that denies writes to the store.
The `layr_read` tool reads states outside that sandbox through a command allowlist.
It permits `show`, `diff`, `log`, and the existing history read commands.
It does not permit changes to main or worker lines.
Reviewer turn completion does not save or repair main.

The host user owns the entire store.
Native mode does not create separate Unix users.
The native `layr_read` tool also accepts `try <state> -- <command>`.
Studio runs the command in a temporary Layr copy.
On macOS, `sandbox-exec` permits writes only in that copy and the private scratch folder.
On Linux, Bubblewrap starts with an empty root and must be installed.
It mounts only read-only toolchains, the temporary copy, and private scratch.
The Studio state directory and daemon socket stay hidden.
The check has no network access.
The check cannot change main or the worker lines.

Reviewer checks receive an explicit environment allowlist.
It contains `PATH`, `LANG`, `TERM`, and available Rust toolchain selectors.
For try commands, `HOME` and `TMPDIR` point to scratch.
Claude reviewer sessions also retain Studio's provider account selectors, including `CLAUDE_CONFIG_DIR` when configured.
Claude keeps its provider `HOME` for authentication. Try commands keep scratch `HOME`.
Lead and implementer environments keep their existing rules.

Linux reviewer checks cannot use Git alternates that point into the hidden store.
For these states, `git log` in a try copy can fail. Use `layr_read` history commands instead.
The sandbox does not expose the store to repair these Git paths.
Linux still mounts broad `/usr` and `/opt` toolchain directories read-only.
Native reviewers can also access the browser server through `node_repl`, as current WT reviewers can.
These limits remain outside the current reviewer isolation changes.

## Migration

The migration keeps the original project and its Git worktrees.
Each worktree becomes a native line with a stable name.
The line replaces only its repository subtree.
The import includes uncommitted files and omits Git-ignored data.
It excludes the worktree's `.git` pointer. Build output becomes warm caches.
Prunable worktrees are reported and skipped.
Completed receipts make a repeated migration idempotent.
The migration writes typed copy progress as JSON lines on standard error; its
normal report stays on standard output. Line creation and cache preparation
are indeterminate because Layr does not expose counts for those operations.

For a project with running WT chats:

1. Wait for the current turns to finish.
2. Pause those chats in Studio before the import.
3. Run the dry run below and inspect each worktree-to-line entry.
4. Run the migration while the original project and worktrees are quiet.
5. Create a new **Layr** chat for the same original project folder.
6. Review each imported line before you run its printed merge command.

Each selected Git worktree becomes a separate Layr line.
Existing WT chats keep their Git working directories.
The script does not pause chats or change their workspace mode.
Keep those chats paused after the import.
The new chat uses native main.
Completed receipts do not copy later edits from the old Git paths.
Parent Git metadata comes from the first main import. Later refs and objects remain in the original Git store.
Inspect the unpushed commit report before you retire that store.
The report prints the command list for each project.
After import, its merge commands pin each imported state with `--expect`.

The dry run starts no backend and writes no project or store files.
For each project it reports files, logical bytes, tracked bytes, cache bytes, estimated new bytes, and lines to create.
The totals include main and the worktree sources.
The report lists `.worktrees` containers, their versioned data, and any linked worktree without an admitted parent.
If a worktree cannot be sized, `sizesComplete` is false and its refusal includes its source path.
It lists nested repositories, their type, branch, uncommitted changes, untracked files, and unpushed commits.
Unpushed commits are local branch or detached-HEAD commits absent from all remote-tracking refs.
The command does not fetch remote refs.
Each repository report explains its Git metadata and worktree treatment.
Each worktree also has a `classification` and HEAD-specific commit counts.
Class `a` has uncommitted or untracked files.
Class `b` has HEAD commits absent from remote-tracking refs, local branches, or the parent repository HEAD.
Class `c` is clean and has none of these commits.
The `worktreeSummary` counts these classes. All worktrees remain selected by default.
Git status does not count ignored data. Such data can differ even in a clean worktree.
Studio also compares flagged index files by bytes to detect edits hidden by assume-unchanged or skip-worktree.
Use `--worktrees all` to keep the default selection.
Use `--worktrees dirty` to select uncommitted files, untracked files, and HEAD commits outside all local branches and remote-tracking refs.
A clean worktree on a named local branch can be skipped even when its commits are not pushed.
Each skipped entry records its HEAD commit and the reason.
Its Git store remains in main. Its ignored files remain in the original worktree.
If main already exists, Studio verifies that it retains each skipped HEAD commit.
The `selectionEstimates` report shows the costs and line counts for both modes.
Use explicit `--skip-worktree` paths for other operator exclusions.
Refusals include external Git object paths and prevent a real import.

### Switch a WT chat to Layr in place

An idle WT chat can switch to Layr and keep its transcript and provider thread.
Use **Switch to Layr** in the chat menu, or call the API:

```sh
port=<Studio server port>
token=$(curl -s "http://127.0.0.1:$port/api/session" | jq -r .token)
curl -s -X POST "http://127.0.0.1:$port/api/agents/workspace-mode" \
  -H 'content-type: application/json' -H "x-canvas-token: $token" \
  -d '{"id":"<chat ID>","workspaceMode":"layr-native"}'
```

Switch the lead first. Its workspace becomes the project's Layr main line, with the same project identity rules as a new Layr chat.
Then switch each implementer. Studio creates `agent-<SHA-256 of agent ID>` from main and imports the worker's Git worktree into it, as the migration does.
The import omits Git-ignored data and retains warm caches.
An implementer can switch only when its parent is a ready Layr chat.
Its worktree must have no uncommitted or untracked files, and its HEAD must be on a remote-tracking branch.
Studio does not fetch. Push the branch first.
A running turn, monitor, task, queued input, or workspace operation refuses the switch.
Switching requires an idle chat, so its copy progress is not displayed in the
client. The existing start progress carrier applies only to queued or starting
agents.
New workers of a switched lead use Layr.
Existing WT workers keep WT until you switch them.
A switched lead cannot accept Git results from a WT worker. Switch each implementer after the lead.

The switch writes the line under the receipt that preparation uses, so a repeat returns the same line.
A repeat after success returns the saved association. An unknown outcome stays held for inspection.
The Git worktree and branch stay unchanged. Remove them after review.

The next turn resumes the same provider thread in the new folder.
Claude keys its native session file by the working folder below `CLAUDE_CONFIG_DIR/projects`.
On that resume, the Claude bridge copies the current session file to the new folder's key, as a move import does, and resumes the same session.
The old file stays. Usage scans count each response once.
Codex stores threads by date, not by folder. Studio sends the new `cwd` with the thread resume and with each turn.
The bridge copies only the current native session file. Files of earlier native sessions of the chat stay in the old folder.

Inspect the migration first:

```sh
python3 workspaces/runtime/apps/native-layr/codex_layr_native_migration.py \
  --state /path/to/studio-state --dry-run /path/to/project
```

Then omit `--dry-run` to import the project.
The wrapper uses `studio-runtime-server native-layr migrate`.
Use `--help` for the operator steps without starting the migration.
It starts no backend and opens no Studio database.
`STUDIO_RUNTIME_SERVER` can select the exact server binary.

## Project workspace policy

A project can allow only native Layr workspaces. The server stores the policy on the
project record as `workspacePolicy`. Absent means no policy; `"layr-native"` is the
only other value.

Set or clear it with `POST /api/projects`:

```sh
curl -s -X POST http://127.0.0.1:4621/api/projects \
  -H "content-type: application/json" \
  -H "x-canvas-token: $LOCAL_API_TOKEN" \
  -H "X-Studio-API-Schema: $API_SCHEMA_HASH" \
  -d '{"action":"set_workspace_policy","path":"/abs/project","expected_revision":0,"workspace_policy":"layr-native"}'
```

- `expected_revision` is the current `workspacePolicyRevision` (0 when never set).
  A stale value returns a conflict. A repeated save returns the saved record.
- `workspace_policy: null` clears the policy.
- `$API_SCHEMA_HASH` is `API_SCHEMA_HASH` in `workspaces/client/apps/web/src/generated/apiSchema.ts`.
- The write changes only the local server, which stores the project.

With `layr-native`, the server refuses a new chat or worker in the project with
mode `worktree`. A worker that inherits `worktree` from a WT lead is refused too.
Other modes stay allowed. Existing WT chats are not changed. The new chat settings
preselect Layr and disable WT for the project.

## Checks

Run the focused Rust tests:

```sh
cargo test -j 4 -p studio-runtime-services --lib layr_native
```

Run the real host lifecycle check with an installed Layr binary:

```sh
STUDIO_LAYR_NATIVE_TEST_BINARY=/path/to/installed/layr \
  cargo test -j 4 -p studio-runtime-services --lib \
  layr_native::receipt_tests::real_host_import_worker_merge_archive_and_warm_caches \
  -- --ignored --nocapture
```

The check uses a separate state directory and store.
It verifies import exclusions, warm caches, exact-state merge, deletion, and retained state reads.
It does not prove a live model turn or the HTTP path.
On Linux, run this isolated check as root on btrfs.

Check Layr integration with a real Git repository and a local bare remote:

```sh
STUDIO_LAYR_NATIVE_TEST_BINARY=/path/to/installed/layr \
  CARGO_INCREMENTAL=0 cargo test -j 4 -p studio-runtime-services --lib \
  layr_native::git_guard::tests::real_managed_layr_ \
  -- --ignored --test-threads=1
```

These four checks cover named saves, automatic saves, Git sync, and Git push.
They also verify that the managed Git entry still refuses agent writes.

Check the proxy with real Layr, Git, Cargo, and pnpm callers:

```sh
STUDIO_LAYR_NATIVE_TEST_BINARY=/path/to/installed/layr \
  CARGO_INCREMENTAL=0 cargo test -j 4 -p studio-runtime-services --lib \
  layr_native::git_guard::tests::real_managed_git_proxy_ -- --ignored
```

Measure the real preparation caller against a small existing-store fixture:

```sh
STUDIO_LAYR_NATIVE_TEST_BINARY=/path/to/installed/layr \
  CARGO_INCREMENTAL=0 cargo test -j 4 -p studio-runtime-services --lib \
  layr_native::warm_store_tests::real_native_preparation_reuses_an_existing_projects_saved_rust_cache \
  -- --ignored --nocapture
```

The fixture saves a Rust dependency cache and a `target/studio-test-results` witness in project `studio`.
The real lead and worker preparation callers reuse that project.
The check requires fresh dependency artifacts in the worker and prints both build times.
It uses private state and does not change the developer store.
