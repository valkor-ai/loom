const assert = require("node:assert/strict");
const test = require("node:test");

const { createBillingStore } = require("../src/billing");

test("creates and lists billing plans", () => {
  const store = createBillingStore({ now: () => new Date("2026-07-01T00:00:00.000Z") });

  const plan = store.createPlan({ id: "starter", name: "Starter", priceCents: 1900 });

  assert.equal(plan.id, "starter");
  assert.equal(plan.priceCents, 1900);
  assert.deepEqual(store.listPlans().map((item) => item.id), ["starter"]);
});

test("assigns customer subscriptions", () => {
  const store = createBillingStore({ now: () => new Date("2026-07-01T00:00:00.000Z") });
  store.createPlan({ id: "growth", name: "Growth", priceCents: 4900 });

  const subscription = store.assignSubscription({ customerId: "cust-a", planId: "growth" });

  assert.equal(subscription.status, "active");
  assert.deepEqual(store.listSubscriptions({ planId: "growth" }).map((item) => item.customerId), ["cust-a"]);
});

test("summarizes billing counts", () => {
  const store = createBillingStore({ now: () => new Date("2026-07-01T00:00:00.000Z") });
  store.createPlan({ id: "starter", name: "Starter" });
  store.assignSubscription({ customerId: "cust-a", planId: "starter" });

  assert.deepEqual(store.summarizeBilling(), {
    plans: 1,
    subscriptions: 1,
    generatedAt: "2026-07-01T00:00:00.000Z"
  });
});

test("rejects invalid billing inputs with structured errors", () => {
  const store = createBillingStore();

  assert.throws(
    () => store.createPlan({ name: "Missing id" }),
    (error) => error.code === "INVALID_PLAN_ID" && error.status === 400
  );
  assert.throws(
    () => store.createPlan({ id: "bad", name: "Bad", priceCents: -1 }),
    (error) => error.code === "INVALID_PRICE" && error.status === 400
  );
  assert.throws(
    () => store.assignSubscription({ customerId: "cust-a", planId: "missing" }),
    (error) => error.code === "PLAN_NOT_FOUND" && error.status === 400
  );
});
