const assert = require("node:assert/strict");
const test = require("node:test");

const { createFlagRegistry } = require("../src/flags");

test("creates and lists feature flags", () => {
  const registry = createFlagRegistry({ now: () => new Date("2026-09-01T00:00:00.000Z") });

  const flag = registry.createFlag({ key: "search-v2", description: "New search", enabled: true });

  assert.equal(flag.enabled, true);
  assert.deepEqual(registry.listFlags({ enabled: true }).map((item) => item.key), ["search-v2"]);
});

test("enables and disables feature flags", () => {
  const registry = createFlagRegistry({ now: () => new Date("2026-09-01T00:00:00.000Z") });
  registry.createFlag({ key: "checkout-v2", description: "New checkout" });

  assert.equal(registry.setFlagEnabled("checkout-v2", true).enabled, true);
  assert.equal(registry.setFlagEnabled("checkout-v2", false).enabled, false);
});

test("summarizes feature flags", () => {
  const registry = createFlagRegistry({ now: () => new Date("2026-09-01T00:00:00.000Z") });
  registry.createFlag({ key: "search-v2", description: "New search", enabled: true });
  registry.createFlag({ key: "checkout-v2", description: "New checkout" });

  assert.deepEqual(registry.summarizeFlags(), {
    total: 2,
    enabled: 1,
    disabled: 1,
    generatedAt: "2026-09-01T00:00:00.000Z"
  });
});

test("rejects invalid flags with structured errors", () => {
  const registry = createFlagRegistry();

  assert.throws(
    () => registry.createFlag({ description: "Missing key" }),
    (error) => error.code === "INVALID_FLAG_KEY" && error.status === 400
  );
  assert.throws(
    () => registry.setFlagEnabled("missing", true),
    (error) => error.code === "FLAG_NOT_FOUND" && error.status === 400
  );
});
