const assert = require("node:assert/strict");

const { createOnboardingTracker } = require("./src/onboarding");

const tracker = createOnboardingTracker({ now: () => new Date("2026-05-01T12:00:00.000Z") });

const acme = tracker.addCustomer({
  id: "acme",
  name: "Acme",
  owner: "sara",
  stage: "validation",
  targetGoLiveAt: "2026-05-15",
  milestones: [
    { id: "contract", title: "Contract signed", completed: true },
    { id: "sso", title: "SSO configured" }
  ],
  blockers: [
    { id: "security", title: "Security review" }
  ]
});
const globex = tracker.addCustomer({
  id: "globex",
  name: "Globex",
  owner: "mika",
  stage: "validation",
  targetGoLiveAt: "2026-05-20",
  milestones: [{ id: "training", title: "Admin training", completed: true }]
});
tracker.addCustomer({
  id: "initech",
  name: "Initech",
  owner: "sara",
  stage: "implementation",
  targetGoLiveAt: "2026-04-20",
  milestones: [{ id: "import", title: "Data import" }]
});

assert.deepEqual(acme.milestones.map((milestone) => milestone.id), ["contract", "sso"]);
acme.milestones[1].completed = true;
assert.equal(
  tracker.listCustomers().find((customer) => customer.id === "acme").milestones[1].completed,
  false,
  "returned customer objects must be deeply cloned"
);

assert.deepEqual(tracker.listLaunchReady().map((customer) => customer.id), ["globex"]);
tracker.completeMilestone("acme", "sso");
tracker.resolveBlocker("acme", "security");
assert.deepEqual(tracker.listLaunchReady().map((customer) => customer.id), ["acme", "globex"]);

assert.deepEqual(tracker.summarizeOnboarding(), {
  total: 3,
  byStage: {
    discovery: 0,
    implementation: 1,
    validation: 2,
    launched: 0
  },
  readyCount: 2,
  blockedCount: 0,
  overdueCount: 1,
  incompleteMilestoneCount: 1,
  launchStatus: "attention",
  nextAction: "complete_milestones",
  generatedAt: "2026-05-01T12:00:00.000Z"
});

assert.throws(
  () => tracker.completeMilestone("acme", "missing"),
  (error) => error.code === "MILESTONE_NOT_FOUND" && error.status === 400
);
assert.throws(
  () => tracker.addCustomer({ id: "bad-date", name: "Bad Date", targetGoLiveAt: "2026-02-30" }),
  (error) => error.code === "INVALID_TARGET_GO_LIVE" && error.status === 400
);
assert.throws(
  () => tracker.addCustomer({ id: "bad-blocker", name: "Bad Blocker", blockers: [{ title: "Missing id" }] }),
  (error) => error.code === "INVALID_BLOCKER" && error.status === 400
);

console.log("customer-onboarding-continuation benchmark verification passed");
