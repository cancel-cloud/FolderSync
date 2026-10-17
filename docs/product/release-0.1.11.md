# FolderSync 0.1.11

Numeric editing fix, 2026-10-17. Source commit: `44b4dcd` (signed as cancel-cloud).

## Behavior

All nine numeric task controls share an editable text draft: waiting time, bandwidth, transfer concurrency, deletion entry/percentage limits, periodic schedule interval, monthly day, retries and version retention cap. Clearing a number no longer writes `Number("") === 0` into the task and immediately inserts a zero. Type the replacement value normally, for example clear 20 and enter 5 seconds. Leaving a field empty restores the last numeric value; an empty draft never changes the saved numeric setting. Explicit zero remains available where allowed. Existing min/max, disabled states, defaults, backend validation and synchronization safeguards are unchanged.

## Verification

The regression test failed on the previous implementation with expected empty text / received `0`. It now clears and replaces all nine controls, covers empty-field blur and explicit zero, and saves the replacement waiting time through the actual task editor. Shared-control tests cover external value changes and native range/disabled constraints.

`./scripts/verify.ps1` runs formatting, workspace/all-target Clippy, 173 workspace Rust tests, 8 patched-notify tests, TypeScript, ESLint, 36 frontend tests and a production web build. All checks passed; see [verification output](evidence/verification-0.1.11.txt).

The release description requested by the user is exactly `fix`. Windows executable signing is separate from signed Git commits: the installer remains unsigned.

## Windows package and GUI

Production NSIS packaging completed successfully. Silent current-user upgrade exited 0 and the registered version is 0.1.11. The installed payload matches the compiled executable except for the expected three-byte Tauri bundle marker (`UNK` to `NSS`).

Computer Use verified the installed app: waiting time 20 could be selected and erased to a visually empty field, then replaced with 5 and retained after Tab. The bandwidth field's existing 0 could also be erased; Tab restored its previous 0. Windows UI Automation represents an empty spin button as 0, so empty-field verification used the actual screenshot. The test used an unsaved new-task draft, discarded afterward; the three saved tasks remain. Settings -> Version visibly showed 0.1.11. A previously enabled test-folder monitor performed its ordinary startup reconciliation; no new task or automatic trigger was saved.

[Five-second edit](evidence/0.1.11-numeric-editing.jpg), [empty numeric field](evidence/0.1.11-empty-number.jpg), [installed version](evidence/0.1.11-installed-version.jpg), [package evidence](evidence/package-0.1.11.json), [build log](evidence/package-0.1.11.txt).

Installer: `target/release/bundle/nsis/FolderSync_0.1.11_x64-setup.exe`, 4162891 bytes.

SHA256: `6E3B5262F2A83BE090AFDF139682C48C292A0ACFD6B2D58AA6A7D6CB7F9EDF0A`.
