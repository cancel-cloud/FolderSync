# FolderSync 0.1.6 — optional file history and clearer task setup

New task IDs are reserved in memory when creating a draft; that never implied persistence. Unsaved tasks now show an automatic-ID placeholder, a draft explanation and no usable backup-folder link. Saved IDs remain stable.

The editor explains manual execution (Save → Preview/Start → review and Apply), monitoring (reacts after debounce while FolderSync runs) and additional schedules. Monitoring remains OFF by default so choosing a pair does not immediately authorize background changes. Enabling monitoring and saving can start the initial automatic comparison; a prior manual baseline is required for approved automatic deletions, not for ordinary initial additions. Conflicts and destructive review rules remain unchanged.

## Optional file versions

Per-task `history_enabled` defaults to **true**, including old JSON/configurations without that field. Turning it OFF skips ordinary pre-overwrite/deletion version copies and discards only the exact reviewed, atomically displaced old bytes after a successful replacement. Existing saved versions are not erased when switching OFF. Export/import and restart retain the preference. Manual restore of an existing version remains possible, but with history OFF no ordinary copy of the overwritten current content is retained; the confirmation says so.

Verified staging, destination checks, delete guards, roots, conflict handling and crash journals remain active. If replacement fails, concurrent unreviewed contents are displaced, or a crash interrupts cleanup, emergency data is preserved and remains recoverable even with history OFF. This exception is stated beside the checkbox; disabling routine file history does not authorize discarding unexpected data.

The editor section is now “File versions & recovery” with a concrete previous-drawing example, retention ON/OFF, actual storage access and the path to restore active-task files (Tasks → Details → Protected versions). Sidebar recovery explains that it lists versions from deleted tasks. Redundant backup-location/retention controls were removed from the deletion section. The requested “Saving never changes files” sentence was removed; folder identity/overlap validation text remains.

## Regression evidence

New engine tests first failed because history OFF was ignored. Tests cover no retained versions after update and proven deletion, preserving old versions through OFF/restart/manual restore, old-config default ON and export/import OFF. Displacement tests cover crash recovery and unreviewed bytes with history OFF. Frontend regression first failed for the draft ID; it checks the unsaved-folder hint, absent Open link, optional-history checkbox and disabled retention controls.

Final verification, installer and installed GUI outcomes are appended after execution.
