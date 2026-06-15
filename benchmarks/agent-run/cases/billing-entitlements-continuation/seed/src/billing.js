function validationError(code, message) {
  const error = new Error(message);
  error.code = code;
  error.status = 400;
  return error;
}

function clonePlan(plan) {
  return { ...plan };
}

function cloneSubscription(subscription) {
  return { ...subscription };
}

function createBillingStore({ now = () => new Date() } = {}) {
  const plans = new Map();
  const subscriptions = new Map();

  function timestamp() {
    return now().toISOString();
  }

  function createPlan(input) {
    const id = String(input?.id || "").trim();
    const name = String(input?.name || "").trim();
    const priceCents = Number(input?.priceCents ?? 0);
    if (!id) {
      throw validationError("INVALID_PLAN_ID", "Plan id is required.");
    }
    if (!name) {
      throw validationError("INVALID_PLAN_NAME", "Plan name is required.");
    }
    if (!Number.isInteger(priceCents) || priceCents < 0) {
      throw validationError("INVALID_PRICE", "Plan price must be a non-negative integer.");
    }
    if (plans.has(id)) {
      throw validationError("DUPLICATE_PLAN", "Plan already exists.");
    }

    const createdAt = timestamp();
    const plan = {
      id,
      name,
      priceCents,
      createdAt,
      updatedAt: createdAt
    };
    plans.set(id, plan);
    return clonePlan(plan);
  }

  function assignSubscription(input) {
    const customerId = String(input?.customerId || "").trim();
    const planId = String(input?.planId || "").trim();
    if (!customerId) {
      throw validationError("INVALID_CUSTOMER_ID", "Customer id is required.");
    }
    if (!plans.has(planId)) {
      throw validationError("PLAN_NOT_FOUND", "Plan was not found.");
    }

    const createdAt = timestamp();
    const subscription = {
      customerId,
      planId,
      status: "active",
      createdAt,
      updatedAt: createdAt
    };
    subscriptions.set(customerId, subscription);
    return cloneSubscription(subscription);
  }

  function listPlans() {
    return [...plans.values()].map(clonePlan);
  }

  function listSubscriptions(filter = {}) {
    let result = [...subscriptions.values()];
    if (filter.planId) {
      result = result.filter((subscription) => subscription.planId === filter.planId);
    }
    if (filter.status) {
      result = result.filter((subscription) => subscription.status === filter.status);
    }
    return result.map(cloneSubscription);
  }

  function summarizeBilling() {
    return {
      plans: plans.size,
      subscriptions: subscriptions.size,
      generatedAt: timestamp()
    };
  }

  return {
    createPlan,
    assignSubscription,
    listPlans,
    listSubscriptions,
    summarizeBilling
  };
}

module.exports = {
  createBillingStore,
  validationError
};
