# Filterhilfe

Die Hilfe ist im Repository und offline über „Filterhilfe & Beispiele“ verfügbar. Eine öffentliche Repository-URL ist noch nicht hinterlegt; siehe ../../TODO.md.

Ein Muster pro Zeile, relativ zu A und B. Leere Einschlussfelder bedeuten alle Dateien. Die grauen Beispiele sind Platzhalter und werden nicht gespeichert. Groß-/Kleinschreibung wird ignoriert. Ausschlüsse haben Vorrang. Mit „Filter prüfen“ einen relativen Dateipfad testen, beispielsweise Projects/Motor/part.step.

| Feld | Beispiele (jeweils eigene Zeile) | Wirkung |
|---|---|---|
| Dateien einschließen | `*.step`, `*.stp` | Nur STEP-Dateien, auch in Unterordnern |
| Dateien ausschließen | `*.tmp`, `~$*`, `*.bak` | Temporäre Dateien, Office-Sperrdateien und BAK-Dateien überspringen |
| Ordner einschließen | `Projects`, `Drawings` | Dateien unter diesen Ordnerpfaden; Dateien im Stamm bleiben separat steuerbar |
| Ordner ausschließen | `.git`, `node_modules`, `Archive` | Passende Ordnernamen samt Inhalt auf jeder Ebene überspringen |
| Ausgewählte Hauptordner | `Projects`, `Drawings` | Exakte Namen direkt unter A/B; keine Globs. Leer wählt alle. Dateien im Stamm separat ausschließen |

- `*`: beliebige Zeichen, auch Ordnertrenner. `*.step` trifft deshalb auch Projects/a.step.
- `?`: ein Zeichen, z. B. `part-?.step` trifft part-A.step.
- `**/`: beliebige Ordnertiefe, z. B. `**/draft-*.step`.
- `/`: Pfadtrenner; `Projects/Assemblies` ist relativ zum Stammordner.
- Mehrere Zeilen sind Alternativen, keine Befehle. Keine Semikolons oder Anführungszeichen verwenden.

Beispiel CAD-Projekt: Dateien einschließen `*.step` und `*.stp`; Dateien ausschließen `~$*`; Ordner ausschließen `Archive`; „Filter prüfen“ mit Projects/Motor/part.step sollte Eingeschlossen melden, Archive/part.step Ausgeschlossen.

Ausgeschlossene Dateien werden nicht als gelöschte Dateien interpretiert. Umfangsänderungen entfernen daher keine ausgeschlossenen Originale; sie erfordern vor automatischer Löschübernahme einen erneuten manuellen Abgleich. Platzhalter allein ändern keinen Umfang.

Geschützte Dateiinhalte liegen außerhalb von A/B in `%APPDATA%\FolderSync\versions\<Aufgaben-UUID>`. Wiederherstellung über Details → Geschützte Versionen, nicht durch Umbenennen der internen UUID-Dateien. Ältere flache Sicherungen bleiben über den Katalog verfügbar. Die konfigurierte Aufbewahrung begrenzt die Anzahl der Versionen.
