# FolderSync 0.1.10 — live tray hover status

Source commit: `a70b5bf`. Scope: show whether a job is running when hovering over FolderSync's Windows tray icon.

The tooltip now displays:

- `FolderSync — Aufgabe läuft: Dateien übertragen · lukas test` (localized phase and cached task name).
- `FolderSync — Kein aktiver Auftrag` after work finishes or is cancelled.
- `FolderSync — Automatik pausiert · Kein aktiver Auftrag` when automation is paused and no job is active.

Active work takes precedence over the paused flag: pausing automatic triggers does not imply that an existing transfer has stopped. Manual Preview, validation, synchronization, transfers, protection and verification all count as active work. English and German are supported, with the entire tooltip bounded to 127 UTF-16 code units and control characters removed from task names.

The existing 200 ms progress sampler updates the native tooltip only when its text changes. No additional worker thread or full filesystem scan was introduced. Task names/language are cached, refreshed at most once per second through a nonblocking engine-mutex attempt; a long transfer cannot block the running/idle indicator. A newly saved task that starts before metadata refresh can initially show activity without its name. Window visibility does not control the sampler.

## Verification

The old static tooltip failed all three new regression tests. The final implementation passes the running → idle/pause tests, every English/German phase, Unicode/task-name sanitization and the Windows length bound. The existing progress sampler test verifies completion returns to idle.

Full verification passed: **173 workspace Rust tests, 8 patched-notify tests, 33 frontend tests**, formatting, workspace/all-target Clippy with denied warnings, TypeScript, ESLint and production frontend build. The optimized Windows NSIS build also passed. [Verification](evidence/verification-0.1.10.txt), [original failures](evidence/0.1.10-tray-red.txt), [packaging](evidence/package-0.1.10.txt).

## Package and installation status

- Installer: `C:\Users\Lukas\Developer\SyncFolders\target\release\bundle\nsis\FolderSync_0.1.10_x64-setup.exe`.
- Size: **4,165,381 bytes**, Windows x64, unsigned, current-user installer.
- SHA256: `17E3EE6D1B1ADD58B892D9650DBAB8B3BAB018C0E07731AE51FF7DFB2DAA416A`.

**Not installed during this change.** Computer Use confirmed the existing 0.1.9 user copy was still active at the final check: 28,758 / 35,350 entries and 8 / 8 active transfers. No Stop, Quit or installer was invoked. The running application, saved task configuration and ongoing copy were left untouched. Run the installer after the active job finishes, then reopen FolderSync. The native Windows hover appearance remains unverified: Computer Use provides no taskbar/hover action on this machine, and the active installed process was not replaced for testing. The tooltip behavior is covered by automated state/formatting tests; these are not represented as a visual OS-hover pass.

[Package status](evidence/package-0.1.10.json), [actual active-job screenshot](evidence/0.1.10-active-job-update-deferred.png).
