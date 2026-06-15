const assert = require("node:assert/strict");

const { createAnalyticsStore } = require("./src/analytics");

const store = createAnalyticsStore({ now: () => new Date("2026-05-02T00:00:00.000Z") });

store.recordEvent({ userId: "u1", type: "view", occurredAt: "2026-05-01T10:00:00.000Z", properties: { plan: "pro" } });
store.recordEvent({ userId: "u1", type: "signup", occurredAt: "2026-05-01T10:05:00.000Z", properties: { plan: "pro" } });
store.recordEvent({ userId: "u1", type: "activate", occurredAt: "2026-05-01T10:10:00.000Z", properties: { plan: "pro" } });
store.recordEvent({ userId: "u2", type: "view", occurredAt: "2026-05-01T11:00:00.000Z", properties: { plan: "free" } });
store.recordEvent({ userId: "u2", type: "signup", occurredAt: "2026-05-01T11:20:00.000Z", properties: { plan: "free" } });
store.recordEvent({ userId: "u3", type: "signup", occurredAt: "2026-05-01T12:00:00.000Z", properties: { plan: "free" } });
store.recordEvent({ userId: "u4", type: "view", occurredAt: "2026-04-30T23:00:00.000Z", properties: { plan: "pro" } });
store.recordEvent({ userId: "u5", type: "view", occurredAt: "2026-05-01T13:00:00.000Z" });

const report = store.reportFunnel({
  steps: ["view", "signup", "activate"],
  from: "2026-05-01T00:00:00.000Z",
  to: "2026-05-02T00:00:00.000Z",
  segmentBy: "plan"
});

assert.deepEqual(report.steps, [
  { name: "view", users: 3, conversionRate: 1, dropOffFromPrevious: 0 },
  { name: "signup", users: 2, conversionRate: 0.667, dropOffFromPrevious: 1 },
  { name: "activate", users: 1, conversionRate: 0.333, dropOffFromPrevious: 1 }
]);
assert.deepEqual(report.segments, {
  pro: { view: 1, signup: 1, activate: 1 },
  free: { view: 1, signup: 1, activate: 0 },
  unknown: { view: 1, signup: 0, activate: 0 }
});
assert.deepEqual(report.window, {
  from: "2026-05-01T00:00:00.000Z",
  to: "2026-05-02T00:00:00.000Z"
});

assert.throws(
  () => store.reportFunnel({ steps: ["view"] }),
  (error) => error.code === "INVALID_FUNNEL_STEPS" && error.status === 400
);
assert.throws(
  () => store.reportFunnel({ steps: ["view", "signup"], from: "bad-date" }),
  (error) => error.code === "INVALID_FUNNEL_WINDOW" && error.status === 400
);

console.log("analytics-funnel-continuation benchmark verification passed");
