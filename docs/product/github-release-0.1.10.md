# FolderSync 0.1.10 — First Public Release

This is the **first public release of FolderSync**, a Windows desktop application for synchronizing two folders with explicit previews, conflict handling and optional file-version recovery. Built with Rust and Tauri 2; no cloud account is required.

## Highlights

- **Four synchronization modes:** two-way synchronization, mirror A → B, copy A → B, and verified move A → B.
- **Review before changing files:** inspect planned transfers and removals, resolve differing files explicitly, and reject stale plans before applying them.
- **Protected replacements and deletions:** optional per-task version history, restore tools, verified staging, persistent journals and guarded automatic deletion.
- **Incremental Windows monitoring:** watch both folders, coalesce change events and update a persistent SQLite index. Full reconciliation remains available for recovery and explicit checks.
- **Automation and background operation:** schedules, configurable debounce, pause/resume, tray operation and optional current-user sign-in startup.
- **Task management:** groups, bulk actions, file/folder filters, import/export and English/German interface.
- **Live tray status in 0.1.10:** hover over the icon to see whether a job is running, its current phase and task name, or whether the app is idle or automation is paused.

## Install

1. Download **FolderSync_0.1.10_x64-setup.exe** from the assets below.
2. Finish any active synchronization and close FolderSync before upgrading.
3. Run the installer under your Windows user account and launch FolderSync from its shortcut.
4. Create a folder pair, review the first Preview and apply the intended changes. Enable monitoring or scheduling when ready.

**Platform:** Windows 10/11 x64 with Microsoft Edge WebView2 Runtime. Installation targets the current user. Sign-in startup is optional; installing the application does not enable task monitoring, schedules or automatic deletion.

The installer is **unsigned**. Automatic updates are not configured in this release.

## Verification and current limits

- **173 workspace Rust tests, 8 filesystem-notification dependency tests and 33 frontend tests passed**, together with formatting, Clippy, TypeScript, ESLint and production builds.
- Real Windows GUI workflows were exercised during development. The new tray-tooltip formatting/state transitions have automated coverage; its native hover appearance has not yet been visually verified.
- Local folder synchronization is the primary tested workflow. UNC/SMB paths are supported by the engine, but real SMB transfer and disconnect/reconnect testing remain outstanding. Use a dedicated test folder and review the first result before relying on a network share.
- Initial comparisons and explicit full checks still read the complete folder contents. Incremental monitoring avoids that work for ordinary individual changes; it cannot reconstruct changes missed while a folder was unavailable without reconciliation.
- ACLs, alternate data streams, EFS and server-side/power-loss durability are not promised as full-fidelity backup features.

See the repository README and `docs/product/qa-report.md` for measured performance, synchronization semantics and the detailed verification scope.

## Download integrity

**FolderSync_0.1.10_x64-setup.exe** — 4,165,381 bytes.

```text
SHA256 17E3EE6D1B1ADD58B892D9650DBAB8B3BAB018C0E07731AE51FF7DFB2DAA416A
```

The installer was built from source commit `a70b5bf`; tag `v0.1.10` also includes its verification and packaging documentation.
