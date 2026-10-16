# Reference and acceptance matrix

Status on 2026-10-16: all product implementation and acceptance tests below are pending. Existing research is evidence about SyncFolder 2.0.79, not proof about FolderSync.

| Feature | Reference evidence | Intended FolderSync behavior | Required proof |
|---|---|---|---|
| Tasks and native picker | workflows.md; E05, E22, E50–E53 | Persistent tasks, validated roots, duplication/groups, safe bulk review | Save/restart/edit/duplicate/import tests and native picker GUI workflow |
| Two-way | synchronization-behavior.md T02–T05; tray-and-notifications.md | Common-baseline state machine, preserve concurrent edits | A/B edits, conflicts, deletion, idempotency and SQLite assertions |
| Copy | T09–T11, E60–E61 | A→B updates; retain B-only and source-deleted files | Real fixture integration tests |
| Mirror / Move | E45/E49 UI claims only; runtime unknown | Reviewed mirror deletes; verified copy before Move source removal | Destructive review and failed-transfer tests |
| Preview and conflict resolver | T01; T04 documents an actual overwrite reached from simulation | Immutable plan; conflict choices remain non-mutating until Apply | Filesystem snapshots across nested preview screens; stale-plan rejection |
| Deletion and root loss | T06 simulation only; T07 local missing-root test | Baseline-proven deletion; inaccessible roots stop execution | Missing/partial listing, delete/edit, mass-delete threshold tests |
| History/restore | T10 one preserved version; cap/restore unverified | Protected app-data versions, selective retention, reviewed restore | Overwrite recovery, cap, crash and restore tests |
| Scope/filter/comparison | settings.md; E19–E21/E54–E56 UI only | One matcher across scanner, watcher and plan; documented precedence | Glob, hidden/system, Unicode/case/long-path tests |
| Watcher/incremental index | T12 observed copying; implementation unknown | Windows notify backend, durable dirty paths, ancestor hashes, reconciliation | Both-side events/overflow/restart tests and 10,000-file benchmark |
| Schedules/login | E16–E18/E34–E37; T13 inconclusive | Persisted calendar/periodic rules with missed-run/DST policy | Save/restart/sleep/overlap/calendar tests |
| Background/tray/toasts | Closed-window copies verified; E76 menu visuals; notification occurrence user-reported | Close-to-tray, explicit Quit, conflict/failure attention | Real Computer Use and process/filesystem observations |
| Settings/logs/import/export | E02–E04/E63–E64 UI only | Versioned own JSON, no imported hooks, private diagnostics | Round-trip/schema rejection/log consistency/GUI tests |
| Hooks/chaining/power/splitting | UI observations only | Optional after core; absent controls until implemented | Separate opt-in and safety tests if implemented |
| Mobile/discovery | UI observed, runtime unknown | Out of scope | No corresponding product controls |
| SMB | Reference runtime unknown | UNC roots, notification uncertainty fallback, offline safety | Fault tests; real share testing only on specifically authorized disposable share |

Reference files live in `../syncfolders-research/` and remain untouched. Their verified UI, tested runtime, user-reported and unknown distinctions take precedence over shorthand in this matrix.
