const STATUSES = new Set(["open", "packed", "shipped"]);

function validationError(code, message) {
  const error = new Error(message);
  error.code = code;
  error.status = 400;
  return error;
}

function cloneOrder(order) {
  return { ...order };
}

function createFulfillmentQueue({ now = () => new Date() } = {}) {
  const orders = new Map();

  function timestamp() {
    return now().toISOString();
  }

  function addOrder(input) {
    const id = String(input?.id || "").trim();
    const customer = String(input?.customer || "").trim();
    if (!id) {
      throw validationError("INVALID_ORDER_ID", "Order id is required.");
    }
    if (!customer) {
      throw validationError("INVALID_CUSTOMER", "Customer is required.");
    }
    if (orders.has(id)) {
      throw validationError("DUPLICATE_ORDER", "Order already exists.");
    }

    const createdAt = timestamp();
    const order = {
      id,
      customer,
      status: "open",
      createdAt,
      updatedAt: createdAt
    };
    orders.set(id, order);
    return cloneOrder(order);
  }

  function setStatus(id, status) {
    if (!STATUSES.has(status)) {
      throw validationError("INVALID_ORDER_STATUS", "Order status is not supported.");
    }
    const order = orders.get(id);
    if (!order) {
      throw validationError("ORDER_NOT_FOUND", "Order was not found.");
    }
    order.status = status;
    order.updatedAt = timestamp();
    return cloneOrder(order);
  }

  function markPacked(id) {
    return setStatus(id, "packed");
  }

  function markShipped(id) {
    return setStatus(id, "shipped");
  }

  function listOrders(filter = {}) {
    let result = [...orders.values()];
    if (filter.status) {
      result = result.filter((order) => order.status === filter.status);
    }
    return result.map(cloneOrder);
  }

  function summarizeFulfillment() {
    const all = [...orders.values()];
    return {
      total: all.length,
      open: all.filter((order) => order.status === "open").length,
      packed: all.filter((order) => order.status === "packed").length,
      shipped: all.filter((order) => order.status === "shipped").length,
      generatedAt: timestamp()
    };
  }

  return {
    addOrder,
    markPacked,
    markShipped,
    listOrders,
    summarizeFulfillment
  };
}

module.exports = {
  createFulfillmentQueue,
  validationError
};
