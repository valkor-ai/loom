function validationError(code, message) {
  const error = new Error(message);
  error.code = code;
  error.status = 400;
  return error;
}

function cloneService(service) {
  return { ...service };
}

function createReadinessTracker({ now = () => new Date() } = {}) {
  const services = new Map();

  function timestamp() {
    return now().toISOString();
  }

  function addService(input) {
    const id = String(input?.id || "").trim();
    const name = String(input?.name || "").trim();
    if (!id) {
      throw validationError("INVALID_SERVICE_ID", "Service id is required.");
    }
    if (!name) {
      throw validationError("INVALID_SERVICE_NAME", "Service name is required.");
    }
    if (services.has(id)) {
      throw validationError("DUPLICATE_SERVICE", "Service already exists.");
    }

    const createdAt = timestamp();
    const service = {
      id,
      name,
      owner: input?.owner ? String(input.owner) : "platform",
      status: input?.status === "healthy" ? "healthy" : "unhealthy",
      createdAt,
      updatedAt: createdAt
    };
    services.set(id, service);
    return cloneService(service);
  }

  function setStatus(id, status) {
    if (!["healthy", "unhealthy"].includes(status)) {
      throw validationError("INVALID_SERVICE_STATUS", "Service status must be healthy or unhealthy.");
    }
    const service = services.get(id);
    if (!service) {
      throw validationError("SERVICE_NOT_FOUND", "Service was not found.");
    }
    service.status = status;
    service.updatedAt = timestamp();
    return cloneService(service);
  }

  function markHealthy(id) {
    return setStatus(id, "healthy");
  }

  function markUnhealthy(id) {
    return setStatus(id, "unhealthy");
  }

  function listServices(filter = {}) {
    let result = [...services.values()];
    if (filter.status) {
      result = result.filter((service) => service.status === filter.status);
    }
    if (filter.owner) {
      result = result.filter((service) => service.owner === filter.owner);
    }
    return result.map(cloneService);
  }

  function summarizeReadiness() {
    const all = [...services.values()];
    const healthy = all.filter((service) => service.status === "healthy").length;
    return {
      total: all.length,
      healthy,
      unhealthy: all.length - healthy,
      generatedAt: timestamp()
    };
  }

  return {
    addService,
    markHealthy,
    markUnhealthy,
    listServices,
    summarizeReadiness
  };
}

module.exports = {
  createReadinessTracker,
  validationError
};
