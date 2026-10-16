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

Actual final build, install, artifact hash and Computer Use results are recorded below after execution.
