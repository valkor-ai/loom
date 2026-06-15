const assert = require("node:assert/strict");

const { createEvidenceStore } = require("./src/evidence");

const store = createEvidenceStore({ now: () => new Date("2026-06-01T09:00:00.000Z") });

store.registerControl({
  id: "soc2-cc6",
  title: "Logical access",
  requiredEvidenceTypes: ["access-review", "mfa-policy"]
});
store.registerControl({
  id: "soc2-cc7",
  title: "Monitoring",
  requiredEvidenceTypes: ["incident-review", "alert-review"]
});

const access = store.addEvidence({
  id: "access-q2",
  title: "Q2 access review",
  owner: "security",
  evidenceType: "access-review",
  status: "accepted",
  expiresAt: "2026-07-01"
});
store.addEvidence({
  id: "mfa-policy",
  title: "MFA policy",
  evidenceType: "mfa-policy",
  status: "pending",
  expiresAt: "2026-12-31"
});
store.addEvidence({
  id: "incident-q1",
  title: "Incident review",
  evidenceType: "incident-review",
  status: "accepted",
  expiresAt: "2026-05-01"
});

access.title = "mutated";
assert.equal(
  store.listEvidence().find((item) => item.id === "access-q2").title,
  "Q2 access review",
  "returned evidence objects must be cloned"
);

assert.deepEqual(store.listReadyControls().map((control) => control.id), []);
store.setEvidenceStatus("mfa-policy", "accepted");
assert.deepEqual(store.listReadyControls().map((control) => control.id), ["soc2-cc6"]);

assert.deepEqual(store.summarizeCompliance(), {
  totalControls: 2,
  readyControlCount: 1,
  blockedControlCount: 1,
  missingEvidenceCount: 1,
  expiredEvidenceCount: 1,
  pendingEvidenceCount: 0,
  complianceStatus: "blocked",
  nextAction: "refresh_expired_evidence",
  generatedAt: "2026-06-01T09:00:00.000Z"
});

assert.throws(
  () => store.registerControl({ id: "bad", title: "Bad", requiredEvidenceTypes: ["access-review", "access-review"] }),
  (error) => error.code === "DUPLICATE_REQUIRED_EVIDENCE" && error.status === 400
);
assert.throws(
  () => store.addEvidence({ id: "bad-date", title: "Bad Date", evidenceType: "policy", expiresAt: "2026-02-30" }),
  (error) => error.code === "INVALID_EXPIRES_AT" && error.status === 400
);
assert.throws(
  () => store.registerControl({ id: "blank-type", title: "Blank", requiredEvidenceTypes: [""] }),
  (error) => error.code === "INVALID_REQUIRED_EVIDENCE_TYPE" && error.status === 400
);

console.log("compliance-evidence-continuation benchmark verification passed");
