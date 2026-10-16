// @vitest-environment jsdom
import { beforeAll, expect, test, vi } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
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
