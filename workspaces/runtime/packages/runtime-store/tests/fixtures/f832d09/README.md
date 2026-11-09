# Supported release fixture

These zstd frames contain the exact SQLite files created by release
`f832d09308e4ac1ae1c46e7a6882077ec644acea` on 2026-11-09. The release server
ran in a new temporary state directory on a dynamically selected free port
(43205) and shut down with SIGTERM. No live state was read.

The empty databases retain the release's actual catalog fingerprints, individual
completion checkpoints, receipt finish rows and schema versions. Canvas has
catalog schema version 405 and actual schema version 412 after ordinary startup;
a supported baseline must allow that difference. Analytics has version 47.

No current rows were synthesized or removed before compression. The tests add
current data after upgrading and verify reopen, receipt replay and scope guards.
Empty fixture counts do not establish whether historical production rows exist.
