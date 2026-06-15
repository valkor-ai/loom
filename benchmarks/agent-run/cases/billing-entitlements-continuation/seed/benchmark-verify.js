const assert = require("node:assert/strict");

const { createBillingStore } = require("./src/billing");

const store = createBillingStore({ now: () => new Date("2026-07-01T12:00:00.000Z") });

const starter = store.createPlan({
  id: "starter",
  name: "Starter",
  priceCents: 1900,
  features: ["projects"],
  usageLimits: { seats: 5 }
});
const growth = store.createPlan({
  id: "growth",
  name: "Growth",
  priceCents: 4900,
  features: ["projects", "exports"],
  usageLimits: { seats: 10, apiCalls: 1000 }
});

store.assignSubscription({ customerId: "cust-a", planId: "growth" });
store.assignSubscription({ customerId: "cust-b", planId: "starter" });

store.recordUsage({ customerId: "cust-a", metric: "apiCalls", amount: 1200 });
store.recordUsage({ customerId: "cust-a", metric: "seats", amount: 6 });
store.recordUsage({ customerId: "cust-b", metric: "seats", amount: 4 });

assert.deepEqual(starter.features, ["projects"]);
assert.deepEqual(growth.usageLimits, { seats: 10, apiCalls: 1000 });
assert.equal(store.canUseFeature("cust-a", "exports"), true);
assert.equal(store.canUseFeature("cust-b", "exports"), false);
assert.equal(store.canUseFeature("cust-b", "projects"), true);

assert.deepEqual(store.listOverLimitSubscriptions(), [
  {
    customerId: "cust-a",
    planId: "growth",
    overLimit: [
      { metric: "apiCalls", used: 1200, limit: 1000 }
    ]
  }
]);

assert.deepEqual(store.summarizeBilling(), {
  plans: 2,
  subscriptions: 2,
  enabledFeatureCount: 3,
  overLimitCount: 1,
  billingStatus: "attention",
  nextAction: "review_over_limit_usage",
  generatedAt: "2026-07-01T12:00:00.000Z"
});

assert.throws(
  () => store.createPlan({ id: "bad-feature", name: "Bad Feature", features: [""] }),
  (error) => error.code === "INVALID_FEATURE" && error.status === 400
);
assert.throws(
  () => store.recordUsage({ customerId: "missing", metric: "apiCalls", amount: 1 }),
  (error) => error.code === "SUBSCRIPTION_NOT_FOUND" && error.status === 400
);
assert.throws(
  () => store.recordUsage({ customerId: "cust-a", metric: "apiCalls", amount: -1 }),
  (error) => error.code === "INVALID_USAGE_AMOUNT" && error.status === 400
);

console.log("billing-entitlements-continuation benchmark verification passed");
