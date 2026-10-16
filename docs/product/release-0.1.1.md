# FolderSync 0.1.1 release

Release scope follows the final user instruction: reliable local synchronization and conservative conflict handling, a current-user Windows installer, optional tray sign-in, and original application artwork. Deferred OS/network checks are limitations below, not simulated passes.

## Install and startup

Installer: `target/release/bundle/nsis/FolderSync_0.1.1_x64-setup.exe` (unsigned x64 NSIS). Run normally as the current Windows user. No service is installed. WebView2 must be available; this machine already has it. Select **Start FolderSync in the tray when I sign in** only if wanted. It defaults off on a fresh install. Silent options: `/S /STARTUP=1` or `/S /STARTUP=0`; omitting STARTUP preserves the existing registration. Settings can change it later.

Registration is exclusively `HKCU\Software\Microsoft\Windows\CurrentVersion\Run\FolderSync`, with a quoted executable and `--background`. The original autostart library was removed because its Windows implementation could try HKLM. Task Manager or organization policy may independently prevent startup; the app reports its registration, not a guarantee of OS dispatch. Startup loads saved configuration into a hidden-window tray process. Manual launch shows the existing single instance. Installation and the startup option do not change task triggers or deletion policies; previously enabled triggers run when the process starts. New/imported/duplicated tasks default to manual with deletion disabled. Uninstall removes registration and retains configuration unless removal is explicitly selected.

The original generated icon source and prompts are in [assets/icon-provenance.md](../../assets/icon-provenance.md). Committed ICO/PNG variants supply executable/window/tray/shortcut and NSIS installer/uninstaller resources.

## Safety and coverage

Local actual-filesystem tests cover four modes, differing initial files, baseline conflicts/deletions, preview-only choices, stale reviews, protected versions/restore, stream verification and atomic replacement, locked/changed/failed destinations, cancellation and killed-process recovery, native watches and durable incremental indexing. The 7e705af rename-lineage changes preserve ambiguity across split notification batches, restart and rename-plus-editor-replacement. Full-content comparison and require-choices policies are explicit preferences; default comparison is incremental.

A non-elevated attempt to create only `.tools/FolderSync-SMB-ReleaseQA` as `FolderSyncReleaseQA` failed with **Access is denied**. No elevation was requested and no company share was used. The actual UNC `\\GINGER\FolderSyncReleaseQA\pair` was rejected with Windows error 67, no task persisted and a local sentinel remained unchanged. Raw evidence: network-share-setup.json and network-unavailable-unc.json. Local/injected disappearance, failed enumeration, reconnection and replacement-root tests exercise identity/reconciliation guards; they are not real SMB transfer/reconnection evidence.

## Remaining limitations

Real SMB transfer, disconnected-share reconnection and SMB notification behavior could not be verified without a permitted disposable share. Actual OS sign-out/sign-in and sleep/resume were not performed. Hidden `--background` process startup and registry registration are checked separately. Tray popup command targeting and actual toast activation are unavailable through the current Computer Use surface. Physical USB, comprehensive screen-reader/high-DPI coverage, exhaustive AV/power-loss races, ACL/ADS/EFS parity and a complete dependency redistribution audit remain unverified. No signing certificate is configured. Optional hooks, chaining, shutdown actions, splitting, USN and mobile/cloud features are omitted. Use a stable UNC; an unavailable root requires validated identity and reconciliation, never deletion inference or a guessed replacement.

Final build, installer checks, GUI results, measurements and hashes are appended after execution below.
