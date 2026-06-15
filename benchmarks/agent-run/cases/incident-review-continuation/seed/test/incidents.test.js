const assert = require("node:assert/strict");
const test = require("node:test");

const { createIncidentTracker } = require("../src/incidents");

test("adds and lists incidents", () => {
  const tracker = createIncidentTracker({ now: () => new Date("2026-08-01T00:00:00.000Z") });

  const incident = tracker.addIncident({ id: "inc-1", title: "API outage", severity: "sev1" });

  assert.equal(incident.status, "open");
  assert.deepEqual(tracker.listIncidents({ severity: "sev1" }).map((item) => item.id), ["inc-1"]);
});

test("assigns and resolves incidents", () => {
  const tracker = createIncidentTracker({ now: () => new Date("2026-08-01T00:00:00.000Z") });
  tracker.addIncident({ id: "inc-1", title: "API outage" });

  assert.equal(tracker.assignIncident("inc-1", "oncall").owner, "oncall");
  assert.equal(tracker.resolveIncident("inc-1").status, "resolved");
});

test("summarizes incident counts", () => {
  const tracker = createIncidentTracker({ now: () => new Date("2026-08-01T00:00:00.000Z") });
  tracker.addIncident({ id: "inc-1", title: "API outage" });
  tracker.addIncident({ id: "inc-2", title: "Worker delay" });
  tracker.resolveIncident("inc-2");

  assert.deepEqual(tracker.summarizeIncidents(), {
    total: 2,
    open: 1,
    resolved: 1,
    generatedAt: "2026-08-01T00:00:00.000Z"
  });
});

test("rejects invalid incidents with structured errors", () => {
  const tracker = createIncidentTracker();

  assert.throws(
    () => tracker.addIncident({ title: "Missing id" }),
    (error) => error.code === "INVALID_INCIDENT_ID" && error.status === 400
  );
  assert.throws(
    () => tracker.assignIncident("missing", "owner"),
    (error) => error.code === "INCIDENT_NOT_FOUND" && error.status === 400
  );
});
