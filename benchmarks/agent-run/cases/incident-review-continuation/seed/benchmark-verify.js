const assert = require("node:assert/strict");

const { createIncidentTracker } = require("./src/incidents");

const tracker = createIncidentTracker({ now: () => new Date("2026-08-03T12:00:00.000Z") });

tracker.addIncident({ id: "inc-1", title: "API outage", severity: "sev1", owner: "platform" });
tracker.addIncident({ id: "inc-2", title: "Delayed exports", severity: "sev2", owner: "data" });

const overdue = tracker.addActionItem("inc-1", {
  id: "action-1",
  title: "Patch retry timeout",
  owner: "platform",
  dueAt: "2026-08-02T12:00:00.000Z"
});
const upcoming = tracker.addActionItem("inc-1", {
  id: "action-2",
  title: "Publish customer follow-up",
  owner: "support",
  dueAt: "2026-08-04T12:00:00.000Z"
});
const completed = tracker.addActionItem("inc-2", {
  id: "action-3",
  title: "Backfill export jobs",
  owner: "data",
  dueAt: "2026-08-01T12:00:00.000Z"
});
tracker.completeActionItem("inc-2", completed.id);
tracker.resolveIncident("inc-2");

assert.equal(overdue.status, "open");
assert.equal(upcoming.owner, "support");
upcoming.owner = "mutated";
assert.equal(tracker.listOpenActionItems().find((item) => item.id === "action-2").owner, "support");

assert.deepEqual(tracker.listOpenActionItems().map((item) => item.id), ["action-1", "action-2"]);
assert.deepEqual(tracker.listOverdueActionItems().map((item) => item.id), ["action-1"]);
assert.deepEqual(tracker.summarizeIncidents(), {
  total: 2,
  open: 1,
  resolved: 1,
  openActionItemCount: 2,
  overdueActionItemCount: 1,
  reviewStatus: "blocked",
  nextAction: "resolve_overdue_actions",
  generatedAt: "2026-08-03T12:00:00.000Z"
});

assert.throws(
  () => tracker.addActionItem("missing", { id: "bad", title: "Bad", owner: "nobody", dueAt: "2026-08-04T12:00:00.000Z" }),
  (error) => error.code === "INCIDENT_NOT_FOUND" && error.status === 400
);
assert.throws(
  () => tracker.addActionItem("inc-1", { id: "bad-date", title: "Bad date", owner: "owner", dueAt: "tomorrow" }),
  (error) => error.code === "INVALID_DUE_AT" && error.status === 400
);

console.log("incident-review-continuation benchmark verification passed");
