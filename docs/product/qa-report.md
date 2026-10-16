# QA report — 2026-10-16

This is a working, tested engineering release, not a claim that every acceptance criterion or production hardening item is complete. The authoritative specification is CODEX_GOAL.md. All filesystem experiments used generated `FolderSync-*` temp fixtures or `.tools/FolderSync-GUI`. No company files or production shares were accessed.

## Environment and executed checks

Rust 1.99.1 / cargo 1.99.1, Windows MSVC Build Tools 18 (14.51 compiler), Node 24.19, pnpm 11.25, Windows WebView2 already present. The original compiler prerequisite failure was resolved by an externally supplied toolchain; the rejected installer command was never retried.

Final `scripts/verify.ps1` completed with exit 0:

| Command | Actual result |
|---|---|
| `cargo fmt --all -- --check` | Pass |
| `cargo clippy --workspace --all-targets -- -D warnings` | Pass, no warnings |
| `cargo test --workspace` | 47 tests passed, 0 failed; includes native runtime scheduling/notification routing tests |
| `pnpm typecheck` | Pass |
| `pnpm lint` | Pass |
| `pnpm test` | 2 tests passed in 2 files |
| `pnpm build` | Pass; production Vite assets generated |
| `scripts/package.ps1` / `pnpm tauri build` | Release executable + current-user x64 NSIS installer generated |

Raw command evidence is in `evidence/verification-final.txt` and `evidence/package-final.txt`. No test result is inferred from compilation.

## Filesystem coverage

Executed tests cover baseline truth tables and all four real modes; two-way changes/deletions/edit conflicts; stale plans and missing roots; different first-run bytes; preview safety; persisted conflict attention while unrelated paths sync; import transaction/schema rejection/trigger disabling; SQLite migration and Interrupted status; native notifications on both roots; incremental exact-path reads and Merkle ancestor/empty-directory repair; scope patterns/hidden ancestors; case-spelling divergence; Unicode, paths beyond MAX_PATH and empty folders; nested Move; retention cap and reviewed restore including excluded destinations; log privacy; scheduled missed occurrences/calendar/DST and durable scheduled restart.

Safety cases actually executed include a locked Move destination, read-only replacement destination, external edit during throttled staging, changed destination expectation after backup, disk-full injection during stage write, and a real child process terminated during copy followed by restart cleanup/retry. A separate displaced-object journal restart test proves preservation of captured bytes. Review regressions were observed failing before fixes. The independent review found seven Important defects; fixes address filtered absence/restore, unchanged automatic baseline, case spelling, schedule restart, expected destination/raced replacement, and guarded Move source deletion. Retention initially failed after adding atomic displacement; verified-content dedup fixed it, and the full suite passed afterward.

Not executed: physical disk exhaustion, power interruption, every journal crash boundary, antivirus interaction, exhaustive access-denied partial directory enumeration, adversarial junction swaps, a real native notification-buffer overflow (the overflow signal was fault injected), or a real SMB outage/reconnect. An injected overflow signal and duplicate durable events/offline-root/reconnect are tested. That does not prove a real SMB overflow/reconnect.

## Actual Computer Use sessions

Used the bundled `@oai/sky` Windows Computer Use interface, native screenshots and accessibility observations. No mock UI or browser substitute was used.

1. Launched the native debug application; created `FolderSync GUI acceptance`, used the real Windows directory picker for A and entered the dedicated B path; saved.
2. Preview showed two transfers and created no destination files. Applied with separate confirmation; bytes propagated in both directions.
3. Created different A/B edits; entered conflict direction choice from Preview. Verified the two distinct files remained unchanged. Only explicit Apply/Confirm replaced the selected side; the old content appeared in protected versions.
4. Restarted and verified task/history persistence. Opened Details and reviewed/restored a protected version through its confirmation; verified restored bytes and preservation of the displaced current version.
5. Enabled monitoring, closed the native window, confirmed the process remained alive, created one new file on each side and an edit/edit conflict. Both independent creations propagated after debounce while conflicting A/B bytes stayed distinct. See `gui-background-proof.json`.
6. Changed language to German in the live Settings UI and observed translated screen labels. Later corrected untranslated mode/filter labels. Turned monitoring off and saved; login/schedule remained off.
7. Quit through the actual Settings button; the process ended. Installed the unsigned NSIS build using its current-user silent switch (exit 0), then launched the installed executable through Computer Use. The release used `http://tauri.localhost/`, with persisted task, language, history/conflict state and automatic triggers off.
8. In the installed release, created a new A fixture, Previewed it, verified B remained absent, opened the native HTML modal, then explicitly Confirmed. The file copied to B while the independent conflict remained unchanged. Per-operation result showed Applied. `installed-dashboard.png` and other evidence record this session.

The initial border drag did not resize the window. A subsequent native System menu → Size workflow narrowed it to about 795 logical pixels; Settings and dashboard reflowed with usable controls (installed-narrow-settings.png). High-DPI/minimum-height coverage remains pending. Native tray menu dispatch and OS toast activation have not been observed. Notification routing to task Details is implemented and unit tested; that does not prove Windows delivery or activation. The UI helper's initial installed-app launch reported no targetable window; a fresh window inventory found the running release and subsequent real interactions succeeded.

A UTF-8 write mistake truncated App.tsx during localization work. It was restored from Git, later changes reapplied explicitly with UTF-8, and all frontend checks/tests plus the real installed workflow rerun. This did not alter engine fixtures or research.

## Benchmarks

`benchmarks/latest.json` and `latest-memory.json` contain the final measured synthetic run (debug engine, local filesystem). 10,000 files per side, 1,024 bytes each, 100 folders per side. These are discovery measurements; initial fixture creation and baseline Apply are outside discovery times.

| Phase | Wall time | Entries examined | Bytes hashed |
|---|---:|---:|---:|
| Initial discovery | 27,193 ms | 20,200 | 20,480,000 |
| Single-file incremental discovery | 115 ms | 2 | 2,048 |
| Forced full verification | 25,124 ms | 20,200 | 20,480,000 |

Native event through Apply: 292 ms; 2 native events observed. Benchmark bypasses the configured UI debounce to isolate engine/event work. Peak working set sampled from Windows `PeakWorkingSet64`: 54,206,464 bytes; exit 0. Final engine code matches safety commit e542266; subsequent desktop logging/activation changes do not change discovery. Earlier measured runs remain with dated names for comparison, not substituted for final results. Single-file steady state demonstrably avoids sibling enumeration/rehashing. Full verification is intentionally expensive.

## Packaging

Unsigned installer: `target\release\bundle\nsis\FolderSync_0.1.0_x64-setup.exe`.
Standalone release: `target\release\foldersync.exe`.
Installer is current-user NSIS; normal use requires WebView2, no service/admin. Per-user silent installation completed with exit 0 and produced `foldersync.exe`/`uninstall.exe`. Windows packaged-host filesystem redirection may display the installed path under the Codex package LocalCache; normal launch path is `%LOCALAPPDATA%\FolderSync\foldersync.exe`. Install/uninstall final evidence is recorded separately after the final bundle.

## Remaining limitations and acceptance gaps

- Real SMB was not authorized/tested. UNC support and local failure handling are implemented; server notification/durability behavior is unknown.
- Destructive automatic plans always require manual review; all new removals wait, even when unrelated copies share that plan. There is no unattended destructive company-data demo.
- Mode/root edits require a new task. IDs are UUIDs. Comparison is verified content; concurrency is deliberately one engine job. No inert alternative comparison/concurrency controls are shown.
- English/German presentation now covers actions, statuses, engine reasons, dates/numbers, metadata, weekdays, trigger summaries, native tray labels and notification text. Unknown OS error details retain their original diagnostic text. Comprehensive accessibility/high-DPI QA remains pending.
- SQLite retains 100 ordinary plans per task plus recovery runs. UI commands return compact plan summaries and bounded 200-operation detail pages. SQLite still reads stored plan JSON for aggregation/filtering, and the executor retains a full internal plan; this is not a constant-memory plan-execution claim.
- Pause is process-local. Actual logon/sleep/resume OS workflows and tray dispatch remain unverified. Toast activation depends on the installed Windows environment and is not an OS-tested pass.
- Protected file bytes and mtime are supported; ACL/alternate streams/EFS/directory metadata equivalence and server power-loss durability are not promised.
- There is no reset UI; deleting a task leaves protected bytes in storage but removes the corresponding task recovery UI. Pending displaced recovery prevents task deletion.
- Hooks/chaining/shutdown/splitting/mobile/USN are omitted intentionally. Signing and formal redistribution/license audit remain pending.

The manual two-way, strict Preview, protected deletion, crash staging/restart, persistent state, event-driven incremental and real desktop UI paths are implemented and exercised. The remaining acceptance gaps above prevent a production-ready or fully-complete specification claim.

## Final installer smoke outcome

Final bundle from source commit `0b515c2`: 3,787,235 bytes, SHA-256 `8B5F6C7E3DF6E4AD3779E194B2F79E1D33AE29512608E7225F72A1DAD14660E9`. Actual current-user silent uninstall returned 0, removed the installed executable, and preserved SQLite state. Reinstallation of the final bundle returned 0 and restored the executable. Evidence: `install-smoke.json` and `uninstall-smoke.json`. No UAC bypass or service installation occurred. The development process was stopped via the actual Quit UI; production fixtures and protected state were deliberately retained.

Notification task activation and bounded recent-log viewing were added after the first installed workflow. Routing has a Rust test (47 total Rust tests in the final suite); Windows toast activation itself remains unobserved. The final source also honors disabled notifications on exhausted retries.

## Additional final safety/performance coverage

A regression demonstrated indefinite ordinary-run retention (106 rows after 105 new previews). Transactional pruning now keeps 100 ordinary plans per task plus Applying/Interrupted runs, preserving baselines and file contents; the regression passes. An injected watcher overflow flag is consumed as uncertainty, and a durable duplicate-event queue survives a local root becoming unavailable and reconnects by full reconciliation without deleting the surviving B file. These are local/fault tests, not a real SMB pass. The full suite now has 47 passing Rust tests.

## Additional installed GUI evidence

Changed local logging to Basic through the native select control, applied an idempotent plan with its separate confirmation, then viewed the real NDJSON record in Settings (`installed-log.png`). The record contains IDs/metrics/status and no file paths. The actual native Save As workflow exported schema 1 with one task to `.tools/FolderSync-GUI/config-export.json`. The original native dialog opened asynchronously; re-observation found it. These workflows also exercised the narrow window.

The native Open dialog then imported that exported fixture successfully. SQLite inspection showed two tasks with unique IDs and every monitoring/login/calendar trigger off. The second task is a deliberately imported disposable configuration, not a production task. Both point only to the dedicated fixture roots. No import hook/command execution exists.

Single-instance reopen was verified on the installed application: Alt+F4 hid the window while its original process remained alive; launching another instance exited that second process and showed the original window, with process count 1 (`single-instance-smoke.json`). Final bundle uninstall and reinstall were repeated after the run-retention fix, both exit 0, with SQLite state preserved. The installed fixtures contain two manual-only task configurations. No separate standard-user account was created for testing; installation used this user's current non-elevated session and current-user NSIS configuration.


## Subsequent localization, paging and Windows safety milestone

Localization commit `61469c1` adds shared human-readable action/status/reason formatting, localized dates/numbers/metadata/weekday/trigger summaries and native tray/notification labels. Each review row now includes modification time; totals show transfers/removals per side. The language-change handler updates the existing tray menu, preserving command IDs. Native notification labels are unit tested; OS delivery/activation is still unverified.

A separate `operation_page` command returns at most 200 persisted operation records; Preview, Apply, Resolve, History and Status return compact headers. UI query/plan changes invalidate old rows and superseded async responses are ignored. Tests use 405 actual files to verify counts, page bounds, filtering and zero destination writes before Apply. A 10,000-operation SQLite-only fixture proves header size below 4 KiB and count parity; this is a serialization test, not a filesystem benchmark. Independent review found a Unicode filtering regression. The real `Änderung.txt` regression first failed, then passed after both operands used Rust Unicode folding through a deterministic SQLite function.

Three actual Windows tests additionally passed: deny directory-list access on one disposable nested directory (Preview/queued reconciliation fail without deleting B, and the test restores the ACL); junction root/nested-loop rejection without following the target; exact file mtime preservation after verified copy. These tests do not cover adversarial junction swaps or a full SMB ACL matrix.

The new FolderSync tray menu was visually inspected through Computer Use. The original SyncFolder menu was recognized and left untouched. A paused header was observed and automation was restored to ready through the app control. Attempts to target native menu commands dismissed the menu or were rejected by the Computer Use window guard; native Open/Stop/Quit dispatch is not claimed verified. Settings Quit again ended the installed process before upgrade.

Fresh scripts/verify.ps1 completed with exit 0 after the Unicode fix: fmt, Clippy (-D warnings), 54 Rust tests, TypeScript, ESLint, 7 frontend tests and Vite production build all passed. Raw output: evidence/verification-paging.txt. The previous final-build section is retained as historical evidence; the newer milestone supersedes its test counts.


Installed build from `35416ea` was exercised through Computer Use with 300 new files under `.tools/FolderSync-GUI/A`. Preview showed 300 A→B transfers, zero removals, one existing conflict and four unchanged entries. B contained zero new files after Preview, after navigating to page two, after filtering `paging-299`, and while the separate confirmation dialog was open. Only Confirm copied all 300 files. Byte comparison passed for every pair and the existing conflict's two SHA-256 hashes were unchanged (`paging-before.json`, `paging-after.json`). The German review displayed friendly actions/reasons, per-side counts and modification times. Actual History loaded the saved 305-entry run through compact headers.

This GUI session exposed stale dashboard status after manual Apply. A new assertion in the existing confirmation workflow failed (dashboard could not find Applied). Apply now refreshes tasks/statuses before its completion message; the assertion passed. Typecheck, ESLint, all 7 frontend tests and production frontend build passed (`verification-ui-refresh.txt`). An accessible table caption now distinguishes reviewed input metadata from execution status and directs users to Fresh preview for current filesystem metadata. The Rust engine was unchanged after the 54-test verification.


Final installed build for this milestone is source commit `65f96b3`. `scripts/package.ps1` / Tauri build completed with exit 0 and generated the unsigned current-user x64 installer (3,815,108 bytes), SHA-256 `BFB56E1C6D60274BDDF550FBB61BE10E50447CEA891C6CED2464977A94B39937`. Current-user silent reinstall returned 0; the executable and persisted SQLite state were present (`install-ui-refresh.json`). Uninstall of the preceding paging build returned 0, removed the executable and preserved state (`uninstall-paging.json`); installer/uninstaller configuration was unchanged by the final frontend fix.

Computer Use launched this final installed build. A fresh `status-refresh-proof.txt` existed only on A before Preview. After its separate Apply/Confirm, both files matched. Returning directly to the dashboard showed the new run time `16.10.2026, 12:56:30` without using Refresh (`installed-status-refreshed.png`). The reviewed-metadata caption was visible and exposed as the table's accessible name. Two manual-only fixture tasks remained; monitoring, login and schedules were all disabled. The original conflict remained distinct. This completes the localization/paging/status-refresh milestone, not the remaining specification acceptance gaps.


## Progress, cancellation and failure-result milestone

Fresh `scripts/verify.ps1` completed with exit 0: fmt, Clippy with warnings denied, **59 Rust tests**, TypeScript, ESLint, **9 frontend tests**, and production web build. Raw output: `evidence/verification-progress.txt`. This supersedes earlier test counts without changing the historical evidence.

Actual filesystem tests observed a partial 8 MiB throttled staged copy, requested cancellation, verified the old destination and source remained intact, checked stage cleanup and Cancelled history/basic-log status, then completed a fresh retry. A separate 32 MiB fixture was cancelled during both Preview hashing and Apply validation hashing. Discovery cancellation persisted partial hash metrics; validation cancellation recorded a distinct terminal attempt. Background validation cancellation preserved its durable queue and did not create the destination. Stale Apply rejection stored a terminal history attempt without modifying files. Review findings for uncancellable Move verification and flat directory enumeration were fixed; discovery paths/byte counters now update continuously. Metadata I/O was moved outside the progress mutex.

Frontend tests additionally cover persisted failed Apply display/dashboard refresh and a live progress event with actual byte values followed by Cancel → native stop command. These are UI unit tests; installed progress/Stop interaction is recorded separately when executed. Existing test fixtures remain explicitly disposable. `acceptance-audit.md` tracks the full remaining specification scope rather than treating the tested core as all requirements satisfied.

Installed source `bf25084` was tested through Computer Use with an actual 16 MiB fixture at 512 KiB/s. Discovery showed changing path/count/hash progress. Apply showed 4,718,592 staged bytes before Stop. The saved result was Cancelled; the destination remained absent, source and pre-existing conflict hashes were unchanged, and no staging file remained (`cancel-ui-before.json`, `cancel-ui-after.json`, installed progress screenshots). The intentional stop still used a generic error banner; the next source fixes that presentation and records the interrupted operation as Cancelled.

Fresh verification after those fixes and the login-only overlap regression completed with exit 0: **61 Rust tests and 11 frontend tests**, formatting, Clippy, TypeScript, ESLint and production frontend build (`evidence/verification-progress-overlap.txt`). Typed cancellation crosses the command boundary through a reserved marker; arbitrary OS diagnostic text cannot be mistaken for deliberate cancellation. Login-triggered tasks now participate in the same overlapping-root safety guard as monitored/scheduled tasks.


Installed build `2ee8089` (unsigned current-user NSIS, 3,829,256 bytes, SHA-256 `254C5DCC899D53E3FE85FAC2F3ED04F564988AE43035CA40C4F694AA16406128`) installed with exit 0. Computer Use repeated Preview → separate Apply/Confirm → observed partial throttled copy → Stop. The German cancellation message appeared in a neutral warning banner, the run and interrupted operation both persisted as Cancelled, the destination was absent, staging was cleaned, and source/conflict hashes were unchanged (`cancel-refined-after.json`, `installed-cancel-refined.png`).

Using the real task editor, bandwidth was reset from 512 KiB/s to unlimited and saved. A fresh Preview and separate Apply/Confirm copied the entire 16 MiB fixture. Both SHA-256 hashes matched the original; the unrelated conflict remained unchanged and attention stayed visible. The operation persisted Applied and the run Attention (`cancel-fresh-retry.json`, `installed-cancel-retry.png`). Both fixture tasks retained monitor/login/schedules OFF. No company files or shares were accessed.


Benchmark rerun against source `2ee8089` completed with exit 0 after packaging/GUI work was idle. Debug engine, 10,000 files per side, 1,024 bytes each, 100 directories per side: initial discovery 23,822 ms (20,200 entries; 20,480,000 hashed bytes); single-file incremental discovery 139 ms (2 entries; 2,048 hashed bytes); forced full discovery 24,432 ms (20,200 entries; 20,480,000 hashed bytes). Native event through Apply was 324 ms with 2 native events; configured UI debounce is excluded. Windows sampled peak working set was 54,255,616 bytes. Current raw results are benchmarks/latest.json and latest-memory.json; before-progress files preserve the preceding measurements. The incremental acceptance condition still passes after progress/cancellation instrumentation.


## Functional task groups

Added a group filter, collapsible group sections and bulk group assignment/ungrouping. Rust validates all requested task IDs before a single transaction writes configuration and increments review generations; groups persist across engine restart, and user files are untouched. Existing immutable run snapshots keep their historical group label. Filtering or collapsing clears relevant selections. Independent review found a selected task could remain selected after editing it into another hidden group; a regression reproduced that failure, and saving task configuration now clears selection.

The full verification script passed with 62 Rust and 12 frontend tests (`evidence/verification-groups.txt`). After the subsequent frontend-only selection correction, TypeScript, ESLint, all **13 frontend tests** and production frontend build passed again (terminal session 55961, exit 0). Native Rust code did not change after the full suite. The first verification attempt failed Clippy's cloned_ref_to_slice_refs check in the new test; corrected to slice::from_ref before the passing full run. Installed group GUI verification is pending at this checkpoint.


Computer Use verified installed `7c65ad2` bulk-assigned both disposable tasks to GUI-Gruppe, filtered it, collapsed its cards, expanded it with Return, assigned only the first task to GUI-Einzeln, then selected that task and changed the filter to GUI-Gruppe. The first task disappeared and both bulk actions became disabled, with zero selected. Actual SQLite inspection verified distinct groups, all automatic triggers OFF, and unchanged fixture/conflict hashes. Screenshots: installed-groups-collapsed.png and installed-groups-filtered.png. A cramped German toolbar label was observed and corrected with wrapping controls and unbroken label text.

Final group-layout bundle from source `6784ab0` built successfully and installed with exit 0: **3,846,040 bytes**, SHA-256 `24140E59B8CD3B800BC0F142FB6EDF51E09890920FE4837803A8A4EAE302846F`. After actual Settings Quit and relaunch through Computer Use, both distinct groups remained visible and the toolbar labels were readable (`installed-groups-restarted.png`, `groups-gui-restarted.json`). Attempts to perform an additional resize did not change the observed bounds; no additional resized/high-DPI pass is claimed. Installer configuration remains current-user unsigned NSIS; this milestone did not repeat uninstall.

## Event and attribute matrix coverage

The latest full scripts/verify.ps1 passed with exit 0: **66 Rust tests**, **13 frontend tests**, formatting, Clippy with warnings denied, TypeScript, ESLint and production frontend build (`evidence/verification-groups-events.txt`). Newly added actual-file coverage verifies same-length edits with Windows mtime restored, duplicate/delayed queued notifications using latest bytes, rename create/delete planning with old destination preserved until explicit approval and a recovery version retained, plus real Windows SYSTEM file/directory attributes shared by full and incremental scope matching. Delayed notification ordering is injected at the durable queue; it is not a claim of forcing Windows to deliver out-of-order notifications. The first timestamp test incorrectly expected discovery-only counters from cumulative process_queue/Apply metrics; measuring preview_paths separately corrected the assertion. All new tests passed without changing runtime engine behavior. Installed artifact remains source 6784ab0; subsequent changes are tests/documentation only.
