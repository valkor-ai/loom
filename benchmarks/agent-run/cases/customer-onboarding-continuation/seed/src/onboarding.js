const STAGES = new Set(["discovery", "implementation", "validation", "launched"]);

function validationError(code, message) {
  const error = new Error(message);
  error.code = code;
  error.status = 400;
  return error;
}

function cloneCustomer(customer) {
  return { ...customer };
}

function createOnboardingTracker({ now = () => new Date() } = {}) {
  const customers = new Map();

  function timestamp() {
    return now().toISOString();
  }

  function validateStage(stage) {
    if (!STAGES.has(stage)) {
      throw validationError("INVALID_STAGE", "Customer stage is not supported.");
    }
    return stage;
  }

  function addCustomer(input) {
    const id = String(input?.id || "").trim();
    const name = String(input?.name || "").trim();
    if (!id) {
      throw validationError("INVALID_CUSTOMER_ID", "Customer id is required.");
    }
    if (!name) {
      throw validationError("INVALID_CUSTOMER_NAME", "Customer name is required.");
    }
    if (customers.has(id)) {
      throw validationError("DUPLICATE_CUSTOMER", "Customer already exists.");
    }

    const createdAt = timestamp();
    const customer = {
      id,
      name,
      owner: input?.owner ? String(input.owner) : "success",
      stage: validateStage(input?.stage || "discovery"),
      createdAt,
      updatedAt: createdAt
    };
    customers.set(id, customer);
    return cloneCustomer(customer);
  }

  function setStage(id, stage) {
    const customer = customers.get(id);
    if (!customer) {
      throw validationError("CUSTOMER_NOT_FOUND", "Customer was not found.");
    }
    customer.stage = validateStage(stage);
    customer.updatedAt = timestamp();
    return cloneCustomer(customer);
  }

  function listCustomers(filter = {}) {
    let result = [...customers.values()];
    if (filter.stage) {
      result = result.filter((customer) => customer.stage === filter.stage);
    }
    if (filter.owner) {
      result = result.filter((customer) => customer.owner === filter.owner);
    }
    return result.map(cloneCustomer);
  }

  function summarizeOnboarding() {
    const all = [...customers.values()];
    const byStage = {};
    for (const stage of STAGES) {
      byStage[stage] = all.filter((customer) => customer.stage === stage).length;
    }
    return {
      total: all.length,
      byStage,
      generatedAt: timestamp()
    };
  }

  return {
    addCustomer,
    setStage,
    listCustomers,
    summarizeOnboarding
  };
}

module.exports = {
  createOnboardingTracker,
  validationError
};
