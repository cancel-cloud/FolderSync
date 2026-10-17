Fix horizontal page movement with laptop touchpads. Sideways gestures no longer pan the application page or synchronization status area. Wide file tables remain independently scrollable, and normal vertical scrolling is preserved.

Validation: reproduced the original issue with horizontal wheel input; browser layout regression checks pass at 800, 1100 and 1920 pixels. Four existing unavailable-folder, identity and reconnection safety tests also pass. Synchronization behavior is unchanged in this release.

Windows x64, current-user installation. FolderSync 0.1.13 can offer this update beside Settings; review the notes in Version and select Install now. Versions 0.1.11 and earlier require a manual installation to acquire the updater.

The update package is authenticated with FolderSync's updater signature. The Windows executable and installer remain Authenticode unsigned; antivirus detection limitations remain unresolved.
