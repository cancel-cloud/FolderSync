# Offene Punkte

- [x] Öffentliches Repository und Git-Remote eingerichtet: https://github.com/cancel-cloud/FolderSync
- [ ] Den Hilfe-Button zusätzlich auf https://github.com/cancel-cloud/FolderSync/blob/main/docs/help/filters.md verlinken. Dieselbe Hilfe bleibt offline in der App verfügbar.
- [ ] Auto-Updater später im Settings-Reiter „Version“ integrieren; vorher Release-Hosting, Signierung, Update-Signaturschlüssel und Rollback-Verhalten festlegen. Aktuell keine automatische Updateprüfung.
- [ ] Tatsächliche Banneranzeige und OS-Anmeldung in einer geeigneten Windows-Sitzung prüfen. Zustellungserlaubnis und expliziter Benachrichtigungston sind implementiert; Banner/„Nicht stören“ werden durch Windows gesteuert.
- [ ] Reale SMB-Übertragung samt Unterbrechung/Wiederverbindung an einer erlaubten, dedizierten Testfreigabe prüfen. Keine Firmenfreigaben verwenden.

## Windows-Verteilung und Antivirus

- [ ] Norton `IDP.ALEXA.54` für Release 0.1.11 untersuchen und den exakten quarantänisierten Dateihash erfassen. Herstellerprüfung für vermutete Fehlalarme bei Norton, SecureAge und Trapmine vorbereiten; keinen bestätigten Fehlalarm behaupten und keine Antivirus-Ausnahmen als Lösung voraussetzen.
- [ ] Vereinfachte portable ZIP zusätzlich prüfen: entfernt den NSIS-Installer, löst aber nicht automatisch die Trapmine-Erkennung von `foldersync.exe`. MSI als alternative, von Tauri unterstützte Installation prüfen; Current-User-Verhalten separat testen.
- [ ] Microsoft-Store-Verteilung als MSIX evaluieren: Microsoft übernimmt die Paketsignierung, ohne eigenes öffentlich vertrauenswürdiges Zertifikat. Publisher-Registrierung, Zertifizierung und zusätzliche Paketierung einplanen; Ordner-/UNC-Zugriff, Tray, Autostart und Datenmigration testen.
- [ ] Direkt verteiltes MSIX benötigt für normale Installation eine vertrauenswürdige Signatur. Selbstsignierte Zertifikate nur für ausdrücklich eingerichtete Tests; unsigned MSIX ist kein geeigneter öffentlicher Distributionsweg. Weder Signatur noch Paketformat garantieren Antivirus-Akzeptanz.

Referenzen: [Microsoft MSIX signing](https://learn.microsoft.com/en-us/windows/msix/package/sign-msix-package-guide), [unsigned MSIX limitations](https://learn.microsoft.com/en-us/windows/msix/package/unsigned-package), [Tauri Windows packaging](https://tauri.app/distribute/windows-installer/).
