import { expect, test } from "vitest";
import {
  actionLabel,
  statusLabel,
  reasonLabel,
  entryLabel,
  formatDate,
  triggerLabel,
} from "./i18n";
import { newTask } from "./types";
test("every operation and result state has a human German label", () => {
  const labels = [
    "Unchanged",
    "CopyToA",
    "CopyToB",
    "DeleteA",
    "DeleteB",
    "MoveToB",
    "Conflict",
  ] as const;
  for (const action of labels)
    expect(actionLabel(action, "de")).not.toBe(action);
  for (const status of [
    "Preview",
    "Review required",
    "Applying",
    "Applied",
    "Attention",
    "Interrupted",
    "Planned",
    "Conflict",
    "Unchanged",
  ]) {
    expect(statusLabel(status, "de")).not.toBe(status);
  }
  expect(statusLabel("Failed: locked C:/fixture/f", "de")).toBe(
    "Fehlgeschlagen: locked C:/fixture/f",
  );
  expect(
    statusLabel("Applied; retention cleanup pending: locked", "de"),
  ).toBe("Angewendet; Bereinigung ausstehend: locked");
});
test("reasons and metadata use the selected language without changing raw paths/digests", () => {
  expect(
    reasonLabel("Explicit conflict choice; Apply is still required", "de"),
  ).toContain("Anwenden");
  expect(entryLabel(null, "de")).toBe("Nicht vorhanden");
  expect(
    entryLabel(
      {
        path: "工程/f",
        size: 0,
        directory: true,
        digest: "directory",
        mtime_ns: "1000000000",
      },
      "de",
    ),
  ).toContain("Ordner");
  expect(formatDate(0, "de")).toBe(new Date(0).toLocaleString("de-DE"));
  const task = newTask();
  task.login = true;
  task.schedule.kind = "weekly";
  task.monitor = true;
  expect(triggerLabel(task, "de")).toContain("Wöchentlich");
  expect(triggerLabel(task, "de")).toContain("Überwacht");
});
