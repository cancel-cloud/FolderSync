# notify 8.2.0 Windows completion patch

Source: unmodified crates.io `notify` 8.2.0 source (CC0-1.0, license included), except `src/windows.rs` and its focused tests. Cargo's root `[patch.crates-io]` applies this copy consistently to the application and dependencies.

The Windows backend previously ignored the completion byte count and silently unwatched some errors. FolderSync relies on explicit uncertainty signals before reducing periodic full scans:

- Successful zero-byte completions and `ERROR_NOTIFY_ENUM_DIR` emit `Flag::Rescan` and re-arm the watch, without parsing an empty buffer.
- Failed read registration and other completion errors emit an error with the affected root. FolderSync then recreates the watcher or uses its five-minute fallback.
- The existing 16 KiB Windows notification buffer remains below the documented SMB 64 KiB limit.

API reference: https://learn.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-readdirectorychangesw

Run the additional upstream backend test with `cargo test --manifest-path vendor/notify/Cargo.toml --lib foldersync_completion_tests`. Retire this patch when a reviewed upstream release provides equivalent signaling. Other upstream source files are retained for provenance and platform compatibility.
