import { useEffect, useRef } from "react";
import { Button } from "./ui/button";

export const filterExamples = {
  include: "*.step\n*.stp",
  exclude: "*.tmp\n~$*",
  folder_include: "Projects\nDrawings",
  folder_exclude: ".git\nnode_modules\nArchive",
  selected_folders: "Projects\nDrawings",
};

export function FilterHelp({
  lang,
  onClose,
}: {
  lang: string;
  onClose: () => void;
}) {
  const dialog = useRef<HTMLDialogElement>(null);
  useEffect(() => {
    dialog.current?.showModal();
  }, []);
  const tr = (en: string, de: string) => (lang === "de" ? de : en);
  return (
    <dialog
      ref={dialog}
      className="modal filter-help"
      aria-labelledby="filter-help-title"
      onCancel={(event) => {
        event.preventDefault();
        onClose();
      }}
    >
      <h2 id="filter-help-title">
        {tr("Filter help & examples", "Filterhilfe & Beispiele")}
      </h2>
      <p>
        {tr(
          "One pattern per line, relative to each root. Examples in empty fields are hints, not active filters. Exclusions always win. Matching ignores case. Use / for subfolders.",
          "Ein Muster pro Zeile, relativ zu jedem Stammordner. Beispiele in leeren Feldern sind Hinweise, keine aktiven Filter. Ausschlüsse haben Vorrang. Groß-/Kleinschreibung spielt keine Rolle. Unterordner mit / trennen.",
        )}
      </p>
      <table>
        <thead>
          <tr>
            <th>{tr("Field", "Feld")}</th>
            <th>{tr("Example", "Beispiel")}</th>
            <th>{tr("Effect", "Wirkung")}</th>
          </tr>
        </thead>
        <tbody>
          <tr>
            <td>{tr("Include files", "Dateien einschließen")}</td>
            <td>
              <code>
                *.step
                <br />
                *.stp
              </code>
            </td>
            <td>
              {tr(
                "Only STEP files, including subfolders. Empty includes all files.",
                "Nur STEP-Dateien, auch in Unterordnern. Leer schließt alle Dateien ein.",
              )}
            </td>
          </tr>
          <tr>
            <td>{tr("Exclude files", "Dateien ausschließen")}</td>
            <td>
              <code>
                *.tmp
                <br />
                ~$*
              </code>
            </td>
            <td>
              {tr(
                "Skip temporary files and Office lock files.",
                "Temporäre Dateien und Office-Sperrdateien überspringen.",
              )}
            </td>
          </tr>
          <tr>
            <td>{tr("Include folders", "Ordner einschließen")}</td>
            <td>
              <code>
                Projects
                <br />
                Drawings
              </code>
            </td>
            <td>
              {tr(
                "Files below these folder paths. Root files stay included unless excluded separately.",
                "Dateien unter diesen Ordnerpfaden. Dateien im Stamm bleiben enthalten, sofern nicht separat ausgeschlossen.",
              )}
            </td>
          </tr>
          <tr>
            <td>{tr("Exclude folders", "Ordner ausschließen")}</td>
            <td>
              <code>
                .git
                <br />
                node_modules
                <br />
                Archive
              </code>
            </td>
            <td>
              {tr(
                "Skip matching folder names and their complete contents, at any depth.",
                "Passende Ordnernamen samt vollständigem Inhalt auf jeder Ebene überspringen.",
              )}
            </td>
          </tr>
          <tr>
            <td>
              {tr("Selected top-level folders", "Ausgewählte Hauptordner")}
            </td>
            <td>
              <code>
                Projects
                <br />
                Drawings
              </code>
            </td>
            <td>
              {tr(
                "Exact folder names directly below A/B; no glob patterns. Empty selects all. Root files are controlled separately.",
                "Exakte Ordnernamen direkt unter A/B; keine Glob-Muster. Leer wählt alle. Dateien im Stamm werden separat gesteuert.",
              )}
            </td>
          </tr>
        </tbody>
      </table>
      <p>
        {tr(
          "* matches any characters, including folder separators; ? matches one character; **/ can match any folder depth. Patterns are alternatives, not a sequence of commands. Use Test filters with a real relative path before saving. Changing filters never deletes excluded files.",
          "* steht für beliebige Zeichen, auch Ordnertrenner; ? für ein Zeichen; **/ für beliebige Ordnertiefe. Muster sind Alternativen, keine Befehlsfolge. Vor dem Speichern mit „Filter prüfen“ einen echten relativen Pfad testen. Umfangsänderungen löschen keine ausgeschlossenen Dateien.",
        )}
      </p>
      <small>
        {tr(
          "Repository documentation: docs/help/filters.md. The online repository URL has not been configured yet (TODO.md).",
          "Repository-Dokumentation: docs/help/filters.md. Die Online-Repository-URL ist noch nicht hinterlegt (TODO.md).",
        )}
      </small>
      <div className="toolbar">
        <Button onClick={onClose}>
          {tr("Back to task", "Zurück zur Aufgabe")}
        </Button>
      </div>
    </dialog>
  );
}
