function validationError(code, message) {
  const error = new Error(message);
  error.code = code;
  error.status = 400;
  return error;
}

function cloneFlag(flag) {
  return { ...flag };
}

function createFlagRegistry({ now = () => new Date() } = {}) {
  const flags = new Map();

  function timestamp() {
    return now().toISOString();
  }

  function createFlag(input) {
    const key = String(input?.key || "").trim();
    const description = String(input?.description || "").trim();
    if (!key) {
      throw validationError("INVALID_FLAG_KEY", "Flag key is required.");
    }
    if (!description) {
      throw validationError("INVALID_DESCRIPTION", "Flag description is required.");
    }
    if (flags.has(key)) {
      throw validationError("DUPLICATE_FLAG", "Flag already exists.");
    }

    const createdAt = timestamp();
    const flag = {
      key,
      description,
      enabled: Boolean(input?.enabled),
      createdAt,
      updatedAt: createdAt
    };
    flags.set(key, flag);
    return cloneFlag(flag);
  }

  function setFlagEnabled(key, enabled) {
    const flag = flags.get(key);
    if (!flag) {
      throw validationError("FLAG_NOT_FOUND", "Flag was not found.");
    }
    flag.enabled = Boolean(enabled);
    flag.updatedAt = timestamp();
    return cloneFlag(flag);
  }

  function listFlags(filter = {}) {
    let result = [...flags.values()];
    if (filter.enabled !== undefined) {
      result = result.filter((flag) => flag.enabled === Boolean(filter.enabled));
    }
    return result.map(cloneFlag);
  }

  function summarizeFlags() {
    const all = [...flags.values()];
    const enabled = all.filter((flag) => flag.enabled).length;
    return {
      total: all.length,
      enabled,
      disabled: all.length - enabled,
      generatedAt: timestamp()
    };
  }

  return {
    createFlag,
    setFlagEnabled,
    listFlags,
    summarizeFlags
  };
}

module.exports = {
  createFlagRegistry,
  validationError
};
