FolderSync now checks GitHub Releases for updates at startup and every six hours. When an update is available, a small download button appears beside Settings. Click it to open Version, review the release notes and choose **Install now**.

Updates are downloaded only on request and verified using a pinned updater signing key with version-bound signatures before installation. Installation waits for synchronization to be idle. Network/check/download/verification errors leave the current application in place.

This release also enables **Only notify about failures or problems** by default, suppressing successful-sync notifications while retaining conflict, failure and review alerts.

Windows x64, current-user installation. The Windows installer remains unsigned by Authenticode; updater signatures are separate and do not bypass antivirus. Existing antivirus detections remain under investigation. Versions 0.1.11 and earlier need one manual installation to gain this updater.

Passive updates now preserve the existing sign-in preference without displaying the startup-options wizard. The 0.1.12 release was withdrawn after this integration issue was found during the real update test.
