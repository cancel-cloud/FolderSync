# Dependency decisions

Versions are locked by Cargo.lock and pnpm-lock.yaml. Official crate/package documentation and Tauri 2 documentation were checked during setup. The installed lock uses Tauri 2.12.4, notify 8.2.0 stable (9 was a release candidate), rusqlite 0.40.2 with bundled SQLite, BLAKE3 1.8.7, globset 0.4.20, walkdir 2.5, Windows sys 0.61.2, chrono 0.4.45 and UUID 1.28. Tokio is provided through Tauri; engine work is blocking and serialized rather than forcing unnecessary asynchronous file APIs. notify-debouncer-full is unnecessary for the bounded durable coalescer. No USN or service dependency is introduced.

The frontend locks React 19, Vite 7.3.7, TypeScript 5.9, Tailwind 4 and Radix Slot/CVA for the local shadcn-style Button. ESLint 10, typescript-eslint, React hooks lint, Vitest 3 and Testing Library enforce frontend checks. Native dialog, autostart, notification and single-instance plugins are Tauri 2 plugins.

Primary licenses: Rust application MIT; Tauri/notify/chrono generally MIT or Apache-2.0; rusqlite MIT; SQLite public domain; BLAKE3 CC0/Apache-2.0/Apache-2.0 LLVM; React/Vite/Tailwind/Radix/CVA MIT; Windows bindings MIT or Apache-2.0. Lockfile transitive notices should receive a formal distribution review before signed/public release. No proprietary reference assets are shipped.

Windows replacement semantics were checked against [Microsoft ReplaceFileW documentation](https://learn.microsoft.com/windows/win32/api/winbase/nf-winbase-replacefilew), including partial-failure backup behavior. Network watcher constraints follow [ReadDirectoryChangesW documentation](https://learn.microsoft.com/windows/win32/api/winbase/nf-winbase-readdirectorychangesw).
