# FolderSync 0.1.13 release verification

Date: 2026-10-17, Windows x64. Release source: signed commit `8004ebf`, signed tag `v0.1.13`. Published at https://github.com/cancel-cloud/FolderSync/releases/tag/v0.1.13.

## Automated checks

- The updater runtime passed `scripts/verify.ps1`: 179 workspace Rust tests, 8 patched notify tests, 42 frontend tests, formatting, Clippy with warnings denied, TypeScript, ESLint and frontend production build. Log: `.tools/verification-0.1.12.txt`. Subsequent changes were a release-only verification test, fixture line-ending protection, version/documentation updates and the installer-page fix.
- Final 0.1.13 native tests: 38 passed, 0 failed; the release-artifact test was intentionally excluded from normal tests and then run explicitly. Log: `.tools/native-tests-0.1.13.txt`.
- Actual 0.1.13 installer signature and authenticated version passed Tauri verification; modified installer bytes were rejected. Command: `cargo test -p foldersync verify_packaged_release_signature_and_modified_bytes -- --ignored --nocapture`, with the manifest/installer environment paths documented in `github-updates.md`.
- Rust release and frontend production builds succeeded. Corrected NSIS bundle completed without warnings, log `.tools/bundle-0.1.13-corrected.txt`. Installer: 5,220,238 bytes (4.98 MiB), SHA256 `47c4307b0a61fd4145a5c810b04d1155880d32266abb11f4ffacea265a794990`.
- Downloaded all four published assets using GitHub CLI into a new temporary directory and compared each hash against the local release: installer, signature, checksum file and `latest.json` matched.

## Actual installed-app workflow through Computer Use

1. Installed a local 0.1.12-rc.1 candidate with the production updater key. Before publication, missing metadata produced an error confined to Version; the app remained usable and did not display a false update offer.
2. After publication, the first checks encountered transient GitHub Not Found responses. A subsequent check fetched the release successfully without configuration or application changes.
3. Verified the compact update button immediately to the right of Settings from the Tasks view. Clicking it opened Settings directly in Version, displaying the upcoming 0.1.13 notes and explicit Install now control.
4. With no synchronization active, clicked Install now in the real app. The app downloaded the published installer, verified it, launched passive current-user installation, exited and relaunched automatically. No additional startup-options page or elevation prompt appeared during this successful update.
5. Windows current-user uninstall registration and actual Version screen both reported 0.1.13. Startup metadata check reported the application current and the sidebar update indicator disappeared.
6. Compared all three saved task configurations and selected preferences against a pre-update SQLite snapshot: unchanged. Existing Windows sign-in registration remained enabled and pointed to the installed executable with `--background`. No task triggers were changed for testing. The previously enabled local test-folder monitor resumed its normal startup reconciliation.
7. Installed executable: 20,388,352 bytes; SHA256 `c054225dc997d23dd221964a16e02676c7b5d952143662f685ac3aaed79475ef`. It matched the release build except for Tauri's three-byte NSIS bundle marker.

Screenshots: [indicator](evidence/0.1.13-update-indicator.jpg), [notes and install](evidence/0.1.13-release-notes-install.jpg), [installed version and current status](evidence/0.1.13-installed-current.jpg). The earlier default-enabled problems-only notification setting was also inspected in the real candidate: [evidence](evidence/0.1.12-problems-only-default.jpg).

## Corrected integration failure and limits

The initial 0.1.12 real update attempt stopped on the custom startup-options page. It was cancelled before replacement, task/preferences preservation was checked, and that release was withdrawn to draft. Its signed tag was preserved. The first proposed installer guard referenced globals before NSIS declared them and emitted warnings; it was replaced with direct argument parsing before 0.1.13 publication. The final passive update above passed.

Automatic checks run at startup and every six hours; the six-hour interval itself was not observed for six hours. Busy-engine refusal, stale requests and signature tampering are tested automatically rather than by disrupting a live synchronization. OS/network errors can delay update discovery; manual retry is available. A successful update restarts the app and retains existing automation choices.

Updater signing is separate from Authenticode. The Windows executable/installer still lack a trusted Windows publisher signature; this work does not resolve or bypass antivirus detections. Versions 0.1.11 and earlier need one manual installation to acquire the updater. Reconciliation performance and SMB acceptance limitations remain separate from this release scope.
