# Architecture

Implemented structure as of 2026-10-16. Verification scope is recorded separately in qa-report.md.

## Ownership

The Cargo workspace contains `crates/sync-engine` and `src-tauri`. The engine has explicit model, fs, engine/planner/executor, transfer, index, recovery, watch, automation, scheduler and logging modules. It does not depend on Tauri. The React/TypeScript/Vite/Tailwind frontend uses local shadcn-style Button primitives (Radix Slot/CVA) and a native HTML dialog for confirmations. It calls typed Tauri commands; it has no filesystem mutation plugin permission.

The Tauri adapter runs blocking engine work on its blocking pool behind a shared mutex. This deliberately serializes all tasks, avoiding overlapping writers. Automatic tasks with intersecting canonical roots are rejected. Notifications/events carry completion summaries; operation detail is loaded separately. Toast activation carries the task ID, opens the window and routes to its Details screen without applying any plan. The OS activation path remains unverified. Recent local logs are bounded to the final 64 KiB. History/status/preview/apply/resolve commands return compact PlanHeader records with aggregate counts, never operation arrays. A separate operation_page command filters/sorts persisted immutable run JSON inside SQLite and returns at most 200 operations. The frontend invalidates rows when the plan or query changes and ignores superseded asynchronous responses. SQLite still reads the stored JSON for aggregation/paging, and the executor still loads a complete plan; this bounds IPC and frontend payloads rather than claiming constant database work for arbitrarily large plans.

## Persistent state

State lives in the current user's Tauri app-data directory (`%APPDATA%\dev.foldersync.desktop`), outside all synchronization roots. SQLite uses WAL, synchronous FULL, foreign keys, busy timeout, keyed lookup/parent indexes and transactions. Schema version 2 migrated entries to include index generation/native file ID; version 3 adds baseline_reviews for manually approved reconciliation identity/scope; version 4 adds recovery_tasks containing original root identities and disabled configuration for deleted-task recovery. A future schema is rejected rather than downgraded.

Tables contain tasks and generations, per-side JSON entries/digests/parents, common baselines (NULL tombstones), cached Merkle digests, immutable plan revisions/run outcomes, operation journal, protected versions, dirty paths, settings/calendar checkpoints, owned staging and displaced-file recovery records. Run status comes from the authoritative status column, including Interrupted. Each successful path commits its own baseline; unresolved conflicts keep their prior baseline in a persistent conflicts table.

## Discovery and incremental work

The scanner enumerates existing roots and streams BLAKE3 through 256 KiB buffers. Metadata is not trusted as content equality. Each entry records size, nanosecond mtime string, type and native file ID when available. Windows root volume/file IDs and canonical paths detect root substitution. Reparse points, nested roots and state overlap are rejected. Non-Unicode paths fail visibly.

`notify` 8.2 uses ReadDirectoryChangesW on Windows, recursively on both roots. Its inspected Windows buffer is 16,384 bytes, below the SMB 64 KiB limit. A bounded 2,048-message channel marks uncertainty on overflow/error. Events are coalesced into durable dirty paths; owned stage/displaced names are excluded. Own writes are revalidated rather than hidden behind a blanket time window.

A single-file event reads only the two corresponding entries and baseline; directory events enumerate the affected subtree. SQLite ancestor hashes are rebuilt from cached child hashes. These hashes summarize known state and cannot discover missed events. Restart, watcher uncertainty, failure and a five-minute monitored safety interval cause a full reconciliation. No USN optimization is implemented. UNC roots use the user's existing Windows access; no credentials are stored. Real SMB reliability has not been verified.

## Execution and recovery

Preview writes internal metadata only. Apply checks task equality/generation, root identities and every planned input before mutation, then checks each operation again. Sources are opened without write sharing. Stages are uniquely created on the destination filesystem, streamed, flushed, rehashed and given the source mtime.

New-file commit uses no-replace MoveFileExW. Replacement uses ReplaceFileW with a unique same-volume backup path journaled before the API call. The actual displaced object is verified and retained in app-data protected history on success AND failure. A concurrent last-moment rename cannot silently discard its displaced bytes. Matching already-protected content is reused so duplicate backup captures do not consume retention slots. A commit race is reported for review even if replacement took place. Startup preserves journaled displaced bytes before removing owned incomplete stages; offline roots postpone their recovery. Task deletion is refused while displaced recovery is pending.

File deletion holds a Windows handle denying writes/deletes, revalidates and backs up the content, then sets disposition on that same handle. Move additionally holds a read guard denying destination writes/deletes through protected source removal. Directory removal is nonrecursive. File/directory conflicts or changed descendants preserve originals. ACLs, alternate streams, EFS, server durability and power-loss atomicity are not promised.

## Lifecycle

The native window hides on Close, retaining tray/worker lifetime. Single-instance reopening shows it. Quit pauses dispatch, requests cancellation and waits for the engine lock before exit. Cancellation checks bounded transfer chunks and bandwidth sleeps. Pause is process-local. Schedules persist next occurrence and durably queue due work before advancing it; restart resumes queued work even if the next date is later. Sleep/wake coalesces missed occurrences; DST gaps advance to a valid minute, repeated civil time occurs once, missing monthly days skip.

Background removal defaults to Review required. Explicit opt-in after a successful full manual reconciliation permits only proven common-baseline removals within both per-run entry/percentage limits. Structured decisions persist with each destructive automatic plan. Move source removal and unknown baselines still require manual review. Automatic jobs cannot create the manual approval proof. Other non-destructive plans apply, including equal snapshots that establish baseline. See automatic-deletion.md. Conflict attention survives independent successful transfers. Retry count is bounded and delay increases exponentially. Logging is off by default, local only, rotates after 5 MiB (one previous file); basic records omit file paths and detailed logging explicitly includes them. Ordinary run history is pruned transactionally to the latest 100 plans per task; Applying/Interrupted recovery runs are retained. Baselines and protected versions are independent.


## Live work state and cancellation

The engine owns a `ProgressState` containing one replaceable snapshot behind a mutex independent of the engine/database lock. Discovery and input validation report current relative path, entries examined and streamed hash bytes; file staging reports actual bytes written and distinguishes protected-version copies. The Tauri adapter samples at 200 ms, emits only changes and sends `null` when work ends. Intermediate samples are overwritten, not queued. The UI also reads the current snapshot after subscribing so opening mid-run has state.

Hashing/staging checks a typed cancellation token between 256 KiB reads and during bounded 100 ms bandwidth waits. Directory walks check it each iteration, including filtered entries. A currently blocked Windows I/O call may still wait for the OS; cooperative cancellation is not a promise of a hard wall-clock deadline. Cancellation preserves completed operations and stops at a safe boundary; it does not roll back committed copies.

Manual Apply returns an `ApplyOutcome` with the persisted compact header and any error. Validation rejection/cancellation creates a separate terminal attempt snapshot, preserving the original reviewed plan and all baselines. Discovery failures record actual partial discovery metrics with no invented operations. Background Apply uses the same error-recording helper. Intentional background Stop retains durable work for a later event/schedule/reconciliation and does not immediately retry or send a failure toast. Cancelled, Rejected, Failed and unresolved conflicts remain distinct. A failed attempt is never presented as a still-running Apply in the frontend.

## Deleted-task recovery

Deleting configuration transactionally removes its active baseline, conflicts, queue and dispatch settings, while retaining original root identities and recovery-only metadata. The archived UUID cannot be reused by save_task. Global Recovery lists archived tasks with protected versions. Restore previews bind archive state, generation, original target identity and full target root; Apply uses the verified transfer executor and protects changed destination bytes. The opposite root may be offline. Restoration never creates an active task or baseline. Startup staging/displaced recovery accepts archived context. Legacy orphaned versions can recover context from a recorded run, never from a guessed current filesystem identity. Archives without protected versions remain internal UUID reservations.

## Task identifiers

Each task keeps its immutable UUID for database relations, history, watcher queues and recovery. A separate editable identifier is displayed in the dashboard and editor, falling back to the UUID when blank. Active effective identifiers are unique under Unicode lowercase comparison; custom identifiers have at most 64 Unicode characters, no controls and no surrounding whitespace. Saving an identifier advances task generation, invalidating reviews without resetting baselines or rewriting historical snapshots. Existing JSON defaults to a blank identifier. Duplication uses an automatic identifier; import retains custom identifiers when unique and assigns automatic IDs to colliding copies. Archived identifiers do not reserve active user-facing names.

## Typed review and ignored conflicts

Operation serialization adds a computed OperationKind from action/metadata, retaining legacy deserialization and avoiding a second mutable source of operation truth. Plan summaries from memory and bounded SQLite queries count Ignore as unresolved plus an ignored subset. The executor skips Ignore without journaling or baseline advancement, keeps Attention and preserves persistent conflicts. No schema migration is needed for the additive review-kind field or the new action enum variant; earlier application versions cannot read newly ignored runs and should not be used to downgrade this build.

## Window close preference

Global close_behavior persists tray (default) or quit in SQLite. A native CloseBehavior atomic snapshot is published only after a successful setting write under the engine lock; window events read it without waiting on that lock. Tray mode hides the main window. Quit mode emits a renderer close request so a layout-updated draft snapshot can request explicit discard confirmation for an unsaved task editor. Clean state or confirmed discard invokes the same pause/cancel/engine-lock-wait Quit path used elsewhere. The renderer must respond to that request; native tray Quit remains available if it stalls. Unsupported preferences cannot alter saved or native state. This preference does not enable automation.
