# FolderSync 0.1.2 — usability and management fixes

Requested changes:

1. README starts with why to use FolderSync and measured incremental/full-scan performance. Numbers explicitly identify measured 0.1.1 source, not an invented new benchmark.
2. Preference saves no longer insert sync progress above Settings; feedback is anchored, and scrollbar space is reserved.
3. Settings uses Local data / Lokale Daten.
4. Current identifier is dev.lukas.foldersync; human-facing state is %APPDATA%\FolderSync. The historical identifier remains only as a migration/cleanup compatibility key. Migration preserves old source, snapshots committed SQLite/WAL consistently, verifies protected bytes and relocates internal catalog paths. A running legacy instance, junction, incomplete/ambiguous migration, busy DB or invalid backup fails closed. Do not run old executables after switching: the retained legacy folder is recovery-only and later legacy edits are not merged.
5–6. Settings has no footer. General and Version tabs separate preferences from installed app version. The Version section explicitly says updates are not configured; no fake update checker exists.
7. Persistent Tiles/List views; tile cards cap their width, leaving spare grid columns instead of stretching a lone task across a maximized window.
8. Selecting tasks shows Disable/Delete in the header. Disable atomically clears automatic triggers/pending work, preserving manual Preview and all files/baselines/backups. Delete confirms first and atomically removes configuration/baselines while retaining archived recovery. Unknown selections, pending displaced recovery and transactional failures leave all selected tasks unchanged. Background snapshots are revalidated under the same engine lock.
9. Empty filter fields show real placeholder examples, not saved defaults. Local help is bundled and preserves unsaved drafts. docs/help/filters.md contains more examples. User explicitly requested TODO.md for the missing repository URL; no fake remote/link was added.
10. Shorter deletion explanation and checkbox show exactly what propagates, both safety limits, actual recovery path and retention. Backups use versions/<task UUID>/<version UUID>; initial migration also regroups copied legacy flat versions and updates saved restore previews while preserving the original source. Engine-level legacy flat catalogs remain restorable for compatibility. Uninstaller delete-data selection calls a validated helper for the two exact owned data directories; default uninstall and upgrades retain state.
11. Dependency tracing proved omitted sound generated <audio silent="true"/>. Background/test toasts now explicitly request Default sound. Native Windows delivery status and a test button are available; banner/sound/DND suppression cannot be overridden or inferred from delivery permission alone. Existing native permission was Enabled. No Windows settings were changed.
12. Existing Git repository retained and used. Research remains untouched/untracked. No remote configured.

## Verification

Final scripts/verify.ps1 (CARGO_INCREMENTAL=0): cargo fmt, cargo clippy workspace/all-targets -D warnings, cargo test workspace, TypeScript, ESLint, frontend tests and production web build. Actual final exit/results and installed Computer Use evidence appended below.

New actual filesystem/SQLite tests cover nested and legacy backups, consistent migration/restore, safe explicit cleanup, bulk disable/delete atomic rollback and retained recovery. Frontend regressions first failed for unwanted Settings progress/footer, missing Version/Tiles/List/bulk controls, filter placeholders/help, and uncaught deletion errors. Final implementation addresses the observed failures. Review caught simultaneous old/new app risk and bulk error handling; regressions were added before final verification.

Windows notification references: [ToastNotifier.Setting](https://learn.microsoft.com/en-us/uwp/api/windows.ui.notifications.toastnotifier.setting?view=winrt-26100), [toast audio silent flag](https://learn.microsoft.com/en-us/uwp/schemas/tiles/toastschema/element-audio), [Windows notifications and Do not disturb](https://support.microsoft.com/en-us/windows/experience/notifications-and-do-not-disturb-in-windows). The sources distinguish delivery permission, app-requested sound and OS-controlled presentation.

Final source verification: scripts/verify.ps1 exited 0 with CARGO_INCREMENTAL=0, **144 Rust / 29 frontend tests**, formatting, Clippy -D warnings, TypeScript, ESLint and production web build. Runtime commits: 14d58a9 (requested usability/management/notification fixes) and 28019d5 (regroup existing backups). Review found no remaining blocker in staged regrouping, restore previews or source preservation. Full verification evidence: evidence/verification-0.1.2.txt.

## Final package and installation

Optimized Tauri/NSIS build exited 0 from runtime **28019d534aa7a3b470268ea0e72c399617d0976c**, including both runtime commits above. The older 0.1.1 installer does not contain these fixes.

- Installer: `target/release/bundle/nsis/FolderSync_0.1.2_x64-setup.exe`
- Size: **4,100,644 bytes**, unsigned Windows x64.
- SHA-256: `80307619A406DDAFF6E65D49738EC912DD61D292FC739F09FE196C1207C21123`
- Installed EXE SHA-256: `160F0C7B2757C04B1222FB7AB6E5BB9D2B04FD228EABE3F5D8A4440DF34C8D33`.

Actual non-elevated current-user install with `/S` exited **0**. Installation did not launch the app. The user's existing startup preference was ON before installation and remained ON; the installer did not newly enable it.

Run the installer normally as the current user, then launch FolderSync from Start or `%LOCALAPPDATA%\FolderSync\foldersync.exe`. Installer and Settings → General offer sign-in startup. Sign-in starts in the tray with saved configuration; installation alone enables no task monitoring, schedules or deletion policies. This Codex MSIX-hosted session redirects local installation to its package LocalCache; ordinary OS sign-in remains unverified.

## Installed Computer Use and data checks

Computer Use interacted with the actual installed **0.1.2** executable. Verified maximized 2560×1392 Tiles and List, selected-task header actions, actual Disable, Delete confirmation followed by Cancel, editor placeholders, opening/closing bundled filter help, Settings General/Version, absent Settings footer, Local data path and version 0.1.2. Changing action logging Basic → Off → Basic left panels in place. Quit exited the process; relaunch retained List and both tasks.

Initial migration preserved **two task configurations byte-for-byte**, all **five protected payloads**, and all disposable synchronized-folder bytes. Copied versions now reside under task UUID directories; the legacy source remains. Final read-only checks found the same five backup payloads and unchanged synchronized files, tree SHA-256 `d896f6e3ba2fb12b9729ecd57f29e3a8f983dddda87208d918499e37969f7176`. The only intentional task change was Disable clearing inactive schedule fields on the first disposable task. Both tasks remain manual with deletion OFF. No company data was used.

Test notification returned success. Native status initially showed `unknown`; refreshing Settings subsequently reported **Activated**, recorded in a screenshot. Actual sound/banner presentation was not verified. Explicit Default sound fixes the dependency's silent payload without overriding Windows policy.

Evidence: `evidence/install-0.1.2.json`, `evidence/package-0.1.2.txt`, `evidence/state-0.1.2-{before,migrated,final}.json` and `evidence/0.1.2-*.png`. GUI Delete was cancelled; native disposable-fixture tests execute deletion, rollback and retained recovery.

## Remaining limitations

Missing repository URL is tracked in `TODO.md`; help works locally. Auto-updates are not configured. Build is unsigned. Real SMB transfer/disconnect/reconnect, OS sign-in, tray command targeting, notification banners/sound and broad accessibility/high-DPI testing remain unverified. Earlier dedicated SMB-share creation failed Access denied; no network pass is claimed. Uninstaller cleanup is verified on fixtures, not by removing live migrated data. README performance numbers identify measured 0.1.1 source; no new 0.1.2 benchmark is claimed.
