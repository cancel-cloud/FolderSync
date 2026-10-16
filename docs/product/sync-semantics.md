# Synchronization semantics

Implemented rules as of 2026-10-16; concrete validation and remaining gaps are listed in qa-report.md.

## State and identity

Relative paths use forward separators and a case-insensitive Windows lookup key while retaining original spelling. Reject path collisions rather than arbitrarily choosing a spelling. A root must be a reachable directory with the same recorded filesystem identity; no missing root or enumeration error is represented by an empty tree. Reparse points are rejected initially, including inside roots; following links requires a future tested policy.

For each relative file, A and B have either a verified content digest or an explicitly observed absence. A common baseline contains the last successfully reconciled digest, or a tombstone after an approved deletion. Modification times and sizes are cache hints; they do not resolve conflicts. Initial and explicit verification scans hash contents. An event invalidates a cached digest even if metadata is unchanged.

## Two-way file truth table

`base` means matching the common baseline; `changed` means a different digest. Absence is only trusted after successful enumeration or a validated dirty-path refresh on reachable, matching roots.

| Baseline | A | B | Proposed action |
|---|---|---|---|
| None | Present | Absent | Copy A→B |
| None | Absent | Present | Copy B→A |
| None | Same content | Same content | Establish common baseline |
| None | Different content | Different content | Concurrent-create conflict; preserve both |
| Present | base | base | No operation |
| Present | changed | base | Verified replacement A→B |
| Present | base | changed | Verified replacement B→A |
| Present | Same changed content | Same changed content | Advance baseline without copy |
| Present | Different changed content | Different changed content | Edit/edit conflict |
| Present | Absent | base | Propose protected deletion at B |
| Present | base | Absent | Propose protected deletion at A |
| Present | Absent | changed | Delete/edit conflict |
| Present | changed | Absent | Edit/delete conflict |
| Present | Absent | Absent | Persist tombstone |
| Tombstone | Present | Absent | New creation A→B |
| Tombstone | Absent | Present | New creation B→A |

Unresolved conflicts never advance the common baseline. Independent nonconflicting operations may succeed. A task may finish transfers with pending conflicts; that is distinct from transfer failure.

Renames initially remain an explicitly reported create plus protected deletion. Ambiguous identity matches never authorize a destructive rename. Directory/file type conflicts preserve both objects; incompatible descendant operations fail safely rather than replacing the type. Deleting a folder requires reviewing its descendants and rechecking that no unplanned content has appeared.

## Directional modes

| Situation | Copy | Mirror | Move |
|---|---|---|---|
| A file, B absent | Copy A→B | Copy A→B | Verify transfer, then remove A with recovery copy |
| Identical A/B | No operation | No operation | Remove A only after explicit Move review and fresh verification |
| Different A/B, first run | Preserve and report conflict | Preserve and report conflict | Preserve and report conflict |
| A changed from a successful pair baseline, B unchanged | Protected replacement | Protected replacement | Verified transfer before source removal |
| B changed unexpectedly | Conflict | Conflict; explicit direction choice required | Conflict |
| B-only file | Retain | Propose protected deletion with explicit review | Retain |
| A deleted after baseline | Retain B | Propose protected deletion | Retain B |

Mirror deletion on first run always requires explicit review. Move always requires destructive confirmation. Manual removals require the checkbox and separate confirmation bound to the exact plan and root identities. Background deletion defaults off. Two-way/mirror tasks may explicitly opt in after a successful full manual reconciliation for the same roots/mode/scope. Only unchanged common-baseline removals within both configured entry and percentage limits then execute automatically; unproven removals and exceeded limits wait for exact-plan review. Limits count files and directories per run and do not accumulate across runs. See automatic-deletion.md for the index-derived denominator, approval persistence and revalidation rules.

## Preview and Apply

Preview may write internal index/run metadata, but creates, changes, deletes or renames no user file or directory. A conflict selection creates a replacement immutable plan revision. It performs no copy. The UI keeps the Preview label and exposes a separate Apply confirmation.

A plan records its run ID, task revision, root identities, scope revision, baseline generation, per-path before/after expectations, reasons and destructive counts by side. Apply rejects stale state before any mutation. Each operation revalidates its immediate source, destination and parents. Full preflight cannot remove races, so executor checks remain mandatory.

## Verified execution and recovery

1. Durably journal operation intent and expected digests.
2. Reject unexpected reparse points and changed roots/parents.
3. Preserve any overwritten/deleted bytes in protected storage outside both roots. Verify and flush the recovery copy before relying on it.
4. Stream into a uniquely created staging file on the destination filesystem, using bounded buffers; hash while copying and honor cancellation.
5. Flush, verify length/content and verify that the source snapshot stayed stable. Recheck the expected destination digest. On Windows, creation never replaces an object which appeared after discovery. ReplaceFileW captures the actual displaced object in a journaled same-volume backup, then preserves it in protected storage even on partial failure. A racing replacement reports failure and retains recovery bytes.
6. Persist completed journal state. Advance only the corresponding successful baseline entries transactionally. A crash after replacement and before baseline commit must be replayed by comparing actual digests, never by blindly repeating destructive steps.
7. Move source removal occurs only after a verified destination, held under a read guard denying writes/deletes through source backup/removal, and a fresh source match. File removal uses a guarded Windows handle, not a separately re-resolved pathname. Cancellation or transfer failure cannot remove the source.

Unfinished staging files are never treated as user content. Startup recovery reads the journal, validates ownership and paths, and reconciles uncertain operations. A successful copy is not a guarantee of cross-filesystem atomicity or power-loss durability on an arbitrary SMB server.

## Scope, history and reset

Exclusions win over includes. Folder exclusion prunes descendants. Selected top-level folders and root-file exclusion apply before file patterns. The same matcher must serve full scans, event refresh, plan review and execution. Changes to scope invalidate plans and require reconciliation; excluded absences do not become tombstones.

Protected storage is outside synchronized namespaces and cannot be selected as a task root. Preserve replaced/deleted files for recovery; selective history controls additional version retention, with a configurable cap. Cleanup never follows links or removes active journal references. Restore is itself a reviewed, stale-checked operation with protection for the current destination.

There is no reset control. Deleting a task configuration requires confirmation and discards its baseline while leaving user files and protected bytes. Recreating it uses conservative first-run rules. A task with pending displaced-file recovery cannot be deleted. No bulk filesystem deletion or baseline reset is exposed.

Root/mode edits require a fresh task. Included/excluded presence is distinguished before planning: hidden/system objects or ancestors on either side suppress operations for that path, rather than masquerading as deletion. Explicit restore inspects current contents independently of sync filters and protects them before restoring.

Directory case spelling is preserved for display, while comparison uses normalized lookup keys. Unicode lowercasing is conservative rather than a complete Windows ordinal case-fold implementation; ambiguous collisions fail. Renames are create plus delete, not an atomic cross-root rename. Mtime is preserved for copied file content; ACL/alternate-stream/directory-time parity is not guaranteed.
