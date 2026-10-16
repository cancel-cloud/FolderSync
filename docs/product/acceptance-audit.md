# Specification acceptance audit

Checkpoint: 2026-10-16, after source `88c18c2` and the subsequent progress/cancellation work. This audit preserves the full `CODEX_GOAL.md` scope. It is a work list, not a declaration of completion. “Verified” applies only to the stated behavior and cited evidence. “Partial” means implementation or verification remains incomplete.

## Core requirements and evidence

| Specification requirement | Current evidence | Status / remaining work |
|---|---|---|
| Rust/Tauri 2, React/TypeScript/Vite/Tailwind/shadcn boundary | Workspace manifests/lockfiles, native commands, local Radix/CVA button; packaged native app | Verified stack; full dependency redistribution/license audit pending |
| SQLite app-data/WAL/migrations/foreign keys/transactions | index.rs; management/index/history tests; installed persisted state | Verified local Windows; server durability is not claimed |
| Original references/assets/privacy | reference-matrix.md; research untouched and untracked; original application left untouched | Markdown and transcript research read; not every screenshot separately inspected; no proprietary assets used |
| Persistent named tasks, native picker, duplicate, delete confirmation | management tests; Computer Use create/pick/save/restart/import | Verified principal workflows; deletion GUI matrix incomplete |
| Editable task IDs | Editable unique public identifier with stable internal UUID; baseline/history/import/restart tests | Verified 82 Rust / 16 frontend full suite and installed Unicode ID save/restart, unchanged baseline and 614 unchanged files in 105cb00 |
| Task grouping | Group label, filter/collapse and transactional bulk reassignment; backend restart/stale-review tests and UI selection regressions | Verified atomic restart/stale-review tests, hidden-selection UI regressions and installed filtering/collapse/bulk reassignment/restart |
| Local/USB/UNC root validation and identity | filesystem/windows_paths/windows_safety tests; native identity checks | Local/junction/Unicode/long paths verified; physical USB and real SMB pending |
| Unsafe pair/task overlap rejection | validate_roots/save_task guards and tests, including login-only triggers in either save order | Automatic intersecting roots rejected; exhaustive alias/reparse races pending |
| Four real modes | planner/integration/windows_paths tests with actual files | Verified local modes; broader GUI per-mode matrix pending |
| Two-way edit/edit, delete/edit, concurrent creates and deletion baseline | integration/automation/review_safety tests, actual GUI conflict preservation | Verified tested cases; simple rename and delayed/duplicate queued-event regressions pass; ambiguous rename detection remains incomplete |
| Immutable Preview/Apply and stale inputs | filesystem tests, frontend confirmation tests, installed 300-file Preview proof | Verified principal boundary |
| Distinct typed operations and reasons/directions | Action enum, PlanSummary, paged operation table and per-side totals | Typed Create/Update/Mkdir/protected removal kinds and explicit per-run Ignore implemented; full verification and installed QA in progress |
| Destructive review and mass-delete protection | Manual exact-plan approval; opt-in automatic common-baseline deletion with entry/percentage limits and root/scope-bound manual proof | Verified automated baseline/limits/race/native events and installed opt-in, hidden-window debounce deletion, protected bytes and exceeded-limit review in 2575c0f |
| Protected replacement/deletion, restore, selective retention | transfer/recovery/retention tests; actual GUI restore | Verified local file-byte recovery; deleted-task recovery catalog, root-bound restore, Preview safety and inactive persistence verified in installed 5a8d0d8 |
| Stage/verify/atomic Windows replacement and crash recovery | real killed subprocess, disk-full injection, displaced recovery, expected-destination race tests | Verified covered boundaries; exhaustive crash/power-loss/AV/adversarial junction testing pending |
| Inaccessible/partial/offline roots cannot infer deletion | missing/offline/reconnect tests and actual nested ACL denial test | Local/fault paths verified; real SMB outage/partial listing pending |
| Shared filters/scope/attributes | shared Matcher; scope/review_safety tests; GUI pattern tester | Principal filters and actual Windows SYSTEM full/incremental scope verified; selected-folder GUI matrix needs coverage |
| Case/Unicode/long paths/mtime policy | fs.rs, semantics; windows_paths/windows_safety/paging tests | Tested Unicode, long paths, case spelling and file mtime; full Windows ordinal casing/ACL/ADS/EFS parity not promised |
| Native both-root monitoring and incremental index/Merkle | watcher/incremental/index/automation tests; hidden-window real copies; 10k benchmark | Verified local event path; real SMB notifications pending |
| Overflow/missed events/restart recovery scan | injected overflow flag, durable uncertainty/restart/reconnect tests | Verified injected/local cases; native OS buffer exhaustion untested |
| Durable queue, baseline transactions, bounded retries/bandwidth | automation/scheduler/transfer modules and tests | Implemented; explicit fairness/large simultaneous-task acceptance needs coverage |
| Schedules/login, last/next run, missed occurrences/DST | scheduler/automation_recovery/runtime tests; actual last-run refresh GUI | Persisted calendar logic verified; actual OS sign-in/sleep/resume and startup registration GUI pending |
| Progress and cancellation | Latest-snapshot sampler; actual streamed-hash/staging cancellation tests; installed partial 16 MiB copy and Stop with source/stage/conflict checks | Verified partial copy/Stop, localized cancellation, source/stage safety and fresh successful retry in installed 2ee8089 |
| Close-to-tray, reopen, Quit, pause | actual hidden-window sync, single-instance reopen and Settings Quit; native menu visible | Tray command dispatch remains unsupported by current Computer Use targeting; process-local pause |
| Toast conflict/failure delivery and task deep link | localized native builders and routing unit test | Actual Windows delivery/activation pending |
| Settings/logging/import-export | actual language/log viewer/native JSON save/open; management/privacy tests | Verified principal workflows; configurable close/tray behavior absent |
| Task speed/concurrency/comparison/conflict controls | Bandwidth/retry/scope/history controls; fixed one engine job, verified content comparison, preserve conflicts | Partial: concurrency/comparison/conflict policy controls absent rather than inert |
| Detailed result/error/actions/history UI | persisted plans, bounded 200-row loading, error banner, protected versions and restore | Principal workflows verified; richer per-operation failure details and failed discovery result navigation need review |
| Localization/accessibility/responsive/paths | English/German helpers, modal focus tests, actual narrow window | Comprehensive screen-reader, high-DPI/minimum-height and full-path copy/expand controls pending |
| Installer/standard-user behavior | current-user unsigned NSIS, installed GUI, uninstall/reinstall state evidence | Verified this user's non-elevated session; separate standard account not created |
| Optional hooks/chaining/shutdown/splitting/mobile/USN | Absent controls, explicit docs | Intentionally omitted as permitted by specification |

## Mandatory test gaps to close

- Complete ambiguous/concurrent rename detection and regressions beyond the verified simple rename, delayed queued notifications and timestamp-preserving edit cases. Actual Windows SYSTEM matching is verified.
- Exercise additional installed failure modes beyond the verified progress/Stop/cancel and successful retry workflow.
- Verify every remaining required task-management control; deleted-task recovery is now verified; do not equate internal UUIDs or group labels with completed management features.
- Extend approved automatic deletion testing to specifically authorized disposable SMB/physical USB roots when available; local baseline, threshold and hidden-window workflows pass.
- Verify startup registration and schedule dispatch through the real app. Actual sign-in/sleep and toast activation need an available OS session/interface; do not simulate these as OS passes.
- Real SMB testing requires a specifically authorized disposable share. No production share or company path is authorized.
- Complete resized/high-DPI/keyboard/screen-reader QA and full-path actions.
- Rebuild/reinstall after final source changes and preserve exact source/hash/test evidence. Existing benchmark measurements remain actual measurements of their recorded build; rerun if engine performance architecture changes.

The hard acceptance gate's manual two-way, Preview safety, protected deletion, crash recovery, persistence, incremental events and real UI paths have evidence. The additional explicit requirements above remain part of the goal; they are not waived by those passing paths.
