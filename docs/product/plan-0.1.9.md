# FolderSync 0.1.9 — incremental monitoring

Approved scope: implement the four improvements discussed with the user on 2026-10-16. Keep existing root identity, conflict, deletion, staging and crash recovery safeguards.

1. Replace the raw 2,048-event application queue with a bounded, coalescing collector fed directly by the independent native notification callback. Preserve split rename pairing, rescan/error signals and events arriving during synchronization or polling. Persist batches before processing; recover conservatively after persistence errors or restart. Test bursts, limits, rename ambiguity and native changes on both sides.
2. Replace unconditional five-minute scans with a configurable global safety interval (default 1,440 minutes, zero disables only periodic safety scans). Keep five-minute fallback and retry watcher setup when unavailable. Restart, invalidation, overflow, root identity validation and explicit schedules remain authoritative. Test interval policy and setting validation.
3. Skip the second content pass only for freshly generated automatic plans whose operations are all Unchanged. Retain task, generation and root guards and all validation for manual plans or any mutating plan. Test bytes read, baseline advancement, concurrent edit follow-up and stale manual plans.
4. Run the full verification suite, benchmark disposable fixtures including 35k events and incremental follow-up, build the current-user installer, install while idle and inspect the actual GUI with Computer Use. Commit source and measured evidence. Do not mutate the user's existing task files.

Progress and verification results are recorded here as work completes.

- Implemented callback coalescing, explicit Windows backend loss signals, root probes and reconnect fallback; commit d778a7a.
- Implemented automatic equal-observation fast path, batched incremental index observations and ordered child lookup index; commit 9cbe229.
- RED→GREEN observed for native save burst (old queue stopped at 2,048 events), duplicate content reads (88 bytes versus required 44), ordered child query (50,413 VM steps versus a bound below 2,000), and partial-index failure under an exclusive file lock.
- Final workspace Rust rerun: 170 passed. Patched notify library: 8 passed. Frontend: 33 passed. Formatting, all-target Clippy with denied warnings, TypeScript, ESLint and web build passed. Independent read-only review found no actionable issues, including the final batch transaction change.
- Release benchmark: 35,000 files per side / 332,640,000 bytes each. Manual Preview+Apply 42,861 ms; automatic equal reconciliation 42,008 ms, half the content bytes; native single-file event through verified Apply 337 ms excluding debounce, 8 entries, zero full reconciliations. Peak working set observed 194,940,928 bytes. Timings vary with cache/load; no wall-time speedup is inferred from the manual/automatic full comparison.

- Production installer built and installed without elevation; version/settings/real Preview checked with Computer Use. Two task configurations and five protected payloads preserved. Release report and evidence are complete.
