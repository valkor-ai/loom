function validationError(code, message) {
  const error = new Error(message);
  error.code = code;
  error.status = 400;
  return error;
}

function cloneTicket(ticket) {
  return { ...ticket };
}

function createSupportQueue({ now = () => new Date() } = {}) {
  const tickets = [];
  let nextId = 1;

  function timestamp() {
    return now().toISOString();
  }

  function findTicket(id) {
    const ticket = tickets.find((candidate) => candidate.id === id);
    if (!ticket) {
      throw validationError("TICKET_NOT_FOUND", "Support ticket was not found.");
    }
    return ticket;
  }

  function addTicket(input) {
    const title = String(input?.title || "").trim();
    if (!title) {
      throw validationError("INVALID_TITLE", "Support ticket title is required.");
    }

    const createdAt = timestamp();
    const ticket = {
      id: String(nextId++),
      title,
      customer: input?.customer ? String(input.customer) : "unknown",
      status: "open",
      assignedTo: null,
      createdAt,
      updatedAt: createdAt
    };
    tickets.push(ticket);
    return cloneTicket(ticket);
  }

  function assignTicket(id, owner) {
    const assignee = String(owner || "").trim();
    if (!assignee) {
      throw validationError("INVALID_ASSIGNEE", "Assignee is required.");
    }

    const ticket = findTicket(id);
    ticket.assignedTo = assignee;
    ticket.updatedAt = timestamp();
    return cloneTicket(ticket);
  }

  function resolveTicket(id) {
    const ticket = findTicket(id);
    ticket.status = "resolved";
    ticket.updatedAt = timestamp();
    return cloneTicket(ticket);
  }

  function listTickets(filter = {}) {
    let result = tickets.slice();
    if (filter.status) {
      result = result.filter((ticket) => ticket.status === filter.status);
    }
    if (filter.assignedTo) {
      result = result.filter((ticket) => ticket.assignedTo === filter.assignedTo);
    }
    return result.map(cloneTicket);
  }

  function summarizeQueue() {
    const open = tickets.filter((ticket) => ticket.status === "open").length;
    const resolved = tickets.filter((ticket) => ticket.status === "resolved").length;
    return {
      total: tickets.length,
      open,
      resolved,
      generatedAt: timestamp()
    };
  }

  return {
    addTicket,
    assignTicket,
    resolveTicket,
    listTickets,
    summarizeQueue
  };
}

module.exports = {
  createSupportQueue,
  validationError
};
