# SQLite read-page fixture

Measurements on 2026-11-05, based on GitHub main Layr state `fbe74b7e6e60708fa3e901a1a8bdfb66`. An optimized release build opens an isolated temporary store, inserts 12,000 rows of 520 bytes, checkpoints, and runs ten sequential aggregate scans through `read_async`. Each variant had a warm-up run and three measured runs for each tool. Each entry is median (minimum–maximum). The baseline explicitly applies SQLite's default 2,000 KiB cache and zero mmap through the same connection setup as the variants; it is not a build without those pragma calls. The entire process, including initialization, migration, writes and reads, is measured. Wall times are separate uninstrumented runs. No live state was used.

| Policy                            |        `pread64` calls |           All syscalls |                                Callgrind Ir |        Wall seconds |
| --------------------------------- | ---------------------: | ---------------------: | ------------------------------------------: | ------------------: |
| Baseline: 2,000 KiB, mmap 0       | 20,065 (20,065–20,065) | 32,833 (32,702–34,244) | 1,914,861,484 (1,914,434,592–1,915,611,404) | 0.141 (0.139–0.147) |
| Cache only: 4,096 KiB, mmap 0     |    4,594 (4,594–4,594) | 18,000 (16,495–19,392) | 1,910,037,411 (1,909,891,342–1,911,274,097) | 0.154 (0.140–0.166) |
| mmap only: 2,000 KiB, 64 MiB mmap |    2,633 (2,633–2,633) | 15,419 (13,627–15,486) | 1,908,539,160 (1,908,398,127–1,909,605,909) | 0.138 (0.137–0.139) |
| Both: 4,096 KiB, 64 MiB mmap      |    2,633 (2,633–2,633) | 15,424 (15,046–15,757) | 1,908,758,059 (1,908,627,603–1,910,065,032) | 0.139 (0.137–0.153) |

Cache alone removes 77% of preads versus 87% for mmap; its wall times overlap the baseline and do not establish an improvement. Median instructions decrease 0.25% with cache alone. These are short runs, not a general latency estimate. To reproduce, build `cargo build --release -j 4 -p studio-runtime-store --example read_fixture`, adjusting the two policy constants for each variant, warm `target/release/examples/read_fixture`, then measure three separate runs with `strace -f -c`, `valgrind --tool=callgrind`, and a monotonic wall clock. Restore the constants to the defaults afterward.

The runtime-store constants own the per-schema policy: `DATABASE_CACHE_SIZE_KIB` is 4,096 KiB and `DATABASE_MMAP_SIZE_BYTES` is zero. The policy is installed once when each serving connection opens, including both `main` analytics and attached `canvas` on the cost reader. The 15 pooled Canvas readers have a 60 MiB aggregate suggested cache budget; adding the version reader and two writers makes 72 MiB. The cost reader has two additional schema caches, so those listed connections can suggest up to 80 MiB in total. Caches are connection-local, allocated on demand, and SQLite treats cache size as a suggestion, not a hard process memory limit. Other independent repository connections can increase the total.

At the experimental 64 MiB mmap setting, the 18 pooled/version/writer schema mappings have up to 1,152 MiB of virtual address space; adding the cost reader's two mappings gives up to 1,280 MiB. Actual mapped size is limited by database size, SQLite/VFS limits and platform support. Mapping is virtual address space, not preallocated resident memory. WAL pages still use SQLite's normal WAL machinery. SQLite controls mappings when files grow; externally truncating or replacing an open database is unsupported.

**mmap is disabled pending an owner decision.** SQLite documents that an I/O error on an already mapped page can raise SIGBUS and terminate the process instead of reporting an ordinary SQLite error. This changes failure semantics despite the fixture's read reduction. See [SQLite memory-mapped I/O](https://www.sqlite.org/mmap.html). macOS/APFS behavior and performance were not verified.

A baseline debug-build DHAT run of the same setup and ten reads recorded 1,148,033 allocated blocks and 164,066,339 cumulative bytes. Largest individual stacks by block count were SQLite schema parsing (`tokenExpr`/`yy_reduce`, 26,514 blocks), expression duplication (`exprDup`, 25,482), and rename-token mapping during parsing (24,594). These include migrations and fixture creation and do not establish live serving allocation rankings. No allocation changes are made here.

The saved five-minute, 199 Hz production `perf script` profile supplied for this task contains 13,741 samples, of which 1,215 include `snapshot_on`. Grouping by instantiated operation closure gives:

| Operation                        | Samples containing `snapshot_on` | Share of all samples |
| -------------------------------- | -------------------------------: | -------------------: |
| `Worker::idle_state_changed`     |                              326 |                2.37% |
| `pump_due`                       |                              166 |                1.21% |
| `federation::snapshot`           |                              115 |                0.84% |
| `process_controls::read_listing` |                               97 |                0.71% |
| `next_deadline`                  |                               73 |                0.53% |
| `next_due_at`                    |                               59 |                0.43% |

These are sample-weighted stack observations, not invocation counts or a direct measure of dispatch overhead; long operations receive more samples. Stacks without `snapshot_on`, including tasks waiting or dispatching, are excluded. The profile is from GitHub commit `515fcf14`. The idle operation is the strongest candidate to examine first for avoiding or combining work. A dedicated reader thread needs a separate design for cancellation, fairness, pool concurrency and shutdown. Existing opt-in `blockingPoolMetrics` can measure invocation counts and rates in a representative workload.

In the earlier unoptimized whole-process experiment, instructions increased despite fewer preads. The bundled SQLite source marks `cache_size` as requiring the schema, so it can load schema during initialization; the fixture also changes schema after opening the store. Those effects were not separately isolated. Callgrind counts user-space instructions; mapped-page faults themselves are kernel work. The optimized controlled comparison above supersedes that single-run observation.
