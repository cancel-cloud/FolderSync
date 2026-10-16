# FolderSync 0.1.3

This release addresses the second UI-fix list without changing synchronization, conflict resolution or backup retention.

- Removed the sidebar privacy slogan and renamed “Engine ready” to “Ready to sync” / “Bereit zum Synchronisieren”.
- Test-notification feedback belongs to Settings; navigating to Tasks no longer carries it into the dashboard.
- Filter hints are comma-separated, single-line placeholders. The fields still accept actual patterns one per line; hints are never saved as filters. Empty fields show no scrollbar.
- Re-encoded the two invalid Windows-1252 umlaut bytes in App.tsx as UTF-8. The actual German deletion label is tested.
- Backup boxes show the editable task ID, plus an Open button that resolves the exact saved task's backup folder. Internal UUID storage is deliberately retained: editing a public ID cannot orphan previous versions. The displayed base path is the versions directory; the task-specific button opens its actual internal subdirectory. This is a presentation change, not a physical rename of backup directories.
- Local data has its own Open button and separated log/quit buttons. Native opening accepts only the owned storage root or a canonical UUID belonging to a saved/archived task; it rejects traversal and reparse ancestors. Missing task directories are created safely.
- Added read-only presentation/banner diagnostics and a shortcut to Windows notification settings. Default notification sound remains requested. No settings are overridden.

## Notification root cause observed on this machine

Computer Use inspected Windows Settings → System → Notifications. **Do not disturb was ON**, and Windows explicitly stated that notifications would go directly to notification center. The FolderSync sender was ON with **Banners, Sounds**. This explains the user's center-only observation; changing the app payload cannot legitimately override this global policy. No OS setting was changed. To allow banners, turn off Do not disturb or configure FolderSync under priority notifications. Test notification requests alone are not proof of a displayed banner or audible sound.

References: [Microsoft notification/Do not disturb guidance](https://support.microsoft.com/en-us/windows/experience/notifications-and-do-not-disturb-in-windows), [SuppressPopup behavior](https://learn.microsoft.com/en-us/uwp/api/windows.ui.notifications.toastnotification.suppresspopup). Dependency source inspection found no explicit popup suppression; the prior explicit Default sound request remains in all notification paths.

## Verification and installation

Full `scripts/verify.ps1` exited 0: **145 Rust tests / 31 frontend tests**, cargo fmt, Clippy workspace/all-targets -D warnings, TypeScript, ESLint and web production build. All frontend/native source files also decoded strictly as UTF-8. After Computer Use caught the MSIX path mismatch, the native path resolver was corrected; Clippy and all **26 native application tests** were rerun successfully. The engine and frontend were unchanged by that final path correction.

Runtime commits: **75041b7** (UI/diagnostics), **32ad5a1** (physical path resolution). Final optimized Tauri/NSIS build exited 0 from 32ad5a1, and the final installer was reinstalled non-elevated with exit 0.

- Installer: `target/release/bundle/nsis/FolderSync_0.1.3_x64-setup.exe`, **4,126,624 bytes**, unsigned Windows x64.
- SHA-256: `21C42D3FA81D47E601608D2806C437AB508AF09AD4532E06D072013E30161BE6`.
- Installed EXE SHA-256: `591DE3B7FFF5A6D4EB9B1B6BAB2EB48BEDEE434114D66836CDE4655DE6C09A64`.
- Existing startup ON preserved; installation did not launch the app or synchronization. Launch through Start → FolderSync or `%LOCALAPPDATA%\FolderSync\foldersync.exe`. Sign-in startup remains configurable in Settings.

Actual installed Computer Use checks: absent sidebar slogan, friendly ready label, comma-separated filter hints with no scrollbar, German deletion umlauts, task-ID backup box, button spacing, test notification success followed by Tasks without the test hint, and relaunch with saved List/tasks. The first Explorer check exposed logical MSIX AppData being inaccessible to the outside Explorer process. The final resolver uses the real canonical path; after rebuild and reinstall, **both Open buttons succeeded**: local data showed the SQLite files/versions directory, and the task button opened the selected task's four protected files. The archived task's fifth protected file remains in its separate directory.

Exact task JSON, all five version payloads and synchronized fixture bytes were unchanged through both installations and GUI checks. Tree fingerprint: `d896f6e3ba2fb12b9729ecd57f29e3a8f983dddda87208d918499e37969f7176`. No company files or production shares were used; no sync Apply or destructive option was enabled during this UI regression run.

Evidence: `evidence/verification-0.1.3.txt`, `evidence/native-final-0.1.3.txt`, `evidence/package-0.1.3.txt`, `evidence/install-0.1.3.json`, `evidence/state-0.1.3-{before,after}.json` and `evidence/0.1.3-*.png`. Windows Settings was read only; its account/sidebar data was not saved into repository screenshots. Do not disturb remained ON, so an actual banner/sound is not claimed. Public task IDs replace UUIDs in the app's backup presentation; physical backup directory names remain stable UUIDs. Existing release limitations (unsigned build, no updater, no verified real SMB test or OS sign-in) remain.
