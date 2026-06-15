const assert = require("node:assert/strict");
const test = require("node:test");

const { createOnboardingTracker } = require("../src/onboarding");

test("adds and lists customers by stage", () => {
  const tracker = createOnboardingTracker({ now: () => new Date("2026-05-01T12:00:00.000Z") });

  const customer = tracker.addCustomer({
    id: "acme",
    name: "Acme",
    owner: "sara",
    stage: "implementation"
  });

  assert.equal(customer.id, "acme");
  assert.equal(customer.stage, "implementation");
  assert.deepEqual(tracker.listCustomers({ stage: "implementation" }).map((item) => item.id), ["acme"]);
});

test("moves customers between supported stages", () => {
  const tracker = createOnboardingTracker({ now: () => new Date("2026-05-01T12:00:00.000Z") });
  tracker.addCustomer({ id: "globex", name: "Globex" });

  assert.equal(tracker.setStage("globex", "validation").stage, "validation");
  assert.equal(tracker.setStage("globex", "launched").stage, "launched");
});

test("summarizes stage counts", () => {
  const tracker = createOnboardingTracker({ now: () => new Date("2026-05-01T12:00:00.000Z") });
  tracker.addCustomer({ id: "acme", name: "Acme", stage: "implementation" });
  tracker.addCustomer({ id: "globex", name: "Globex", stage: "validation" });

  assert.deepEqual(tracker.summarizeOnboarding(), {
    total: 2,
    byStage: {
      discovery: 0,
      implementation: 1,
      validation: 1,
      launched: 0
    },
    generatedAt: "2026-05-01T12:00:00.000Z"
  });
});

test("rejects invalid customers with structured errors", () => {
  const tracker = createOnboardingTracker();

  assert.throws(
    () => tracker.addCustomer({ name: "Missing id" }),
    (error) => error.code === "INVALID_CUSTOMER_ID" && error.status === 400
  );
  assert.throws(
    () => tracker.setStage("missing", "validation"),
    (error) => error.code === "CUSTOMER_NOT_FOUND" && error.status === 400
  );
});
