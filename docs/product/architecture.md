# FolderSync architecture

Design contract only. No product components are implemented or tested yet.

## Ownership and modules

- `sync-core`: typed task/scope/entry/plan models, shared matcher, root validation, scanner, planner, verified executor and recovery. Has no UI dependency.
- `sync-index`: bundled SQLite migrations, per-side entry cache, baseline/tombstones, dirty queue, directory digests, immutable run history and operation journal.
- `sync-watch`: Windows recursive notifications through notify; event coalescing, dirty-path invalidation, overflow/disconnect uncertainty. No direct file mutations.
- `sync-scheduler`: persisted trigger definitions, calendar/periodic due times, overlap prevention, pause/resume and missed-run rules.
- Tauri 2 adapter: per-user app-data location, typed commands/events, native folder dialog, tray, autostart and notifications. Coordinates a bounded worker queue. React/TypeScript has no filesystem mutation API.

All run origins use the same planner and executor. Serialize per task and acquire root-intersection guards across tasks. A task edit invalidates its pending previews. Imported task configuration remains disabled until validated and reviewed; arbitrary hooks are not accepted.

## SQLite design

Store the database in the Windows user's app-data directory, outside task roots. Enable foreign keys, WAL and a busy timeout; use prepared statements and explicit transactions. Schema migrations have versioned, transactional steps.

Tables: `tasks`, `settings`, `entries(task_id, side, path_key, display_path, kind, size, mtime_ns, file_id, digest, generation)`, `baseline(task_id, path_key, digest, tombstone, generation)`, `directories(task_id, side, path_key, digest, generation)`, `dirty_paths(task_id, side, path_key, uncertainty, queued_at)`, `runs`, `operations`, `versions` and `schedule_state`. Composite indexes support exact-path/parent lookups and pending work. Completed run records are immutable; current task status is derived separately.

Baseline updates follow durable operation completion. Never commit success merely because a plan was created. Conflicts retain their prior common baseline. Startup reconciles journal entries that were in progress at shutdown.

## Incremental processing

Install watchers before initial enumeration and buffer concurrent events; reconcile the buffer before marking the cache certain. Events are hints, not proof of absence or stable content. Coalesce bursts with the task's debounce, initially 20 seconds, and validate stabilization/unlock before reading.

A known single-file event updates that entry and affected ancestors only. A known directory event scans the narrow subtree. Dirty records survive process interruption. Ancestor digests are built from sorted child names/types/content digests; they summarize known state and cannot detect missed external changes on their own.

On event overflow, watcher error, root disappearance, unsupported notifications or process restart, mark the affected scope uncertain and perform controlled reconciliation. Cache reuse is forbidden where uncertainty could hide timestamp-preserving edits. A complete verify scan hashes files when required for correctness. Explicit full reconciliation remains available.

Self-write events are revalidated against verified post-operation state; never suppress a time window that could hide an external edit. Rename pairs may narrow refresh scope but cannot independently authorize destructive actions.

## Windows and SMB

Use the Windows notify backend built on ReadDirectoryChangesW, watching both roots. Verify the chosen release's network buffer stays within the documented 64 KiB limit before shipping. Unsupported network notifications require a visible reconciliation policy; offline changes cannot be reconstructed from the local cache.

Root canonicalization, filesystem identity and reparse-point checks precede execution and are repeated during it. UNC paths remain visible and are not rewritten to mapped drive assumptions. No credentials are stored. Real SMB coverage requires a specifically authorized disposable share; local error tests cannot prove SMB behavior.

USN is optional and initially omitted. No administrator rights or Windows service is required for normal application use. ACL equivalence, arbitrary alternate streams and complete timestamp parity are not claimed without corresponding tests.

## Lifecycle and automation

Closing the main window hides it while the process and tray remain active. Quit cancels or checkpoints active work and exits explicitly. Pause stops scheduling new work; Stop requests cancellation of current work at safe boundaries. Debounce and queued-path state must survive worker restarts.

Schedules persist their last dispatched occurrence and next due time. On wake, dispatch at most one missed occurrence per task, then compute the next future occurrence. Never overlap. Calendar schedules use local civil time and an explicit DST policy: nonexistent times advance to the first valid instant, repeated times dispatch once. Monthly invalid days skip that month. These rules require tests before activation.

Notifications distinguish completed transfers, pending conflicts and failures. Deep links must route to a real task/run. Event payloads are bounded summaries with paginated detail queries; backend work and filesystem access never run on the UI thread.

## Verification and diagnostics

Instrument enumerated entries, hashed bytes, copied bytes, watcher events, queued paths and reconciliations per run. The benchmark uses at least 10,000 synthetic files and reports initial, incremental and forced-full measurements plus peak memory. No performance claims precede executed measurements.

Use tracing with configurable local verbosity and redacted diagnostics by default. No telemetry or remote reporting. Optional hooks, chaining, power actions, splitting and mobile controls remain absent until separately implemented and verified.
