const STATUSES = new Set(["pending", "accepted", "rejected"]);

function validationError(code, message) {
  const error = new Error(message);
  error.code = code;
  error.status = 400;
  return error;
}

function cloneEvidence(evidence) {
  return { ...evidence };
}

function createEvidenceStore({ now = () => new Date() } = {}) {
  const evidenceItems = new Map();

  function timestamp() {
    return now().toISOString();
  }

  function validateStatus(status) {
    if (!STATUSES.has(status)) {
      throw validationError("INVALID_EVIDENCE_STATUS", "Evidence status is not supported.");
    }
    return status;
  }

  function addEvidence(input) {
    const id = String(input?.id || "").trim();
    const title = String(input?.title || "").trim();
    if (!id) {
      throw validationError("INVALID_EVIDENCE_ID", "Evidence id is required.");
    }
    if (!title) {
      throw validationError("INVALID_EVIDENCE_TITLE", "Evidence title is required.");
    }
    if (evidenceItems.has(id)) {
      throw validationError("DUPLICATE_EVIDENCE", "Evidence already exists.");
    }

    const createdAt = timestamp();
    const evidence = {
      id,
      title,
      owner: input?.owner ? String(input.owner) : "compliance",
      status: validateStatus(input?.status || "pending"),
      createdAt,
      updatedAt: createdAt
    };
    evidenceItems.set(id, evidence);
    return cloneEvidence(evidence);
  }

  function setEvidenceStatus(id, status) {
    const evidence = evidenceItems.get(id);
    if (!evidence) {
      throw validationError("EVIDENCE_NOT_FOUND", "Evidence was not found.");
    }
    evidence.status = validateStatus(status);
    evidence.updatedAt = timestamp();
    return cloneEvidence(evidence);
  }

  function listEvidence(filter = {}) {
    let result = [...evidenceItems.values()];
    if (filter.status) {
      result = result.filter((evidence) => evidence.status === filter.status);
    }
    if (filter.owner) {
      result = result.filter((evidence) => evidence.owner === filter.owner);
    }
    return result.map(cloneEvidence);
  }

  function summarizeEvidence() {
    const all = [...evidenceItems.values()];
    return {
      total: all.length,
      accepted: all.filter((evidence) => evidence.status === "accepted").length,
      pending: all.filter((evidence) => evidence.status === "pending").length,
      rejected: all.filter((evidence) => evidence.status === "rejected").length,
      generatedAt: timestamp()
    };
  }

  return {
    addEvidence,
    setEvidenceStatus,
    listEvidence,
    summarizeEvidence
  };
}

module.exports = {
  createEvidenceStore,
  validationError
};
