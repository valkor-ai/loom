function validationError(code, message) {
  const error = new Error(message);
  error.code = code;
  error.status = 400;
  return error;
}

function cloneIncident(incident) {
  return { ...incident };
}

function createIncidentTracker({ now = () => new Date() } = {}) {
  const incidents = new Map();

  function timestamp() {
    return now().toISOString();
  }

  function addIncident(input) {
    const id = String(input?.id || "").trim();
    const title = String(input?.title || "").trim();
    if (!id) {
      throw validationError("INVALID_INCIDENT_ID", "Incident id is required.");
    }
    if (!title) {
      throw validationError("INVALID_TITLE", "Incident title is required.");
    }
    if (incidents.has(id)) {
      throw validationError("DUPLICATE_INCIDENT", "Incident already exists.");
    }

    const createdAt = timestamp();
    const incident = {
      id,
      title,
      severity: input?.severity ? String(input.severity) : "sev3",
      owner: input?.owner ? String(input.owner) : "unassigned",
      status: "open",
      createdAt,
      updatedAt: createdAt
    };
    incidents.set(id, incident);
    return cloneIncident(incident);
  }

  function findIncident(id) {
    const incident = incidents.get(id);
    if (!incident) {
      throw validationError("INCIDENT_NOT_FOUND", "Incident was not found.");
    }
    return incident;
  }

  function assignIncident(id, owner) {
    const assignee = String(owner || "").trim();
    if (!assignee) {
      throw validationError("INVALID_OWNER", "Incident owner is required.");
    }
    const incident = findIncident(id);
    incident.owner = assignee;
    incident.updatedAt = timestamp();
    return cloneIncident(incident);
  }

  function resolveIncident(id) {
    const incident = findIncident(id);
    incident.status = "resolved";
    incident.updatedAt = timestamp();
    return cloneIncident(incident);
  }

  function listIncidents(filter = {}) {
    let result = [...incidents.values()];
    if (filter.status) {
      result = result.filter((incident) => incident.status === filter.status);
    }
    if (filter.severity) {
      result = result.filter((incident) => incident.severity === filter.severity);
    }
    return result.map(cloneIncident);
  }

  function summarizeIncidents() {
    const all = [...incidents.values()];
    const open = all.filter((incident) => incident.status === "open").length;
    return {
      total: all.length,
      open,
      resolved: all.length - open,
      generatedAt: timestamp()
    };
  }

  return {
    addIncident,
    assignIncident,
    resolveIncident,
    listIncidents,
    summarizeIncidents
  };
}

module.exports = {
  createIncidentTracker,
  validationError
};
