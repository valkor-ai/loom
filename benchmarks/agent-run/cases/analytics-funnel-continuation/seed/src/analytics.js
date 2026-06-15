function validationError(code, message) {
  const error = new Error(message);
  error.code = code;
  error.status = 400;
  return error;
}

function cloneEvent(event) {
  return { ...event, properties: { ...event.properties } };
}

function createAnalyticsStore({ now = () => new Date() } = {}) {
  const events = [];
  let nextId = 1;

  function timestamp() {
    return now().toISOString();
  }

  function recordEvent(input) {
    const userId = String(input?.userId || "").trim();
    const type = String(input?.type || "").trim();
    if (!userId) {
      throw validationError("INVALID_USER_ID", "Event userId is required.");
    }
    if (!type) {
      throw validationError("INVALID_EVENT_TYPE", "Event type is required.");
    }

    const event = {
      id: String(nextId++),
      userId,
      type,
      occurredAt: input?.occurredAt ? String(input.occurredAt) : timestamp(),
      properties: input?.properties && typeof input.properties === "object" ? { ...input.properties } : {}
    };
    events.push(event);
    return cloneEvent(event);
  }

  function listEvents(filter = {}) {
    let result = events.slice();
    if (filter.userId) {
      result = result.filter((event) => event.userId === filter.userId);
    }
    if (filter.type) {
      result = result.filter((event) => event.type === filter.type);
    }
    return result.map(cloneEvent);
  }

  function summarizeEvents() {
    const byType = {};
    for (const event of events) {
      byType[event.type] = (byType[event.type] || 0) + 1;
    }
    return {
      total: events.length,
      byType,
      generatedAt: timestamp()
    };
  }

  return {
    recordEvent,
    listEvents,
    summarizeEvents
  };
}

module.exports = {
  createAnalyticsStore,
  validationError
};
