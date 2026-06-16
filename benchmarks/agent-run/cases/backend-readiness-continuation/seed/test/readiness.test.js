const assert = require("node:assert/strict");
const test = require("node:test");

const { createReadinessTracker } = require("../src/readiness");

test("adds and lists backend services", () => {
  const tracker = createReadinessTracker({ now: () => new Date("2026-04-01T12:00:00.000Z") });

  const service = tracker.addService({ id: "api", name: "API", owner: "platform", status: "healthy" });

  assert.equal(service.id, "api");
  assert.equal(service.status, "healthy");
  assert.deepEqual(tracker.listServices({ status: "healthy" }).map((item) => item.id), ["api"]);
});

test("marks services healthy and unhealthy", () => {
  const tracker = createReadinessTracker({ now: () => new Date("2026-04-01T12:00:00.000Z") });
  tracker.addService({ id: "worker", name: "Worker" });

  assert.equal(tracker.markHealthy("worker").status, "healthy");
  assert.equal(tracker.markUnhealthy("worker").status, "unhealthy");
});

test("summarizes basic readiness", () => {
  const tracker = createReadinessTracker({ now: () => new Date("2026-04-01T12:00:00.000Z") });
  tracker.addService({ id: "api", name: "API", status: "healthy" });
  tracker.addService({ id: "worker", name: "Worker" });

  assert.deepEqual(tracker.summarizeReadiness(), {
    total: 2,
    healthy: 1,
    unhealthy: 1,
    generatedAt: "2026-04-01T12:00:00.000Z"
  });
});

test("rejects invalid services with structured errors", () => {
  const tracker = createReadinessTracker();

  assert.throws(
    () => tracker.addService({ name: "Missing id" }),
    (error) => error.code === "INVALID_SERVICE_ID" && error.status === 400
  );
  assert.throws(
    () => tracker.markHealthy("missing"),
    (error) => error.code === "SERVICE_NOT_FOUND" && error.status === 400
  );
});
