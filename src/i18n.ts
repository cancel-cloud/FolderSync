import type { Action, Entry, Task, Operation, OperationKind } from "./types";
export const locale = (language: string) =>
  language === "de" ? "de-DE" : "en-GB";
export const formatDate = (seconds: number, language: string) =>
  new Date(seconds * 1000).toLocaleString(locale(language));
export function formatBytes(bytes: number, language: string) {
  const unit = bytes < 1024 ? "B" : bytes < 1048576 ? "KiB" : "MiB";
  const value =
    unit === "B" ? bytes : unit === "KiB" ? bytes / 1024 : bytes / 1048576;
  return `${value.toLocaleString(locale(language), { maximumFractionDigits: unit === "B" ? 0 : 1, minimumFractionDigits: unit === "B" ? 0 : 1 })} ${unit}`;
}
const actions: Record<Action, [string, string]> = {
  Unchanged: ["Unchanged", "Unverändert"],
  CopyToA: ["Copy B → A", "Kopieren B → A"],
  CopyToB: ["Copy A → B", "Kopieren A → B"],
  DeleteA: ["Remove from A", "Aus A entfernen"],
  DeleteB: ["Remove from B", "Aus B entfernen"],
  MoveToB: ["Move A → B", "Verschieben A → B"],
  Conflict: ["Conflict", "Konflikt"],
  Ignore: ["Ignore this run", "Diesen Lauf überspringen"],
};
export const actionLabel = (action: Action, language: string) =>
  actions[action][language === "de" ? 1 : 0];
export function operationLabel(operation: Operation, language: string) {
  const labels: Record<OperationKind, [string, string]> = {
    Create: ["Create file", "Datei erstellen"],
    Update: ["Update file", "Datei aktualisieren"],
    Mkdir: ["Create folder", "Ordner erstellen"],
    DeleteToTrash: ["Protected removal", "Geschützt entfernen"],
    Move: ["Verified move", "Geprüft verschieben"],
    Conflict: ["Conflict", "Konflikt"],
    Ignore: ["Ignore this run", "Diesen Lauf überspringen"],
    Unchanged: ["Unchanged", "Unverändert"],
  };
  return operation.kind
    ? labels[operation.kind][language === "de" ? 1 : 0]
    : actionLabel(operation.action, language);
}
const statuses: Record<string, string> = {
  Preview: "Vorschau",
  "Review required": "Prüfung erforderlich",
  Applying: "Wird angewendet",
  Applied: "Angewendet",
  Attention: "Aufmerksamkeit erforderlich",
  Interrupted: "Unterbrochen",
  Cancelled: "Abgebrochen",
  "Not run": "Nicht ausgeführt",
  Failed: "Fehlgeschlagen",
  Planned: "Geplant",
  Conflict: "Konflikt",
  Ignored: "Übersprungen",
  Unchanged: "Unverändert",
};
export function statusLabel(status: string, language: string): string {
  if (language !== "de") return status;
  if (status.startsWith("Failed:")) return `Fehlgeschlagen:${status.slice(7)}`;
  if (status.startsWith("Rejected:"))
    return `Zurückgewiesen:${status.slice(9)}`;
  const retention = status.indexOf("; retention cleanup pending:");
  if (retention >= 0)
    return `${statusLabel(status.slice(0, retention), language)}; Bereinigung ausstehend:${status.slice(retention + "; retention cleanup pending:".length)}`;
  return statuses[status] || status;
}
const reasons: Record<string, string> = {
  "Rename identity connects divergent paths; originals preserved until explicit review":
    "Eine Umbenennung verbindet unterschiedlich geänderte Pfade. Originale bleiben bis zur ausdrücklichen Prüfung erhalten.",
  "Ambiguous rename candidates share baseline content; originals preserved until explicit review":
    "Mehrdeutige Umbenennungen haben denselben Ausgangsinhalt. Originale bleiben bis zur ausdrücklichen Prüfung erhalten.",
  "Explicit ignore for this run; conflict remains unresolved":
    "Für diesen Lauf ausdrücklich übersprungen; der Konflikt bleibt offen.",
  "Both originals preserved; conflict remains unresolved":
    "Beide Originale bleiben erhalten; der Konflikt bleibt offen.",
  "Both sides differ from a common baseline, or there is no safe baseline":
    "Beide Seiten weichen vom gemeinsamen Ausgangszustand ab, oder es fehlt ein sicherer Ausgangszustand.",
  "Reviewed deletion; a protected version will be kept":
    "Geprüfte Entfernung; eine geschützte Version bleibt erhalten.",
  "Verify destination before removing source":
    "Zieldatei prüfen, bevor die Quelle entfernt wird.",
  "No transfer required": "Keine Übertragung erforderlich.",
  "One-sided creation or change": "Einseitig erstellt oder geändert.",
  "Directory deletion conflicts with changed descendants; originals preserved":
    "Ordnerentfernung kollidiert mit geänderten Inhalten. Originale bleiben erhalten.",
  "Explicit conflict choice; Apply is still required":
    "Ausdrückliche Konfliktauswahl; Anwenden ist weiterhin erforderlich.",
};
export function reasonLabel(reason: string, language: string) {
  return language === "de"
    ? reasons[reason] || `Technische Details: ${reason}`
    : reason;
}
export function deletionReasonLabel(reason: string, language: string) {
  const labels: Record<string, [string, string]> = {
    Approved: [
      "Automatic deletion approved against the common baseline and configured limits.",
      "Automatische Entfernung anhand des gemeinsamen Ausgangszustands und der eingestellten Grenzen freigegeben.",
    ],
    Disabled: [
      "Automatic deletion is off. Review this run before applying.",
      "Automatische Entfernung ist aus. Prüfen Sie diesen Lauf vor dem Anwenden.",
    ],
    BaselineApprovalRequired: [
      "A successful full manual reconciliation is required before automatic deletion.",
      "Vor automatischer Entfernung ist ein erfolgreicher vollständiger manueller Abgleich erforderlich.",
    ],
    UnprovenDeletion: [
      "Some removals have no unchanged common baseline. Manual review is required.",
      "Für einige Entfernungen fehlt ein unveränderter gemeinsamer Ausgangszustand. Manuelle Prüfung erforderlich.",
    ],
    EntryLimit: [
      "The entry limit was exceeded. Review this run before applying.",
      "Die Eintragsgrenze wurde überschritten. Prüfen Sie diesen Lauf vor dem Anwenden.",
    ],
    PercentLimit: [
      "The percentage limit was exceeded. Review this run before applying.",
      "Die Prozentgrenze wurde überschritten. Prüfen Sie diesen Lauf vor dem Anwenden.",
    ],
    MoveRequiresReview: [
      "Move source removals always require manual review.",
      "Quellentfernungen beim Verschieben erfordern immer manuelle Prüfung.",
    ],
  };
  return labels[reason]?.[language === "de" ? 1 : 0] || reason;
}
export function entryLabel(entry: Entry | null, language: string) {
  if (!entry) return language === "de" ? "Nicht vorhanden" : "Absent";
  return `${entry.directory ? (language === "de" ? "Ordner" : "Folder") : formatBytes(entry.size, language)} · ${entry.digest.slice(0, 12)}`;
}
export function modifiedLabel(entry: Entry | null, language: string) {
  if (!entry) return "";
  try {
    return new Date(Number(BigInt(entry.mtime_ns) / 1000000n)).toLocaleString(
      locale(language),
    );
  } catch {
    return "—";
  }
}
export function triggerLabel(task: Task, language: string) {
  const de = language === "de";
  const schedules: Record<string, [string, string]> = {
    periodic: ["Periodic", "Periodisch"],
    daily: ["Daily", "Täglich"],
    weekly: ["Weekly", "Wöchentlich"],
    monthly: ["Monthly", "Monatlich"],
  };
  return (
    [
      task.monitor ? (de ? "Überwacht" : "Monitoring") : null,
      task.schedule.kind
        ? schedules[task.schedule.kind]?.[de ? 1 : 0] || task.schedule.kind
        : null,
      task.login ? (de ? "Beim App-Start" : "At app startup") : null,
    ]
      .filter(Boolean)
      .join(" · ") || (de ? "Manuell" : "Manual")
  );
}
export function messageLabel(message: string, language: string) {
  if (language !== "de") return message;
  const messages: Record<string, string> = {
    "Choose a copy direction or Ignore for every conflict before applying this task":
      "Vor dem Anwenden für jeden Konflikt eine Kopierrichtung oder Überspringen wählen.",
    "Task has newly discovered conflicts; preview again":
      "Die Aufgabe hat neu erkannte Konflikte. Erneut prüfen.",
    "Run cancelled. Completed operations remain committed. Create a fresh preview to continue.":
      "Lauf abgebrochen. Bereits abgeschlossene Aktionen bleiben erhalten. Erstellen Sie zum Fortsetzen eine neue Vorschau.",
    "Task configuration exported.": "Aufgabenkonfiguration exportiert.",
    "Run finished. See the actual result and protected versions.":
      "Lauf abgeschlossen. Prüfen Sie das Ergebnis und die geschützten Versionen.",
    "Protected version restored. Preview the task to reconcile this change.":
      "Geschützte Version wiederhergestellt. Prüfen Sie die Aufgabe erneut, um diese Änderung abzugleichen.",
  };
  const imported = message.match(
    /^Imported (\d+) tasks\. All automatic triggers are disabled\.$/,
  );
  if (imported)
    return `${imported[1]} Aufgaben importiert. Alle automatischen Auslöser sind deaktiviert.`;
  const previews = message.match(
    /^(\d+) previews saved\. Open Details to review each before applying\.$/,
  );
  if (previews)
    return `${previews[1]} Vorschauen gespeichert. Öffnen Sie Details, um jede vor dem Anwenden zu prüfen.`;
  return messages[message] || statusLabel(message, language);
}
