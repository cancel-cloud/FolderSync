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
