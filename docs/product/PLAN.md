# FolderSync implementation plan

Authoritative specification: `../../CODEX_GOAL.md`. Checkpoint: 2026-10-16.

Purpose: safely synchronize an engineering workspace and a user-accessible share from one Windows session. Rust owns filesystem access, state and execution; React owns presentation. No company paths are configured or tested automatically.

## Execution sequence

1. Audit the repository and reference evidence, preserve the research, initialize Git, verify native compilation and desktop access.
2. Establish a Rust workspace with sync-core, sync-index, sync-watch, sync-scheduler and a Tauri 2 adapter; React/TypeScript/Vite/Tailwind/shadcn frontend. Pin compatible maintained dependencies after reviewing official documentation and licenses. Build and launch an empty shell early.
3. Test root identity/overlap/reparse-point validation, the shared scope matcher, streaming scanner, SQLite migrations/index/baselines and the pure planner. Write destructive-case tests before enabling execution.
4. Implement immutable previews and stale-plan validation, verified staged replacement, protected version storage, operation journal/recovery, and all four modes. Test real disposable files after each subsystem.
5. Add recursive Windows notifications, durable dirty paths, ancestor digest updates, uncertainty reconciliation, cancellation and bounded retries. Add schedules and tray lifetime only after manual correctness passes.
6. Connect task editor/dashboard, operation review, conflict choices, history/restore, logs, settings, import/export and English/German strings. Exercise actual native picker and workflows through Computer Use.
7. Execute formatting, Clippy, Rust and frontend suites, fault tests, a 10,000-file incremental-versus-full benchmark and end-to-end GUI checks. Fix failures before advancing.
8. Build an unsigned x64 NSIS installer, smoke-test installation/uninstallation when permitted, document measured results and remaining limitations, and commit substantive milestones.

## Gates

- Preview, including conflict selection, never mutates user files.
- Missing or partially enumerated roots cannot create deletion evidence.
- Deletion requires a successful common baseline, recoverable storage and explicit destructive review.
- Source removal in Move follows verified destination persistence.
- Baselines advance only after successful operations; conflicts remain pending independently of transfer errors.
- A steady-state single-file event must not enumerate/hash the entire tree.
- No production-ready claim before the specification's core acceptance gate passes.

## Current checkpoint

Implementation steps 1–8 have produced a running Rust/Tauri application, real filesystem tests, Computer Use workflows and an installed release. Development continues against acceptance-audit.md: several explicit management and unattended-deletion requirements remain incomplete, alongside real SMB, tray dispatch/toast activation, accessibility and OS lifecycle QA. These requirements are not waived by the passing core workflows. The initial missing MSVC prerequisite was supplied externally; the original blocked provisioning command was never retried. Actual results supersede the early audit checkpoint and are recorded in qa-report.md. Ordinary history is bounded, reviews use backend summary/detail pages, and English/German progress and cancellation are undergoing installed-build verification.
