function validationError(code, message) {
  const error = new Error(message);
  error.code = code;
  error.status = 400;
  return error;
}

function cloneItem(item) {
  return { ...item };
}

function createReleasePlanner({ now = () => new Date() } = {}) {
  const items = new Map();

  function timestamp() {
    return now().toISOString();
  }

  function addItem(input) {
    const id = String(input?.id || "").trim();
    const title = String(input?.title || "").trim();
    if (!id) {
      throw validationError("INVALID_ID", "Release item id is required.");
    }
    if (!title) {
      throw validationError("INVALID_TITLE", "Release item title is required.");
    }
    if (items.has(id)) {
      throw validationError("DUPLICATE_ITEM", "Release item already exists.");
    }

    const createdAt = timestamp();
    const item = {
      id,
      title,
      owner: input?.owner ? String(input.owner) : "unassigned",
      status: input?.status === "done" ? "done" : "todo",
      createdAt,
      updatedAt: createdAt
    };
    items.set(id, item);
    return cloneItem(item);
  }

  function completeItem(id) {
    const item = items.get(id);
    if (!item) {
      throw validationError("ITEM_NOT_FOUND", "Release item was not found.");
    }
    item.status = "done";
    item.updatedAt = timestamp();
    return cloneItem(item);
  }

  function listItems(filter = {}) {
    let result = [...items.values()];
    if (filter.status) {
      result = result.filter((item) => item.status === filter.status);
    }
    return result.map(cloneItem);
  }

  function summarizeRelease() {
    const all = [...items.values()];
    const completed = all.filter((item) => item.status === "done").length;
    return {
      total: all.length,
      completed,
      pending: all.length - completed,
      generatedAt: timestamp()
    };
  }

  return {
    addItem,
    completeItem,
    listItems,
    summarizeRelease
  };
}

module.exports = {
  createReleasePlanner,
  validationError
};
