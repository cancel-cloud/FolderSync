# FolderSync 0.1.9 — event collection and controlled reconciliation

Source commits: `d778a7a` (Windows event collection, recovery and interval settings), `9cbe229` (equal-observation optimization, batched incremental index writes and ordered child queries). Existing company folders and shares were not used.

## Behavior

- Windows notification callbacks coalesce changes independently of the engine into bounded unique paths. A busy copy no longer fills an application queue after 2,048 raw events. Limits remain explicit: 65,536 paths, 65,536 rename hints and 16 MiB of buffered path/hint strings; exceeded limits request reconciliation instead of dropping uncertainty.
- Dirty paths, rename hints and event counts are persisted together before processing. New callbacks remain queued for the next batch while the current run executes. Failed enqueue preserves a recovery request. Restart still reconciles monitored tasks because the memory collector cannot reconstruct changes during process downtime.
- Settings → General → Safety reconciliation defaults to Daily. Hourly, every 6 hours, Weekly and Off are available. Off disables only the periodic check for healthy monitoring. Existing explicit task schedules are unaffected. Failed monitoring retains a five-minute fallback and attempts to reconnect every 30 seconds. Roots are probed for availability/identity every 30 seconds without recursively scanning the tree; the engine independently verifies configured identities before applying work.
- A documented local patch to maintained `notify` 8.2 reports successful zero-byte completions, `ERROR_NOTIFY_ENUM_DIR`, read-registration failures and terminal completion errors to the consumer. This is required to reduce reliance on periodic scans. The native Windows buffer remains 16 KiB. See [patch provenance](../../vendor/notify/FOLDERSYNC-PATCH.md) and [Microsoft's API contract](https://learn.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-readdirectorychangesw).
- A fresh automatic all-Unchanged plan advances the observed common baseline without hashing both sides a second time. Manual plans, mixed/mutating plans, task/generation guards, root checks, deletion approval, conflict handling and staged verified copies retain their safeguards. An edit after an equal observation remains pending for a subsequent event/recovery comparison; no synchronization root is changed by the equal-only fast path.
- Incremental observations are stored in one transaction after all requested reads succeed. A locked/unreadable later path cannot leave the earlier paths partially committed. The existing display-path map avoids a linear search through all queued paths for every new file. An additional `(task, side, parent, path)` index prevents ordered child queries from scanning a whole task for each directory. Existing state opens and receives that additive index without resetting baselines or task configuration.
- Own completed writes are still checked when reported. Internal staging paths are excluded, but external changes are never suppressed by a blanket “ignore my writes” time window. A large initial copy can therefore have a subsequent verification batch; steady-state individual edits do not require a full-tree comparison.

## Actual verification

`cargo fmt --all -- --check`, all-target/workspace Clippy with `-D warnings`, final `cargo test --workspace` (**170 passed**), the patched library's complete unit suite (**8 passed**), TypeScript, ESLint, frontend tests (**33 passed**) and production web build passed. The full frontend checks were followed by a Rust-only batch/index change; final formatting, Clippy and the entire workspace Rust suite were rerun after it. [Full verification](evidence/verification-0.1.9-full.txt), [final Rust rerun](evidence/verification-0.1.9-rust-final.txt).

Observed regression failures before fixes:

- Native repeated-save burst: old collector stopped at exactly 2,048 events and reported uncertainty. It now coalesces the burst while the consumer does not poll, then receives a later event on the other root.
- Automatic equal content: 88 hashed bytes instead of the required 44. It now hashes each side once.
- Ordered child query: 50,413 SQL VM steps for 100 children among 10,000 entries. With the added index the test completes below 2,000 steps. [RED](evidence/0.1.9-index-red.txt), [GREEN](evidence/0.1.9-index-green.txt).
- Failed incremental discovery under an exclusive Windows file lock committed earlier observations. It now preserves the previous index batch. [RED](evidence/0.1.9-batch-red.txt), [GREEN](evidence/0.1.9-batch-green.txt).

Additional tests cover 106,050 injected events coalescing to 35,350 paths, collector count/byte limits, concurrent producer/drains without lost events, backend errors and rescan signals, rename ambiguity, unavailable/replaced roots, interval validation/fallback, post-observation edits, stale manual equal plans and the global validation barrier for automatic mutating plans. Existing conflict, deletion, copy/move, crash, cancellation, retention, restart and queue tests remain passing. Independent read-only review of the changes, including the final transaction adjustment, reported no actionable findings.

## Reproducible benchmark

`cargo run --release -p foldersync-engine --example profile_incremental` generates a unique disposable `.tools/FolderSync-Incremental-*` tree and state, 35,000 mixed-size files plus 350 directories per side, **332,640,000 bytes per side**. It never opens the installed application's database or applies against the user's existing test destination. A harness-owned fixture can be reused by passing its path; canonical path guards and content checks reject unrelated/changed input. Each run creates a fresh state directory.

Final source, release build, Windows C: SATA SSD, with OS cache/load variability and other verification work running during measurement:

| Work | Wall time | Entries examined | Bytes hashed | Copied bytes |
|---|---:|---:|---:|---:|
| Manual Preview + Apply, equal files, fresh baseline | 42.861 s | 141,400 | 1,330,560,000 | 0 |
| Automatic full equal reconciliation, established baseline | 42.008 s | 70,700 | 665,280,000 | 0 |
| Native single-file event through verified Apply, excluding debounce | 337 ms | 8 | 918 | 153 |

The incremental operation count is one, with zero full reconciliations, and its destination content was checked. Copied bytes include the 128-byte protected old version plus the 25-byte replacement. The initial/default 20-second debounce is additional. Windows process peak working set observed: **194,940,928 bytes (185.9 MiB)**. [Raw metrics](benchmarks/incremental-0.1.9.json), [memory observation](benchmarks/incremental-0.1.9-memory.json).

The full runs use different baseline states and are not a controlled wall-time speedup comparison. Halved read counts are measured; full scans still take tens of seconds. An initial pre-index-fix benchmark was deliberately interrupted after identifying the unfavorable child-query plan; it is not reported as a completed benchmark. A prior completed candidate measured 60.932 s full automatic / 347 ms incremental, illustrating timing variability. These measurements do not establish SSD health, cold-cache throughput, SMB performance or large-copy throughput.

## Limitations

Real SMB disconnect/reconnect is still unverified: the earlier dedicated-share setup was denied to this account. This release adds error/root recovery logic and local unavailable/substituted-root tests, not a claim of tested SMB reconnection. Servers that silently fail to deliver notifications may require more frequent safety reconciliation; turning that periodic check off accepts delayed detection until another recovery/manual check. No USN journal or OS service was added. Initial scans, explicit full comparisons and a large batch of own-write checks remain substantial work. The notify patch must be reassessed when upgrading its upstream dependency. No Windows notification/privacy/security setting was changed.


## Package, installation and actual GUI

The final Windows x64 NSIS build from source commit `9cbe229` succeeded. The current-user upgrade installed without elevation, exit 0, and did not launch the app by itself. Existing sign-in registration remained enabled, matching the previous preference. The app was then launched and inspected with Computer Use.

- Installer: `C:\Users\Lukas\Developer\SyncFolders\target\release\bundle\nsis\FolderSync_0.1.9_x64-setup.exe` (**4,146,519 bytes**, unsigned).
- Installer SHA256: `CFEA999339A900B2A930045580A3D0D321C626930DC7A3758B21B39B4947C633`.
- Installed executable SHA256: `B07B387E789BE76FB2F0A0D00EC48D225A60665D9B15A9444AA68F09D72F3273`.
- The installed executable and release build have identical size (16,860,160 bytes); the only difference is Tauri's three-byte bundle marker `UNK` → `NSS`. This confirms the installed payload matches the latest compiled code. [Byte comparison](evidence/0.1.9-bundle-difference.json).
- [Build log](evidence/package-0.1.9.txt), [installation checks](evidence/install-0.1.9.json), [GUI and preservation checks](evidence/gui-0.1.9.json).

Computer Use verified the actual Version tab shows 0.1.9; Settings displayed the daily default, saved six hours through the real native command (live SQLite value `360`), then restored daily (`1440`). The existing two saved task JSON records, all five protected file payloads and prior selected preferences matched the preservation snapshot after installation and GUI checks. The additional child index exists in the live database.

An actual Preview of the existing dedicated `.tools/FolderSync-GUI` fixture completed with 315 unchanged entries and one conflict. Apply stayed disabled pending conflict choices. The operation table showed both sides' conflict metadata, and toggling Show unchanged displayed the unchanged entries' metadata. No Apply, conflict choice or deletion approval was invoked. The application remains open on Tasks with no active run.

[Screenshot: settings](evidence/0.1.9-reconciliation-settings.png), [installed version](evidence/0.1.9-installed-version.png), [conflict preview](evidence/0.1.9-preview-conflict.png), [unchanged metadata](evidence/0.1.9-unchanged-metadata.png).

For installation elsewhere, run the setup executable normally under the intended account, then start FolderSync from its shortcut. Settings → General → Safety reconciliation controls the additional interval. Leave Daily unless your workload requires more frequent checking. The installer and Settings retain the optional current-user sign-in startup in the tray with saved configuration. Installation does not enable task monitoring, schedules or destructive policies. Previously enabled triggers can run when the application starts. An actual Windows sign-in was not performed during this release check.
