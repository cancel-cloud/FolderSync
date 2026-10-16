# FolderSync 0.1.9 — incremental monitoring

Approved scope: implement the four improvements discussed with the user on 2026-10-16. Keep existing root identity, conflict, deletion, staging and crash recovery safeguards.

1. Replace the raw 2,048-event application queue with a bounded, coalescing collector fed directly by the independent native notification callback. Preserve split rename pairing, rescan/error signals and events arriving during synchronization or polling. Persist batches before processing; recover conservatively after persistence errors or restart. Test bursts, limits, rename ambiguity and native changes on both sides.
2. Replace unconditional five-minute scans with a configurable global safety interval (default 1,440 minutes, zero disables only periodic safety scans). Keep five-minute fallback and retry watcher setup when unavailable. Restart, invalidation, overflow, root identity validation and explicit schedules remain authoritative. Test interval policy and setting validation.
3. Skip the second content pass only for freshly generated automatic plans whose operations are all Unchanged. Retain task, generation and root guards and all validation for manual plans or any mutating plan. Test bytes read, baseline advancement, concurrent edit follow-up and stale manual plans.
4. Run the full verification suite, benchmark disposable fixtures including 35k events and incremental follow-up, build the current-user installer, install while idle and inspect the actual GUI with Computer Use. Commit source and measured evidence. Do not mutate the user's existing task files.

Progress and verification results are recorded here as work completes.
