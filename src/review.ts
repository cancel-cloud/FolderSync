import type { Operation } from "./types";
export function summarize(ops: Operation[]) {
  return {
    copies: ops.filter((o) =>
      ["CopyToA", "CopyToB", "MoveToB"].includes(o.action),
    ).length,
    deletes: ops.filter((o) =>
      ["DeleteA", "DeleteB", "MoveToB"].includes(o.action),
    ).length,
    conflicts: ops.filter(
      (o) => o.action === "Conflict" || o.action === "Ignore",
    ).length,
    ignored: ops.filter((o) => o.action === "Ignore").length,
    unchanged: ops.filter((o) => o.action === "Unchanged").length,
  };
}

export function directions(ops: Operation[]) {
  return {
    toA: ops.filter((o) => o.action === "CopyToA").length,
    toB: ops.filter((o) => o.action === "CopyToB" || o.action === "MoveToB")
      .length,
    removeA: ops.filter((o) => o.action === "DeleteA" || o.action === "MoveToB")
      .length,
    removeB: ops.filter((o) => o.action === "DeleteB").length,
  };
}
