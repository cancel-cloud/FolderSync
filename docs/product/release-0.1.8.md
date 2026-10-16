# FolderSync 0.1.8 — local comparison and small-file performance

The 35,000-file user fixture on C: exposed software overhead rather than a sustained-bandwidth limit. C: maps to a Samsung SSD 860 EVO 500GB (SATA); Windows reports Healthy. That status and these cached-file tests are not an SSD health certification or a cold-device benchmark.

## Why files were checked more than once

Preview hashes both full folder trees. Apply revalidates all reviewed inputs before its first mutation so a stale review cannot partially execute, and each actual copy checks its source/destination again, verifies the staged bytes, flushes data and commits durably. Existing destination content is protected according to the task policy. The pre-Apply barrier and just-in-time guards solve different race windows and are retained.

Monitoring additionally reconciles on startup, after uncertain/overflowed events and every 300 seconds. The observed cancelled user run had copied 17,285,374 bytes with two workers; a subsequent review required run recorded 2,048 watch events and an uncertain full reconciliation. These records explain real repeated discovery; they do not prove a successful full 35,000-file copy. The existing source and destination were not modified by the profiling harness. No company folders or shares were used.

## Changes

- Full discovery and pre-Apply input validation use at most 10 reading workers, capped by logical CPU count and work size. The transfer setting does not control comparisons. Paths, filters, file identity, changed-during-read checks, root validation and fail-closed scan behavior remain active. Workers never write the SQLite index while scanning.
- Reuse compiled scope matchers during a scan, cache recurring SQL statements, and avoid querying historical rename relationships for every path when no historical relationships exist. Content-based ambiguous rename protection still runs.
- Commit directory digest updates together. Persist every pending mutation intent in one durable transaction before any file mutation, then commit each completed operation's journal, baseline, identities and both indexed entries together. SQLite remains WAL + synchronous FULL. Staging recovery records, file flushes and atomic Windows commits remain enabled.
- New tasks default to 4 verified transfers; selectable range is 1–16. Existing explicit task settings remain unchanged. Parallel workers wait up to 30 seconds for SQLite instead of 5 seconds. More transfers do not guarantee more throughput.
- Cancellation/error paths retain worker byte/entry metrics and reject the entire scan/validation; they never return a partial tree as a successful empty folder.

## Reproducible measurements

Run `cargo run --release -p foldersync-engine --example profile_local` (or `-- --quick` for only the 4-worker copy sample). This reads only `.tools/FolderSync-35000-Test/Quelle` and creates unique `.tools/FolderSync-Perf-<UUID>` output/state folders. It never applies a plan against the user's existing destination. Copy samples select the first ten fixture folders, 1,000 actual files totaling 2,150,831 bytes; every output is rehashed and checked against the reviewed source digest. Full comparison measures all 35,000 files / 874,806,229 bytes / 35,350 entries.

Baseline raw evidence: [before](benchmarks/profile-local-before.json). The old 4-transfer copy sample failed with SQLite error 5, database locked; this is explicitly a failed run. The first raw hash pass had a different cache state. An intermediate build repeats raw hashing in forward/reverse worker order: warm 10-worker passes took 1.220/1.368 s versus warm 1-worker 5.756 s. [Intermediate measurements](benchmarks/profile-local-intermediate.json). Its successfully verified copy samples took 26.800/23.474/22.393/24.613 s at 1/2/4/10 workers respectively. Ten transfers were slower than four in that sample.

The isolated SQLite experiment measured 1,000 individual FULL durable commits at 3,448 ms versus 6.43 ms for one transaction containing the same 1,000 rows: [flush experiment](benchmarks/flush-cost.json). This supports the per-file persistence bottleneck; it is not a projected end-to-end speedup. Phase sampling in the harness reports time occupying a progress phase, which can recur during individual copies; it must not be interpreted as an isolated global pre-validation timer.

Performance-pipeline measurements from commit `9eff048`, release build 0.1.8: [raw results](benchmarks/profile-local-latest.json).

| Work | Original 0.1.7 | Final 0.1.8 |
|---|---:|---:|
| Safe full filesystem scan, 35,000 files | 24.754 s | 4.615 s |
| Complete full Preview, 35,350 entries | 57.546 s | 24.732 s |
| Verify-copy 1,000 small files | 40.453 s (1 worker), 40.813 s (2 workers); 4 failed | 24.509 s (4 workers) |

Final raw content hashing took 1.663 s with 10 readers and 6.466 s with one reader. These warm-cache readings are distinct from full Preview, which additionally builds the index and plans/checks conflicts. The 4-worker final copy sample rehashed all 1,000 output files successfully. Copied bytes were exactly 2,150,831; cumulative measured bytes hashed across Preview/Apply were 12,904,986. Copy samples use the same files but not the same concurrency in the before/final comparison; the intermediate report supplies the controlled 1/2/4/10 comparison. A full 35,000-file copy is not claimed. File flushes and safe recovery still have a substantial cost for many tiny files. Antivirus contribution was not isolated and no antivirus/security setting was changed.

Fresh verification passed: **153 Rust tests, 32 frontend tests**, cargo fmt, workspace/all-targets Clippy with warnings denied, TypeScript, ESLint and production frontend build. [Rust/Clippy evidence](evidence/verification-0.1.8-rust.txt), [frontend evidence](evidence/verification-0.1.8-frontend.txt). A cancellation-metrics regression introduced during parallelization was found by the existing streaming-hash cancellation test, fixed and passed in the complete Rust rerun. Frontend test label/encoding corrections were verified by rerunning the complete frontend checks after the Rust suite passed.

Installed GUI verification then exposed mixed per-file progress fields from concurrent hash readers (bytes from one file, length from another). The final package includes a correction publishing each hash chunk's relative display path, bytes and total atomically. Staged verification preserves the reviewed display name. This display-only correction followed the performance measurements above. Fresh workspace/all-targets Clippy and 24 targeted Rust checks passed, covering the new coherent-snapshot unit regression, engine unit tests, parallel scan/validation, transfers, cancellation and crash recovery: [final progress verification](evidence/verification-0.1.8-progress.txt). No content verification or durability check was removed.
