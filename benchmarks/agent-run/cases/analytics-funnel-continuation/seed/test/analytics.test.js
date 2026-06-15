const assert = require("node:assert/strict");
const test = require("node:test");

const { createAnalyticsStore } = require("../src/analytics");

test("records and lists events", () => {
  const store = createAnalyticsStore({ now: () => new Date("2026-05-01T00:00:00.000Z") });

  const event = store.recordEvent({ userId: "u1", type: "view", properties: { plan: "pro" } });

  assert.equal(event.id, "1");
  assert.equal(event.occurredAt, "2026-05-01T00:00:00.000Z");
  assert.deepEqual(store.listEvents({ userId: "u1" }).map((item) => item.type), ["view"]);
  event.properties.plan = "mutated";
  assert.equal(store.listEvents()[0].properties.plan, "pro");
});

test("summarizes events by type", () => {
  const store = createAnalyticsStore({ now: () => new Date("2026-05-01T00:00:00.000Z") });
  store.recordEvent({ userId: "u1", type: "view" });
  store.recordEvent({ userId: "u2", type: "view" });
  store.recordEvent({ userId: "u1", type: "signup" });

  assert.deepEqual(store.summarizeEvents(), {
    total: 3,
    byType: {
      view: 2,
      signup: 1
    },
    generatedAt: "2026-05-01T00:00:00.000Z"
  });
});

test("rejects invalid events with structured errors", () => {
  const store = createAnalyticsStore();

  assert.throws(
    () => store.recordEvent({ type: "view" }),
    (error) => error.code === "INVALID_USER_ID" && error.status === 400
  );
  assert.throws(
    () => store.recordEvent({ userId: "u1" }),
    (error) => error.code === "INVALID_EVENT_TYPE" && error.status === 400
  );
});
