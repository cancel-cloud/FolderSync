// @vitest-environment jsdom
import { afterEach, beforeAll, expect, test, vi } from "vitest";
import { cleanup, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import App from "./App";
import { newTask } from "./types";
const api = vi.hoisted(() => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/core", () => ({ invoke: api.invoke }));
vi.mock("@tauri-apps/api/event", () => ({ listen: async () => () => {} }));
vi.mock("@tauri-apps/plugin-dialog", () => ({ open: vi.fn(), save: vi.fn() }));
vi.mock("@tauri-apps/plugin-autostart", () => ({
  isEnabled: async () => false,
  enable: vi.fn(),
  disable: vi.fn(),
}));
beforeAll(() => {
  HTMLDialogElement.prototype.showModal = function () {
    this.open = true;
  };
  HTMLDialogElement.prototype.close = function () {
    this.open = false;
  };
});
afterEach(() => {
  cleanup();
  vi.clearAllMocks();
});
test("nested conflict choice only revises preview; Apply requires its separate confirmation", async () => {
  const task = {
    ...newTask("TwoWay"),
    name: "Fixture pair",
    a: "C:/fixture/A",
    b: "C:/fixture/B",
  };
  const entry = {
    path: "f",
    size: 1,
    digest: "a",
    directory: false,
    mtime_ns: "1",
    file_id: null,
  };
  const plan = {
    id: "preview",
    task,
    root_a: "A",
    root_b: "B",
    baseline_generation: 0,
    created_at: 1,
    status: "Preview",
    metrics: {
      scanned_entries: 2,
      hashed_bytes: 2,
      copied_bytes: 0,
      queued_paths: 0,
      reconciliations: 1,
      watch_events: 0,
    },
    operations: [
      {
        path: "f",
        action: "Conflict",
        status: "Planned",
        reason: "Both differ",
        a: entry,
        b: { ...entry, digest: "b" },
      },
    ],
  };
  api.invoke.mockImplementation(async (command: string) => {
    switch (command) {
      case "tasks":
        return [task];
      case "statuses":
        return [];
      case "storage":
        return "state";
      case "preview":
        return plan;
      case "resolve":
        return {
          ...plan,
          id: "revised",
          operations: [
            {
              ...plan.operations[0],
              action: "CopyToB",
              reason: "Explicit conflict choice",
            },
          ],
        };
      case "apply":
        return { ...plan, status: "Applied" };
      default:
        return null;
    }
  });
  const user = userEvent.setup();
  render(<App />);
  await user.click(
    await screen.findByRole("button", { name: "Preview / Run" }),
  );
  await user.selectOptions(
    await screen.findByRole("combobox", { name: "Resolve f" }),
    "a",
  );
  await waitFor(() =>
    expect(api.invoke).toHaveBeenCalledWith("resolve", {
      id: "preview",
      path: "f",
      choice: "a",
    }),
  );
  expect(api.invoke.mock.calls.some(([c]) => c === "apply")).toBe(false);
  await user.click(screen.getByRole("button", { name: "Apply reviewed plan" }));
  expect(api.invoke.mock.calls.some(([c]) => c === "apply")).toBe(false);
  await user.click(screen.getByRole("button", { name: "Cancel" }));
  expect(api.invoke.mock.calls.some(([c]) => c === "apply")).toBe(false);
  await user.click(screen.getByRole("button", { name: "Apply reviewed plan" }));
  await user.click(screen.getByRole("button", { name: "Confirm" }));
  await waitFor(() =>
    expect(api.invoke).toHaveBeenCalledWith("apply", {
      id: "revised",
      destructive: true,
    }),
  );
});

test("saved German preference localizes dashboard, conflict review and metadata", async () => {
  const task = {
    ...newTask(),
    name: "Deutsches Prüfpaar",
    a: "C:/fixture/A",
    b: "C:/fixture/B",
    schedule: { ...newTask().schedule, kind: "weekly" },
  };
  const entry = {
    path: "f",
    size: 1,
    digest: "hash-a",
    directory: false,
    mtime_ns: "1000000000",
  };
  const plan = {
    id: "localized",
    task,
    created_at: 0,
    status: "Attention",
    metrics: {
      scanned_entries: 2,
      hashed_bytes: 2,
      copied_bytes: 0,
      queued_paths: 0,
      reconciliations: 1,
      watch_events: 0,
    },
    operations: [
      {
        path: "f",
        action: "Conflict",
        status: "Conflict",
        reason:
          "Both sides differ from a common baseline, or there is no safe baseline",
        a: entry,
        b: { ...entry, digest: "hash-b" },
      },
    ],
  };
  api.invoke.mockImplementation(
    async (command: string, args?: { key?: string }) => {
      switch (command) {
        case "tasks":
          return [task];
        case "statuses":
          return [
            {
              task_id: task.id,
              last_run: plan,
              pending_conflicts: 1,
              next_run: null,
            },
          ];
        case "setting":
          return args?.key === "language" ? "de" : null;
        case "storage":
          return "state";
        case "preview":
          return { ...plan, status: "Preview" };
        default:
          return null;
      }
    },
  );
  const user = userEvent.setup();
  render(<App />);
  expect(await screen.findByText("Aufmerksamkeit erforderlich")).toBeTruthy();
  expect(screen.getByText("Wöchentlich")).toBeTruthy();
  await user.click(screen.getByRole("button", { name: "Vorschau / Start" }));
  expect(
    await screen.findByRole("combobox", { name: "f auflösen" }),
  ).toBeTruthy();
  expect(screen.getByText("Konflikt")).toBeTruthy();
  expect(screen.queryByText("Conflict")).toBeNull();
  expect(
    screen.getByText(/Beide Seiten weichen vom gemeinsamen Ausgangszustand/),
  ).toBeTruthy();
  expect(screen.getAllByText(/Geändert:/)).toHaveLength(2);
  expect(screen.getByRole("columnheader", { name: "Ordner A" })).toBeTruthy();
  expect(screen.getByText(/Entfernungen aus A/)).toBeTruthy();
});
