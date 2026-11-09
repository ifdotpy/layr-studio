# Populated pre-ledger release upgrade

These exact closed SQLite files were generated in a new disposable directory by
release `ff02c9d4ea4b40564c2008cb0cf6e8bd374ee804`, populated offline with one
agent and one work item, restarted with that release, then opened and stopped by
`f832d09308e4ac1ae1c46e7a6882077ec644acea`. Each run used a free port and shut
down with SIGTERM. No live state was read.

The extra ff02c9d restart stabilizes its catalog marker at the actual schema
version. f832d09 then takes its matching-catalog fast path and leaves the
`2026-11-07-explicit-review-model` individual checkpoint absent. This absence is
historical evidence and must remain absent when the new baseline opens/reopens.
The fixture contains `pre-ledger-root` with its explicit review model
`gpt-5.3-codex` and `pre-ledger-work` titled `Preserved upgrade task`.
