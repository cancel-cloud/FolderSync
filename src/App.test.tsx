// @vitest-environment jsdom
import { afterEach, beforeAll, expect, test, vi } from "vitest";
import { act, cleanup, render, screen, waitFor } from "@testing-library/react";
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
const api = vi.hoisted(() => ({
  invoke: vi.fn(),
  listeners: new Map<string, (event: { payload: unknown }) => void>(),
}));
vi.mock("@tauri-apps/api/core", () => ({ invoke: api.invoke }));
vi.mock("@tauri-apps/api/event", () => ({
  listen: async (
    name: string,
    listener: (event: { payload: unknown }) => void,
  ) => {
    api.listeners.set(name, listener);
    return () => {
      api.listeners.delete(name);
    };
  },
}));
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
  api.listeners.clear();
});
test("deleted-task recovery stays separate from synchronization and restore requires confirmation", async () => {
  const task = {
    ...newTask(),
    name: "Deleted pair",
    a: "C:/owned/A",
    b: "C:/owned/B",
  };
  const version = {
    id: "version",
    task_id: task.id,
    side: "B",
    path: "recover.txt",
    digest: "old",
    stored: "state/versions/version",
    created_at: 1,
  };
  let applied = false;
  const oldPlan = header({
    id: "archived-plan",
    task,
    created_at: 1,
    status: "Preview",
    operations: [],
    metrics: {
      scanned_entries: 0,
      hashed_bytes: 0,
      copied_bytes: 0,
      queued_paths: 0,
      reconciliations: 0,
      watch_events: 0,
    },
  });
  api.invoke.mockImplementation(async (command: string) => {
    if (command === "tasks" || command === "statuses") return [];
    if (command === "history") return [oldPlan];
    if (command === "operation_page") return { total: 0, operations: [] };
    if (command === "recovery_tasks")
      return [{ task, versions: 1, archived_at: 2 }];
    if (command === "versions") return [version];
    if (command === "restore_preview")
      return {
        id: "review",
        version,
        current: null,
        archived: true,
        target_root: task.b,
      };
    if (command === "restore_apply") applied = true;
    return null;
  });
  const user = userEvent.setup();
  render(<App />);
  await user.click(await screen.findByRole("button", { name: "Recovery" }));
  await user.click(
    await screen.findByRole("button", {
      name: "Open recovery for Deleted pair",
    }),
  );
  expect(screen.queryByRole("button", { name: "New preview" })).toBeNull();
  expect(screen.queryByRole("button", { name: "Delete task" })).toBeNull();
  await user.click(screen.getByRole("button", { name: "Review restore" }));
  const dialog = await screen.findByRole("dialog", {
    name: "Restore protected version?",
  });
  expect(dialog.textContent).toContain("C:/owned/B/recover.txt");
  expect(applied).toBe(false);
  await user.click(screen.getByRole("button", { name: "Confirm" }));
  await waitFor(() => expect(applied).toBe(true));
  await user.click(screen.getByRole("button", { name: /archived-plan/ }));
  expect(
    screen.queryByRole("button", { name: "Apply reviewed plan" }),
  ).toBeNull();
  expect(screen.queryByRole("button", { name: "Fresh preview" })).toBeNull();
});
test("enabling automatic deletion requires a separate policy confirmation before saving", async () => {
  let saved = { ...newTask(), name: "Approved baseline", a: "A", b: "B" };
  api.invoke.mockImplementation(
    async (command: string, args?: { task: typeof saved }) => {
      if (command === "tasks") return [saved];
      if (command === "statuses")
        return [
          {
            task_id: saved.id,
            last_run: null,
            pending_conflicts: 0,
            next_run: null,
            deletion_baseline_run: "manual-proof",
          },
        ];
      if (command === "save_task" && args) saved = args.task;
      return null;
    },
  );
  const user = userEvent.setup();
  render(<App />);
  await user.click(await screen.findByRole("button", { name: "Configure" }));
  await user.click(
    screen.getByRole("checkbox", {
      name: "Allow proven baseline deletions automatically",
    }),
  );
  await user.click(screen.getByRole("button", { name: "Save task" }));
  expect(
    await screen.findByRole("dialog", { name: "Enable automatic deletion?" }),
  ).toBeTruthy();
  expect(saved.deletion_policy.enabled).toBe(false);
  await user.click(screen.getByRole("button", { name: "Confirm" }));
  await waitFor(() => expect(saved.deletion_policy.enabled).toBe(true));
});
test("group filtering clears hidden selections and bulk reassignment refreshes collapsible groups", async () => {
  let tasks = [
    { ...newTask(), name: "Mechanical", group: "Engineering", a: "A", b: "B" },
    { ...newTask(), name: "Archive", group: "Archive", a: "C", b: "D" },
  ];
  api.invoke.mockImplementation(
    async (command: string, args?: { ids: string[]; group: string }) => {
      if (command === "tasks") return tasks;
      if (command === "statuses") return [];
      if (command === "assign_group") {
        tasks = tasks.map((t) =>
          args?.ids.includes(t.id) ? { ...t, group: args.group } : t,
        );
      }
      return null;
    },
  );
  const user = userEvent.setup();
  render(<App />);
  await screen.findByRole("heading", { name: "Mechanical" });
  await user.click(screen.getByRole("checkbox", { name: "Select Archive" }));
  await user.selectOptions(
    screen.getByRole("combobox", { name: "Filter by group" }),
    "group:Engineering",
  );
  expect(screen.queryByRole("heading", { name: "Archive" })).toBeNull();
  expect(
    (screen.getByRole("button", { name: "Assign group" }) as HTMLButtonElement)
      .disabled,
  ).toBe(true);
  await user.click(
    screen.getByRole("checkbox", { name: "Select all visible tasks" }),
  );
  await user.type(
    screen.getByRole("textbox", { name: "Group for selected tasks" }),
    "Review",
  );
  await user.click(screen.getByRole("button", { name: "Assign group" }));
  await waitFor(() => expect(tasks[0].group).toBe("Review"));
  expect(tasks[1].group).toBe("Archive");
  await waitFor(() =>
    expect(
      (
        screen.getByRole("combobox", {
          name: "Filter by group",
        }) as HTMLSelectElement
      ).value,
    ).toBe("*"),
  );
  await user.click(screen.getByRole("button", { name: "Review (1)" }));
  expect(screen.queryByRole("heading", { name: "Mechanical" })).toBeNull();
  expect(
    screen
      .getByRole("button", { name: "Review (1)" })
      .getAttribute("aria-expanded"),
  ).toBe("false");
  await user.click(screen.getByRole("button", { name: "Review (1)" }));
  expect(screen.getByRole("heading", { name: "Mechanical" })).toBeTruthy();
});
test("saving a selected task into another group clears its hidden bulk selection", async () => {
  let task = {
    ...newTask(),
    name: "Move group",
    group: "Before",
    a: "A",
    b: "B",
  };
  api.invoke.mockImplementation(
    async (command: string, args?: { task: typeof task }) => {
      if (command === "tasks") return [task];
      if (command === "statuses") return [];
      if (command === "save_task" && args) task = args.task;
      return null;
    },
  );
  const user = userEvent.setup();
  render(<App />);
  await screen.findByRole("heading", { name: "Move group" });
  await user.selectOptions(
    screen.getByRole("combobox", { name: "Filter by group" }),
    "group:Before",
  );
  await user.click(screen.getByRole("checkbox", { name: "Select Move group" }));
  await user.click(screen.getByRole("button", { name: "Configure" }));
  await user.clear(screen.getByRole("textbox", { name: "Group" }));
  await user.type(screen.getByRole("textbox", { name: "Group" }), "After");
  await user.click(screen.getByRole("button", { name: "Save task" }));
  await screen.findByText("No tasks in this group.");
  expect(
    (screen.getByRole("button", { name: "Assign group" }) as HTMLButtonElement)
      .disabled,
  ).toBe(true);
  expect(
    (
      screen.getByRole("button", {
        name: "Preview selected",
      }) as HTMLButtonElement
    ).disabled,
  ).toBe(true);
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
        return current.status === "Applied"
          ? [
              {
                task_id: task.id,
                last_run: header(current),
                pending_conflicts: 0,
                next_run: null,
              },
            ]
          : [];
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
        current = { ...current, status: "Applied" };
        return { plan: header(current), error: null };
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
  await screen.findByText(
    "Run finished. See the actual result and protected versions.",
  );
  await user.click(screen.getByRole("button", { name: "All tasks" }));
  expect(await screen.findByText("Applied")).toBeTruthy();
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

test.each(["Failed: destination locked", "Cancelled"])(
  "terminal Apply result %s updates the dashboard and requires a fresh preview",
  async (status) => {
    const task = { ...newTask(), name: "Failed copy", a: "A", b: "B" };
    let failed = false;
    const plan: Plan = {
      id: "failed-run",
      task,
      created_at: 1,
      status: "Preview",
      metrics: {
        scanned_entries: 1,
        hashed_bytes: 1,
        copied_bytes: 0,
        queued_paths: 0,
        reconciliations: 1,
        watch_events: 0,
      },
      summary: {
        entries: 1,
        copies: 1,
        deletes: 0,
        conflicts: 0,
        unchanged: 0,
        to_a: 0,
        to_b: 1,
        remove_a: 0,
        remove_b: 0,
        explicit: false,
      },
    };
    api.invoke.mockImplementation(async (command: string) => {
      if (command === "tasks") return [task];
      if (command === "statuses")
        return failed
          ? [
              {
                task_id: task.id,
                last_run: { ...plan, status },
                pending_conflicts: 0,
                next_run: null,
              },
            ]
          : [];
      if (command === "preview") return plan;
      if (command === "operation_page") return { operations: [], total: 0 };
      if (command === "apply") {
        failed = true;
        return {
          plan: { ...plan, status },
          error:
            status === "Cancelled"
              ? "Cancelled at a safe boundary"
              : "destination locked",
        };
      }
      return null;
    });
    const user = userEvent.setup();
    render(<App />);
    await user.click(
      await screen.findByRole("button", { name: "Preview / Run" }),
    );
    await user.click(
      await screen.findByRole("button", { name: "Apply reviewed plan" }),
    );
    await user.click(screen.getByRole("button", { name: "Confirm" }));
    if (status === "Cancelled") {
      expect(
        await screen.findByText(
          "Run cancelled. Completed operations remain committed. Create a fresh preview to continue.",
        ),
      ).toBeTruthy();
      expect(screen.queryByRole("alert")).toBeNull();
    } else
      expect(await screen.findByText("Error: destination locked")).toBeTruthy();
    expect(screen.getByText(status)).toBeTruthy();
    expect(
      screen.queryByRole("button", { name: "Apply reviewed plan" }),
    ).toBeNull();
    await user.click(screen.getByRole("button", { name: "All tasks" }));
    expect(await screen.findByText(status)).toBeTruthy();
  },
);

test("latest progress is visible during background work and Cancel reaches the native stop command", async () => {
  api.invoke.mockImplementation(async (command: string) =>
    command === "tasks" || command === "statuses" ? [] : null,
  );
  const user = userEvent.setup();
  render(<App />);
  await screen.findByRole("button", { name: "New task" });
  await act(async () => {
    api.listeners.get("run-progress")?.({
      payload: {
        task_id: "task",
        run_id: "run",
        phase: "Transferring",
        completed_entries: 2,
        total_entries: 5,
        path: "large.bin",
        file_bytes: 1048576,
        file_total: 4194304,
      },
    });
  });
  expect(await screen.findByText("large.bin")).toBeTruthy();
  const bar = screen.getByRole("progressbar", {
    name: "Current file progress",
  }) as HTMLProgressElement;
  expect(bar.value).toBe(1048576);
  expect(bar.max).toBe(4194304);
  await user.click(screen.getByRole("button", { name: "Cancel" }));
  expect(api.invoke).toHaveBeenCalledWith("stop");
  await act(async () => {
    api.listeners.get("run-progress")?.({ payload: null });
  });
  expect(screen.queryByRole("progressbar")).toBeNull();
});

test("cancelled discovery refreshes persisted status and shows a localized cancellation message", async () => {
  const task = { ...newTask(), name: "Cancelled discovery", a: "A", b: "B" };
  let cancelled = false;
  const plan: Plan = {
    id: "cancelled-discovery",
    task,
    created_at: 1,
    status: "Cancelled",
    metrics: {
      scanned_entries: 1,
      hashed_bytes: 262144,
      copied_bytes: 0,
      queued_paths: 0,
      reconciliations: 1,
      watch_events: 0,
    },
    summary: {
      entries: 0,
      copies: 0,
      deletes: 0,
      conflicts: 0,
      unchanged: 0,
      to_a: 0,
      to_b: 0,
      remove_a: 0,
      remove_b: 0,
      explicit: false,
    },
  };
  api.invoke.mockImplementation(
    async (command: string, args?: { key?: string }) => {
      if (command === "tasks") return [task];
      if (command === "statuses")
        return cancelled
          ? [
              {
                task_id: task.id,
                last_run: plan,
                pending_conflicts: 0,
                next_run: null,
              },
            ]
          : [];
      if (command === "setting") return args?.key === "language" ? "de" : null;
      if (command === "preview") {
        cancelled = true;
        throw "[FolderSync:Cancelled]";
      }
      return null;
    },
  );
  const user = userEvent.setup();
  render(<App />);
  await user.click(
    await screen.findByRole("button", { name: "Vorschau / Start" }),
  );
  expect(
    await screen.findByText(
      "Lauf abgebrochen. Bereits abgeschlossene Aktionen bleiben erhalten. Erstellen Sie zum Fortsetzen eine neue Vorschau.",
    ),
  ).toBeTruthy();
  expect(screen.getByText("Abgebrochen")).toBeTruthy();
  expect(screen.queryByRole("alert")).toBeNull();
  expect(screen.queryByText("[FolderSync:Cancelled]")).toBeNull();
});
