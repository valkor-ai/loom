const assert = require("node:assert/strict");

const { createReadinessTracker } = require("./src/readiness");

const tracker = createReadinessTracker({
  now: () => new Date("2026-04-01T12:00:00.000Z"),
  env: {
    DATABASE_URL: "postgres://example",
    REDIS_URL: "redis://example"
  }
});

const db = tracker.addService({
  id: "db",
  name: "Database",
  status: "healthy",
  requiredEnv: ["DATABASE_URL"]
});
const api = tracker.addService({
  id: "api",
  name: "API",
  status: "healthy",
  requiredEnv: ["JWT_SECRET"],
  dependencies: ["db"]
});
const worker = tracker.addService({
  id: "worker",
  name: "Worker",
  status: "healthy",
  requiredEnv: ["REDIS_URL"],
  dependencies: ["api"]
});
const web = tracker.addService({
  id: "web",
  name: "Web",
  status: "unhealthy",
  dependencies: ["api"]
});

assert.deepEqual(db.requiredEnv, ["DATABASE_URL"]);
assert.deepEqual(api.dependencies, ["db"]);
assert.deepEqual(worker.dependencies, ["api"]);
worker.dependencies.push("web");
assert.deepEqual(
  tracker.listServices().find((service) => service.id === "worker").dependencies,
  ["api"],
  "returned service objects must be cloned"
);

assert.deepEqual(
  tracker.listReadyServices().map((service) => service.id),
  ["db"],
  "only env-satisfied healthy services with ready dependencies should be ready"
);
assert.deepEqual(tracker.summarizeReadiness(), {
  total: 4,
  healthy: 3,
  unhealthy: 1,
  readyCount: 1,
  blockedCount: 2,
  missingEnvCount: 1,
  unhealthyCount: 1,
  deploymentStatus: "blocked",
  nextAction: "set_missing_env",
  generatedAt: "2026-04-01T12:00:00.000Z"
});

assert.throws(
  () => tracker.addService({ id: "bad-dep", name: "Bad dependency", dependencies: ["missing"] }),
  (error) => error.code === "UNKNOWN_DEPENDENCY" && error.status === 400
);
assert.throws(
  () => tracker.addService({ id: "bad-env", name: "Bad env", requiredEnv: [""] }),
  (error) => error.code === "INVALID_REQUIRED_ENV" && error.status === 400
);

console.log("backend-readiness-continuation benchmark verification passed");
