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

Final-source measurements, verification and installation evidence follow after execution. The full 35,000-file comparison is measured; a complete 35,000-file copy is not claimed. File flushes and safe recovery still have a substantial cost for many tiny files. Antivirus contribution was not isolated and no antivirus/security setting was changed.
