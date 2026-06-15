const assert = require("node:assert/strict");

const { createReleasePlanner } = require("./src/release");

const planner = createReleasePlanner({ now: () => new Date("2026-02-01T12:00:00.000Z") });

planner.addItem({ id: "design", title: "Approve final launch design", owner: "design" });
planner.addItem({
  id: "api",
  title: "Finish launch API",
  owner: "eng",
  blockedBy: ["design"],
  carryOverFrom: "phase-1"
});
planner.addItem({ id: "docs", title: "Publish operator notes", owner: "pm" });

assert.deepEqual(
  planner.listReadyItems().map((item) => item.id),
  ["design", "docs"],
  "blocked API work must not appear ready until design is completed"
);
assert.deepEqual(planner.summarizeRelease(), {
  total: 3,
  completed: 0,
  pending: 3,
  readyCount: 2,
  blockedCount: 1,
  carriedOverCount: 1,
  status: "blocked",
  generatedAt: "2026-02-01T12:00:00.000Z"
});

planner.completeItem("design");

assert.deepEqual(
  planner.listReadyItems().map((item) => item.id),
  ["api", "docs"],
  "completed blockers should make dependent incomplete work ready"
);
assert.deepEqual(planner.summarizeRelease(), {
  total: 3,
  completed: 1,
  pending: 2,
  readyCount: 2,
  blockedCount: 0,
  carriedOverCount: 1,
  status: "ready",
  generatedAt: "2026-02-01T12:00:00.000Z"
});

assert.throws(
  () => planner.addItem({ id: "bad", title: "Bad blocker", blockedBy: ["missing"] }),
  (error) => error.code === "UNKNOWN_BLOCKER" && error.status === 400
);

console.log("release-readiness-continuation benchmark verification passed");
