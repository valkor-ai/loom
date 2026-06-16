const assert = require("node:assert/strict");

const { createFlagRegistry } = require("./src/flags");

const registry = createFlagRegistry({ now: () => new Date("2026-09-01T12:00:00.000Z") });

const search = registry.createFlag({
  key: "search-v2",
  description: "New search",
  enabled: true,
  allowedSegments: ["beta"]
});
registry.createFlag({
  key: "checkout-v2",
  description: "New checkout",
  enabled: true,
  allowedSegments: ["beta"],
  prerequisites: ["search-v2"]
});
registry.createFlag({
  key: "payments-v2",
  description: "New payments",
  enabled: false
});
registry.createFlag({
  key: "billing-v2",
  description: "New billing",
  enabled: true,
  allowedSegments: ["enterprise"],
  prerequisites: ["payments-v2"]
});

assert.deepEqual(search.allowedSegments, ["beta"]);
search.allowedSegments.push("mutated");
assert.deepEqual(registry.listFlags().find((flag) => flag.key === "search-v2").allowedSegments, ["beta"]);

assert.equal(registry.evaluateFlag("search-v2", { segment: "beta" }), true);
assert.equal(registry.evaluateFlag("search-v2", { segment: "public" }), false);
assert.equal(registry.evaluateFlag("checkout-v2", { segment: "beta" }), true);
assert.equal(registry.evaluateFlag("billing-v2", { segment: "enterprise" }), false);
assert.deepEqual(registry.listBlockedFlags().map((flag) => flag.key), ["billing-v2"]);

assert.deepEqual(registry.summarizeFlags(), {
  total: 4,
  enabled: 3,
  disabled: 1,
  targetedCount: 3,
  blockedCount: 1,
  releaseStatus: "blocked",
  nextAction: "enable_prerequisites",
  generatedAt: "2026-09-01T12:00:00.000Z"
});

assert.throws(
  () => registry.createFlag({ key: "bad-segment", description: "Bad", allowedSegments: [""] }),
  (error) => error.code === "INVALID_SEGMENT" && error.status === 400
);
assert.throws(
  () => registry.createFlag({ key: "bad-prereq", description: "Bad", prerequisites: ["missing"] }),
  (error) => error.code === "UNKNOWN_PREREQUISITE" && error.status === 400
);
assert.throws(
  () => registry.evaluateFlag("missing", { segment: "beta" }),
  (error) => error.code === "FLAG_NOT_FOUND" && error.status === 400
);

console.log("feature-flags-continuation benchmark verification passed");
