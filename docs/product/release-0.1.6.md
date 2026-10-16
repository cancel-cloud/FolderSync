# FolderSync 0.1.6 — optional file history and clearer task setup

New task IDs are reserved in memory when creating a draft; that never implied persistence. Unsaved tasks now show an automatic-ID placeholder, a draft explanation and no usable backup-folder link. Saved IDs remain stable.

The editor explains manual execution (Save → Preview/Start → review and Apply), monitoring (reacts after debounce while FolderSync runs) and additional schedules. Monitoring remains OFF by default so choosing a pair does not immediately authorize background changes. Enabling monitoring and saving can start the initial automatic comparison; a prior manual baseline is required for approved automatic deletions, not for ordinary initial additions. Conflicts and destructive review rules remain unchanged.

## Optional file versions

Per-task `history_enabled` defaults to **true**, including old JSON/configurations without that field. Turning it OFF skips ordinary pre-overwrite/deletion version copies and discards only the exact reviewed, atomically displaced old bytes after a successful replacement. Existing saved versions are not erased when switching OFF. Export/import and restart retain the preference. Manual restore of an existing version remains possible, but with history OFF no ordinary copy of the overwritten current content is retained; the confirmation says so.

Verified staging, destination checks, delete guards, roots, conflict handling and crash journals remain active. If replacement fails, concurrent unreviewed contents are displaced, or a crash interrupts cleanup, emergency data is preserved and remains recoverable even with history OFF. This exception is stated beside the checkbox; disabling routine file history does not authorize discarding unexpected data.

The editor section is now “File versions & recovery” with a concrete previous-drawing example, retention ON/OFF, actual storage access and the path to restore active-task files (Tasks → Details → Protected versions). Sidebar recovery explains that it lists versions from deleted tasks. Redundant backup-location/retention controls were removed from the deletion section. The requested “Saving never changes files” sentence was removed; folder identity/overlap validation text remains.

## Regression evidence

New engine tests first failed because history OFF was ignored. Tests cover no retained versions after update and proven deletion, preserving old versions through OFF/restart/manual restore, old-config default ON and export/import OFF. Displacement tests cover crash recovery and unreviewed bytes with history OFF. Frontend regression first failed for the draft ID; it checks the unsaved-folder hint, absent Open link, optional-history checkbox and disabled retention controls.

Full `scripts/verify.ps1` completed successfully: **149 Rust tests and 32 frontend tests**, Rust formatting, workspace/all-targets Clippy with warnings denied, TypeScript, ESLint and production frontend build. Raw output: [verification-0.1.6.txt](evidence/verification-0.1.6.txt). Final copy edits also passed TypeScript and ESLint before packaging.

The production NSIS installer was built from source commit `2981947` and installed successfully without elevation. Installed executable version is **0.1.6**. The existing user sign-in preference was preserved; installation did not launch the application or enable task automation. [Installation evidence](evidence/install-0.1.6.json).

- Installer: `target/release/bundle/nsis/FolderSync_0.1.6_x64-setup.exe` (4,123,378 bytes).
- Installer SHA256: `1C864E10F0A5E5EB7177924AFFB5150FD1484119088EF6D2B7B04ADDAEADF4D8`.
- Installed executable SHA256: `F4FEEA7A0741CBE3249821FFFC6723B6E015E2D802D94D123BFC507F2C6BCE91`.

Computer Use launched the installed executable and inspected the task editor. Turning file versions OFF showed the explanation and disabled retention fields; turning it back ON restored the previous controls. Navigating back to Tasks required no discard prompt. The GUI check did not save a changed policy. The OFF engine behavior, restart and recovery were verified separately against disposable filesystem fixtures by the Rust tests. [Installed GUI screenshot](evidence/0.1.6-versions-off.png).

Read-only checks before installation and after the installed GUI interaction matched exactly: two saved tasks, five protected versions, and all disposable source/destination file contents unchanged. Fixture SHA256: `d896f6e3ba2fb12b9729ecd57f29e3a8f983dddda87208d918499e37969f7176`. Evidence: [before](evidence/state-0.1.6-before.json), [after](evidence/state-0.1.6-after.json).

The package remains unsigned. No new network-share or Windows notification-banner claims are made by this change; previously documented OS/network limitations remain applicable.
