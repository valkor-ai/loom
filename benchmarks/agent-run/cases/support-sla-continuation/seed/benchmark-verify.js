const assert = require("node:assert/strict");

const { createSupportQueue } = require("./src/support");

const queue = createSupportQueue({ now: () => new Date("2026-03-10T12:00:00.000Z") });

const enterprise = queue.addTicket({
  title: "Enterprise SSO outage",
  customerTier: "enterprise",
  openedAt: "2026-03-10T06:30:00.000Z"
});
const business = queue.addTicket({
  title: "Business billing export failure",
  customerTier: "business",
  openedAt: "2026-03-09T10:30:00.000Z"
});
const standard = queue.addTicket({
  title: "Standard account import stalled",
  customerTier: "standard",
  openedAt: "2026-03-07T10:30:00.000Z"
});
const freshEnterprise = queue.addTicket({
  title: "Enterprise dashboard question",
  customerTier: "enterprise",
  openedAt: "2026-03-10T10:30:00.000Z"
});
const resolvedBusiness = queue.addTicket({
  title: "Resolved business follow-up",
  customerTier: "business",
  openedAt: "2026-03-08T10:30:00.000Z"
});
const defaulted = queue.addTicket({ title: "Default tier ticket" });

queue.resolveTicket(resolvedBusiness.id);

assert.equal(defaulted.customerTier, "standard");
assert.equal(defaulted.openedAt, "2026-03-10T12:00:00.000Z");
assert.equal(enterprise.customerTier, "enterprise");
assert.equal(enterprise.openedAt, "2026-03-10T06:30:00.000Z");

assert.deepEqual(
  queue.listBreachedTickets().map((ticket) => ticket.id).sort(),
  [business.id, enterprise.id, standard.id].sort(),
  "only unresolved tickets past the tier-specific SLA should be breached"
);
assert.deepEqual(
  queue.listBreachedTickets({ customerTier: "enterprise" }).map((ticket) => ticket.id),
  [enterprise.id],
  "tier filters should apply to breached tickets"
);
assert.equal(
  queue.listBreachedTickets().some((ticket) => ticket.id === freshEnterprise.id || ticket.id === resolvedBusiness.id),
  false,
  "fresh or resolved tickets must not be included as breached"
);

assert.deepEqual(queue.summarizeQueue(), {
  total: 6,
  open: 5,
  resolved: 1,
  breachedCount: 3,
  breachedByTier: {
    enterprise: 1,
    business: 1,
    standard: 1
  },
  escalationStatus: "breached",
  nextAction: "escalate_enterprise",
  generatedAt: "2026-03-10T12:00:00.000Z"
});

const clearQueue = createSupportQueue({ now: () => new Date("2026-03-10T12:00:00.000Z") });
clearQueue.addTicket({
  title: "Fresh business ticket",
  customerTier: "business",
  openedAt: "2026-03-10T10:00:00.000Z"
});
assert.deepEqual(clearQueue.listBreachedTickets(), []);
assert.equal(clearQueue.summarizeQueue().escalationStatus, "clear");
assert.equal(clearQueue.summarizeQueue().nextAction, "monitor");

assert.throws(
  () => queue.addTicket({ title: "Bad tier", customerTier: "vip" }),
  (error) => error.code === "INVALID_CUSTOMER_TIER" && error.status === 400
);
assert.throws(
  () => queue.addTicket({ title: "Bad opened at", openedAt: "yesterday" }),
  (error) => error.code === "INVALID_OPENED_AT" && error.status === 400
);

console.log("support-sla-continuation benchmark verification passed");
