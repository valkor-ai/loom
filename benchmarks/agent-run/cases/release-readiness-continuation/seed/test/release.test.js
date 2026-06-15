const assert = require("node:assert/strict");
const test = require("node:test");

const { createReleasePlanner } = require("../src/release");

test("adds and lists release items", () => {
  const planner = createReleasePlanner({ now: () => new Date("2026-02-01T00:00:00.000Z") });

  const item = planner.addItem({ id: "docs", title: "Publish upgrade notes", owner: "pm" });

  assert.equal(item.id, "docs");
  assert.equal(item.status, "todo");
  assert.deepEqual(planner.listItems({ status: "todo" }).map((candidate) => candidate.id), ["docs"]);
});

test("completes items and summarizes release progress", () => {
  const planner = createReleasePlanner({ now: () => new Date("2026-02-01T00:00:00.000Z") });
  planner.addItem({ id: "api", title: "Ship API" });
  planner.addItem({ id: "qa", title: "Run QA" });

  planner.completeItem("api");

  assert.deepEqual(planner.listItems({ status: "done" }).map((candidate) => candidate.id), ["api"]);
  assert.deepEqual(planner.summarizeRelease(), {
    total: 2,
    completed: 1,
    pending: 1,
    generatedAt: "2026-02-01T00:00:00.000Z"
  });
});

test("rejects invalid items with structured errors", () => {
  const planner = createReleasePlanner();

  assert.throws(
    () => planner.addItem({ title: "Missing id" }),
    (error) => error.code === "INVALID_ID" && error.status === 400
  );
  assert.throws(
    () => planner.addItem({ id: "missing-title" }),
    (error) => error.code === "INVALID_TITLE" && error.status === 400
  );
});
