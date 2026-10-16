// @vitest-environment jsdom
import { afterEach, beforeAll, expect, test, vi } from "vitest";
import { cleanup, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import App from "./App";
import { newTask } from "./types";
import type { Plan, Operation } from "./types";
import { summarize, directions } from "./review";
type FixturePlan = Omit<Plan, "summary"> & {
  operations: Operation[];
  root_a?: string;
  root_b?: string;
  baseline_generation?: number;
};
function header(p: FixturePlan): Plan {
  const d = directions(p.operations);
  return {
    id: p.id,
    task: p.task,
    created_at: p.created_at,
    status: p.status,
    metrics: p.metrics,
    summary: {
      ...summarize(p.operations),
      entries: p.operations.length,
      to_a: d.toA,
      to_b: d.toB,
      remove_a: d.removeA,
      remove_b: d.removeB,
      explicit: p.operations.some((o) => o.reason.startsWith("Explicit")),
    },
  };
}
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
  const plan: FixturePlan = {
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
  let current = plan;
  api.invoke.mockImplementation(async (command: string) => {
    switch (command) {
      case "tasks":
        return [task];
      case "statuses":
        return [];
      case "storage":
        return "state";
      case "preview":
        return header(plan);
      case "operation_page":
        return {
          operations: current.operations,
          total: current.operations.length,
        };
      case "resolve":
        current = {
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
        return header(current);
      case "apply":
        return header({ ...current, status: "Applied" });
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
  const plan: FixturePlan = {
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
              last_run: header(plan),
              pending_conflicts: 1,
              next_run: null,
            },
          ];
        case "setting":
          return args?.key === "language" ? "de" : null;
        case "storage":
          return "state";
        case "preview":
          return header({ ...plan, status: "Preview" });
        case "operation_page":
          return { operations: plan.operations, total: plan.operations.length };
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

test("review requests bounded pages and resets paging when filters change", async () => {
  const task = { ...newTask(), name: "Large review", a: "A", b: "B" };
  const plan: Plan = {
    id: "large",
    task,
    created_at: 1,
    status: "Preview",
    metrics: {
      scanned_entries: 405,
      hashed_bytes: 0,
      copied_bytes: 0,
      queued_paths: 0,
      reconciliations: 1,
      watch_events: 0,
    },
    summary: {
      entries: 405,
      copies: 405,
      deletes: 0,
      conflicts: 0,
      unchanged: 0,
      to_a: 0,
      to_b: 405,
      remove_a: 0,
      remove_b: 0,
      explicit: false,
    },
  };
  api.invoke.mockImplementation(
    async (
      command: string,
      args?: { query?: { offset: number; limit: number; filter: string } },
    ) => {
      if (command === "tasks") return [task];
      if (command === "statuses") return [];
      if (command === "preview") return plan;
      if (command === "operation_page") {
        const q = args!.query!;
        const count = q.filter ? 1 : Math.min(200, 405 - q.offset);
        return {
          total: q.filter ? 1 : 405,
          operations: Array.from({ length: count }, (_, i) => ({
            path: q.filter || `row-${q.offset + i}`,
            action: "CopyToB",
            reason: "One-sided creation or change",
            status: "Planned",
            a: null,
            b: null,
          })),
        };
      }
      return null;
    },
  );
  const user = userEvent.setup();
  render(<App />);
  await user.click(
    await screen.findByRole("button", { name: "Preview / Run" }),
  );
  expect(await screen.findByText("row-0")).toBeTruthy();
  expect(screen.queryByText("row-200")).toBeNull();
  await user.click(screen.getByRole("button", { name: "Next" }));
  expect(await screen.findByText("row-200")).toBeTruthy();
  expect(screen.queryByText("row-0")).toBeNull();
  await user.type(
    screen.getByRole("textbox", { name: "Filter operations" }),
    "needle",
  );
  expect(await screen.findByText("needle")).toBeTruthy();
  await waitFor(() =>
    expect(api.invoke).toHaveBeenLastCalledWith("operation_page", {
      id: "large",
      query: {
        offset: 0,
        limit: 200,
        filter: "needle",
        show_unchanged: false,
        sort: "path",
      },
    }),
  );
  expect(
    (screen.getByRole("button", { name: "Next" }) as HTMLButtonElement)
      .disabled,
  ).toBe(true);
});
