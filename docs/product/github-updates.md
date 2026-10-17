# GitHub updates

The native updater checks `https://github.com/cancel-cloud/FolderSync/releases/latest/download/latest.json` at application startup and every six hours, including tray operation. Version has a manual check button. Checking downloads metadata only. A new version adds a compact button immediately to the right of Settings; it opens Version with the upcoming release notes. Notes are displayed as plain text. Checking failures are shown in Version, without notification spam.

Only an explicit Install now request downloads the offered version. The backend pins the expected GitHub repository, version-specific filename and HTTPS URL. Tauri verifies the update with the public key embedded in the app and requires the signature's authenticated version to match the advertised version. Old/equal versions are not offered. The frontend cannot supply arbitrary installer URLs. No frontend updater-plugin permissions are granted; guarded Rust commands own installation.

Downloading does not stop synchronization. Immediately before installer launch the backend pauses new automation and acquires the engine only if idle; an active run is never cancelled for an update. If busy or installation launch fails, previous pause state is restored. Current-user NSIS handles replacement/relaunch. Existing tasks and saved preferences are retained; previously enabled task triggers may resume on startup. Windows antivirus is never disabled or bypassed.

## Release signing and publishing

Updater signing is independent of Windows Authenticode and of Git commit signing. This development machine stores its encrypted updater private key and password outside the repository under `%USERPROFILE%/.codex/keys/FolderSync/`, restricted to the current account. Keep a separate secure backup of `updater.key` and `updater.password`; losing the key prevents future updates for installed clients. Never publish either file. Other build environments must provide `TAURI_SIGNING_PRIVATE_KEY` and `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` securely.

Run `scripts/package.ps1`, then `scripts/update-manifest.ps1 -Version <stable version> -NotesPath <release notes file>`. Publish the EXE, its `.sig`, `latest.json` and checksum file together on a GitHub Release, using the same release notes file for the body and manifest. Publish complete assets before marking the release latest. Manifest generation rejects prerelease versions and signatures without a matching authenticated version. The official Tauri CLI records that version while signing.

Unsigned Windows installers can still trigger Norton or other antivirus products. Updater signatures authenticate our package; they do not establish Windows publisher reputation. Store/MSIX and antivirus vendor review remain tracked separately in TODO.

## Verification plan

Unit tests cover official asset URL pinning, stale/double install requests, busy-engine rejection and pause restoration. A separate non-production signing-key fixture exercises Tauri's actual verifier against genuine bytes, modified bytes and substituted version metadata. Frontend tests cover indicator placement/navigation, plaintext notes and explicit install gating/error display.

For release 0.1.13, install a local 0.1.12-rc.1 candidate carrying the same updater key, publish stable 0.1.13, then exercise the real GitHub download/verification/installation through Computer Use. The candidate is only for this local upgrade test and is not a public release. Older 0.1.11 installations require one manual upgrade because they contain no updater.

References: https://tauri.app/plugin/updater/ and https://docs.rs/tauri-plugin-updater/2.14.0/tauri_plugin_updater/struct.Update.html.

For a packaged release, run the explicit native artifact check before publishing:

```powershell
$env:FOLDERSYNC_RELEASE_MANIFEST = (Resolve-Path target/release/bundle/nsis/latest.json).Path
$env:FOLDERSYNC_RELEASE_INSTALLER = (Resolve-Path target/release/bundle/nsis/FolderSync_0.1.13_x64-setup.exe).Path
cargo test -p foldersync verify_packaged_release_signature_and_modified_bytes -- --ignored --nocapture
```

This release-only test uses the production public key, validates the authenticated version, and confirms changed installer bytes fail verification. It is intentionally excluded from normal tests because source checkouts do not contain packaged installers. It never runs the installer.

The first 0.1.12 real update attempt found that the custom startup-options page did not skip passive/update mode. That installer was cancelled before replacement; the release was withdrawn to draft with its tag preserved. Version 0.1.13 skips that page during passive/in-app updates and preserves the existing sign-in registration. Interactive first installations still offer the checkbox.
