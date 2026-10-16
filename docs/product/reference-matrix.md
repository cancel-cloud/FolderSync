# Reference and acceptance matrix

Status on 2026-10-16: the table records the original evidence and required proof. The implementation/test outcomes below supersede the initial pending checkpoint. Existing research is evidence about SyncFolder 2.0.79, not proof about FolderSync.

| Feature | Reference evidence | Intended FolderSync behavior | Required proof |
|---|---|---|---|
| Tasks and native picker | workflows.md; E05, E22, E50–E53 | Persistent tasks, validated roots, duplication/groups, safe bulk review | Save/restart/edit/duplicate/import tests, atomic group reassignment and hidden-selection regressions; native picker GUI workflow |
| Two-way | synchronization-behavior.md T02–T05; tray-and-notifications.md | Common-baseline state machine, preserve concurrent edits | A/B edits, conflicts, deletion, idempotency and SQLite assertions |
| Copy | T09–T11, E60–E61 | A→B updates; retain B-only and source-deleted files | Real fixture integration tests |
| Mirror / Move | E45/E49 UI claims only; runtime unknown | Reviewed mirror deletes; verified copy before Move source removal | Destructive review and failed-transfer tests |
| Preview and conflict resolver | T01; T04 documents an actual overwrite reached from simulation | Immutable plan; conflict choices remain non-mutating until Apply | Filesystem snapshots across nested preview screens; stale-plan rejection |
| Deletion and root loss | T06 simulation only; T07 local missing-root test | Baseline-proven deletion; inaccessible roots stop execution | Missing/partial listing, delete/edit, mass-delete threshold tests |
| History/restore | T10 one preserved version; cap/restore unverified | Protected app-data versions, selective retention, reviewed restore | Overwrite recovery, cap, crash and restore tests |
| Scope/filter/comparison | settings.md; E19–E21/E54–E56 UI only | One matcher across scanner, watcher and plan; documented precedence | Glob, hidden/system, Unicode/case/long-path tests |
| Transfer concurrency | settings.md advanced controls; original runtime unverified | Default one, explicit 1–4 independent verified transfers, shared bandwidth and ordered directories/deletions | parallel_transfers real stages, four modes, cancellation, destination race, protected replacements and killed-process recovery; installed workflow recorded separately in QA |
| Watcher/incremental index | T12 observed copying; implementation unknown | Windows notify backend, durable dirty paths, ancestor hashes, reconciliation | Both-side events/overflow/restart tests and 10,000-file benchmark |
| Schedules/login | E16–E18/E34–E37; T13 inconclusive | Persisted calendar/periodic rules with missed-run/DST policy | Save/restart/sleep/overlap/calendar tests |
| Background/tray/toasts | Closed-window copies verified; E76 menu visuals; notification occurrence user-reported | Close-to-tray, explicit Quit, conflict/failure attention | Real Computer Use and process/filesystem observations |
| Settings/logs/import/export | E02–E04/E63–E64 UI only | Versioned own JSON, no imported hooks, private diagnostics | Round-trip/schema rejection/log consistency/GUI tests |
| Hooks/chaining/power/splitting | UI observations only | Optional after core; absent controls until implemented | Separate opt-in and safety tests if implemented |
| Mobile/discovery | UI observed, runtime unknown | Out of scope | No corresponding product controls |
| SMB | Reference runtime unknown | UNC roots, notification uncertainty fallback, offline safety | Fault tests; real share testing only on specifically authorized disposable share |

Reference files live in `../syncfolders-research/` and remain untouched. Their verified UI, tested runtime, user-reported and unknown distinctions take precedence over shorthand in this matrix.

## Executed implementation mapping

| Feature | Implementation and actual proof | Remaining gap |
|---|---|---|
| Tasks/picker | Rust persistent tasks + native Tauri dialog; Computer Use created pair, picked A, saved/restarted/configured; management import tests | Established roots/mode require fresh task; editable public IDs with stable internal UUID; task_identifiers tests cover baseline/history/restart/import semantics |
| Four modes | Pure classifier plus real executor; planner/integration/windows_paths tests | Real SMB modes pending |
| Preview/conflict | Immutable plans, resolve returns a new plan; GUI filesystem snapshots before/after selection; App.test.tsx proves no Apply until Confirm | Typed operation kinds and per-run Ignore/Keep originals choices have automated and installed Preview/Apply evidence; broad GUI deletion matrix pending |
| Deletion/root loss | Common baseline, protected versions, guarded deletion, nonrecursive directories; integration + safety + review_safety tests | Local ACL-denied nested enumeration is tested; partial SMB enumeration fault matrix pending; manual removals require review; opted-in baseline-proven automatic deletion has per-run entry/percentage limits, tested with real native events |
| History/restore | Protected app-data catalog, cap and verified displaced-content dedup; recovery/retention/transfer unit tests; actual GUI restore | History capped at 100 ordinary plans per task; no reset UI; deleted-task catalog, original-root-bound restore and startup staging cleanup covered by deleted_task_recovery tests; installed verification recorded in QA |
| Scope/path | Shared matcher, hidden ancestor/pair exclusion; filesystem/scope/review_safety/windows_paths tests | Actual junction root/loop rejection tested; full Windows Unicode ordinal casing and adversarial junction swapping unverified |
| Incremental | Native watcher, durable queue, exact records, cached ancestors; watcher/incremental/automation/index tests; 10k benchmark | Real network notification/reconnect reliability pending |
| Schedule | Calendar/DST + durable scheduled startup recovery; scheduler/automation_recovery/runtime tests | Actual sleep/logon OS session not exercised |
| Background | GUI close plus real both-direction copies/conflict preservation while hidden; process remained alive | Tray menu dispatch/toast deep link not verified |
| Settings/logs | Language change through GUI; private-basic/detailed log test; JSON round trip/schema/import atomicity tests | Status/action/reason/date/weekday/native tray/toast presentation localized; unknown OS diagnostics retain original text; native export/import GUI verified |
| Recovery | Actual killed subprocess during staged copy, stale destination race, disk-full injection, read-only/locked destination tests, displaced restart test | No exhaustive crash at every journal boundary; physical power-loss/AV faults pending |
| Packaging | Tauri release + x64 current-user NSIS generated | See QA for subsequent install/uninstall evidence |
| Optional | No hooks/chaining/power/splitting/mobile/USN controls | Intentionally omitted |

All Markdown research and locally supplied unique transcript text were read. Fixture evidence was inspected without modification. Not every original reference screenshot was separately visually verified; its contents are not treated as new runtime proof. GUI evidence here refers to the new FolderSync application only. Research files remain untracked because the original indexing permission failure was not bypassed.

Backend review queries now return compact summaries and at most 200 operation details. Real 405-file paging/Preview safety tests and a 10,000-operation SQLite serialization fixture cover bounded payloads, aggregate parity and literal search input. Frontend tests cover page navigation/filter reset and separate Apply confirmation. The 10,000-operation storage fixture is not presented as a filesystem performance benchmark.
