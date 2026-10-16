# FolderSync

A Windows x64 desktop folder synchronizer built with Rust and Tauri 2. React presents tasks and immutable previews; Rust owns every filesystem operation. This is an unsigned Windows release. See [actual QA results](docs/product/qa-report.md) before using valuable files.

## Install and run

Run `target\release\bundle\nsis\FolderSync_0.1.1_x64-setup.exe`. The NSIS installer targets the current user, with no service or administrator requirement. Microsoft Edge WebView2 Runtime is required; the Tauri installer can provision it when missing. This machine already has it. No signing certificate is configured.

The optional installer checkbox **Start FolderSync in the tray when I sign in** registers only the current account. It is unchecked for a new install. Silent installation supports `/S /STARTUP=1` or `/S /STARTUP=0`; omitting the option preserves an existing registration. Settings offers the same option. Sign-in uses the quoted installed executable with `--background`, loads saved configuration and leaves the window hidden. Installing or enabling startup does not enable any task, watch, schedule or deletion policy. Previously enabled task triggers still run when FolderSync starts. Uninstall removes its sign-in entry and retains saved data unless you explicitly request removal.

Create a task with two existing writable folders, choose the direction, then Preview. Different pre-existing bytes produce a conflict. Selecting a conflict direction only revises the preview. Apply requires a separate confirmation; every manual removal also needs approval of the exact plan. Existing file contents are protected in `%APPDATA%\dev.foldersync.desktop\versions`.

Closing the window leaves the process running in the tray by default. Settings can instead make Close stop work safely and quit; unsaved task drafts require confirmation. Reopen with the shortcut or executable; single-instance handling shows the existing window. Quit from Settings or the tray stops the process. Triggers are off on new/imported/duplicated tasks. Only enable monitoring after checking the first manual result.

## Implemented

- Two-way, copy, mirror and verified move; baseline-aware conflicts and protected deletions.
- Persistent tasks with editable IDs, groups, duplication, own versioned JSON import/export, native directory picker.
- Scope globs, selected folders, hidden/system filters, filter tester, Unicode/long paths.
- Immutable Preview/Apply, conflict choices, stale-input rejection, history and reviewed restore, including deleted-task recovery.
- Native Windows notifications for filesystem changes, persistent dirty paths and incremental ancestor hashes.
- Calendar/periodic schedules, bounded retries/debounce/bandwidth, pause/cancel, close-to-tray.
- Incremental or full content verification; preserve-and-continue or hold-until-reviewed conflict policy.
- Configurable 1–4 verified file transfers sharing a total bandwidth cap, with cancellation and crash recovery; default one transfer.
- English/German principal screens, expandable/copyable full folder paths, local optional operation logs, no telemetry/account/cloud service.

Automatic deletion defaults off. After a successful full manual reconciliation, two-way/mirror tasks can explicitly opt into proven baseline deletion within both per-run entry and percentage limits. Unknown deletions and Move still stop for review; see [policy details](docs/product/automatic-deletion.md). Roots and mode of an established task require a new task; this prevents carrying a baseline into a different folder pair. Recovery copies are ordinary file content, not an ACL/alternate-stream backup system. An actual unavailable UNC was safely rejected. Creating the dedicated disposable SMB share was denied to this account, so real SMB transfers, disconnect/reconnect and network notification behavior are unverified. Use a stable UNC path and manually review the first result. Actual OS sign-in, notification activation/deep links, high-DPI and comprehensive fault coverage remain unverified; see QA for the full list.

![Installed FolderSync dashboard](docs/product/evidence/installed-dashboard.png)

## Develop

Install Rust stable MSVC, Visual Studio C++ Build Tools plus Windows SDK, Node and pnpm. PowerShell scripts also detect this Codex environment's bundled Node/pnpm. TLS verification stays enabled using Windows system trust.

```powershell
./scripts/dev.ps1
./scripts/verify.ps1
./scripts/benchmark.ps1
./scripts/package.ps1
```

`cargo test --workspace` runs real disposable filesystem fixtures, including subprocess termination during copy. `pnpm test` includes the frontend preview/conflict/confirmation boundary. Benchmarks create conspicuously named synthetic folders in the OS temp directory; no company paths are used. Full initial/verification scans hash every file; a confirmed single-file event hashes only affected entries.

Read [architecture](docs/product/architecture.md), [semantics](docs/product/sync-semantics.md), and [reference mapping](docs/product/reference-matrix.md). Original SyncFolder research remains untouched under `docs/syncfolders-research`; no proprietary assets or formats are incorporated.
