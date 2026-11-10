# Consolidation P0-C harness and main baseline attempt

Status at 2026-11-10 17:08:06 UTC: **measurement deferred**, not a latency baseline.
The guarded smoke and main-copy runs both observed 1-minute load **11.06**
(5-minute 21.52, 15-minute 22.60), above the required threshold of 8. Both
returned before a workload, copying, live discovery or SQLite open. An earlier
smoke also deferred at load 18.9. The machine gate directory protocol was used;
ordinary gates completed before either attempt evaluated load.

The base application is GitHub main `646759b6be2d68bad05e8859822ab9368409c0ba`.
The overlay changes benchmark infrastructure and documentation only. The
attempt used the development-profile binary with SHA-256
`63d5174f1db7f9d1da34d01dcf387d9987c0486bb30a0293989a1a9964594ccb`.
The operator input is `~/.cache/codex-studio/tmp/livecopy-20261110/state`.
No source state or live server state was modified, and no backend was started.
Local attempt records are `/tmp/issue-edit/p0-c-main.json` and
`/tmp/issue-edit/p0-c-smoke.json`. Their `git_commit` identifies the saved harness
overlay, while their label identifies the main application base.

Validation passed:

- Workspace example build with `--exclude studio-native-client -j 4`.
- Existing storage benchmark guard: directory equality/overlap, unavailable
  discovery and database/WAL/SHM symlinks all refused; live fixture unchanged.
- Tooling TypeScript check, selected Oxlint/Oxfmt and `git diff --check`.
- `just test-fast`: passed, 509.744 seconds; local check run
  `~/.cache/codex-studio/checks/run-1794329423739-OVPljn`.
- `node workspaces/tooling/apps/repository-checks/clippy-ratchet.ts --exclude-native`:
  exit 0.

No successful hot-path runtime smoke or performance result is claimed. An initial
eligible baseline attempt failed before binary spawn because `/usr/bin/time` was
absent; the runner now uses the earlier F0 driver's Zsh `TIMEFMT` accounting.
Repeat the documented guarded smoke and main-copy command on an idle machine
before using this harness as numerical acceptance evidence for F2–F4. See
[the measurement contract and limits](../testing.md#consolidation-hot-path-gate-p0-c).

Review correction: copy admission now includes all DB/WAL/SHM sizes, target free
space and staging/VACUUM headroom. Hot mode executes exactly 100 frames (smoke
eight); exceeding its deadline fails rather than accepting a partial workload.
The seeded repair path stores typed delivery/assets metadata and checks that its
validation input is nonempty before timing. Three focused regression tests failed
before these fixes and passed afterward; logs are
`/tmp/issue-edit/p0-c-review-red.log` and `p0-c-review-green.log`. The previously
recorded binary hash above identifies the deferred attempt, not the corrected binary.
