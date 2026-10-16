import { expect, test } from "vitest";
import { summarize, directions } from "./review";
test("a move counts both verified transfer and source removal", () => {
  expect(
    summarize([{ path: "f", action: "MoveToB", reason: "", a: null, b: null }]),
  ).toEqual({ copies: 1, deletes: 1, conflicts: 0, unchanged: 0 });
});

test("direction counts separate destination transfers and Move source removal", () => {
  const ops = ["CopyToA", "CopyToB", "MoveToB", "DeleteB"].map((action) => ({
    path: action,
    action,
    reason: "",
    a: null,
    b: null,
  })) as Parameters<typeof directions>[0];
  expect(directions(ops)).toEqual({ toA: 1, toB: 2, removeA: 1, removeB: 1 });
});
