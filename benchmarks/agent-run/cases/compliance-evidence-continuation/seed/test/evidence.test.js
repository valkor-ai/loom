const assert = require("node:assert/strict");
const test = require("node:test");

const { createEvidenceStore } = require("../src/evidence");

test("adds and lists evidence by status", () => {
  const store = createEvidenceStore({ now: () => new Date("2026-06-01T09:00:00.000Z") });

  const evidence = store.addEvidence({
    id: "mfa-policy",
    title: "MFA policy",
    owner: "security",
    status: "accepted"
  });

  assert.equal(evidence.id, "mfa-policy");
  assert.equal(evidence.status, "accepted");
  assert.deepEqual(store.listEvidence({ status: "accepted" }).map((item) => item.id), ["mfa-policy"]);
});

test("updates evidence review status", () => {
  const store = createEvidenceStore({ now: () => new Date("2026-06-01T09:00:00.000Z") });
  store.addEvidence({ id: "access-review", title: "Access review" });

  assert.equal(store.setEvidenceStatus("access-review", "accepted").status, "accepted");
  assert.equal(store.setEvidenceStatus("access-review", "rejected").status, "rejected");
});

test("summarizes evidence statuses", () => {
  const store = createEvidenceStore({ now: () => new Date("2026-06-01T09:00:00.000Z") });
  store.addEvidence({ id: "access-review", title: "Access review", status: "accepted" });
  store.addEvidence({ id: "mfa-policy", title: "MFA policy" });

  assert.deepEqual(store.summarizeEvidence(), {
    total: 2,
    accepted: 1,
    pending: 1,
    rejected: 0,
    generatedAt: "2026-06-01T09:00:00.000Z"
  });
});

test("rejects invalid evidence with structured errors", () => {
  const store = createEvidenceStore();

  assert.throws(
    () => store.addEvidence({ title: "Missing id" }),
    (error) => error.code === "INVALID_EVIDENCE_ID" && error.status === 400
  );
  assert.throws(
    () => store.setEvidenceStatus("missing", "accepted"),
    (error) => error.code === "EVIDENCE_NOT_FOUND" && error.status === 400
  );
});
