type RequestCommandResult = Record<string, unknown> & {
  request?: Record<string, unknown>;
  instruction?: Record<string, unknown>;
};

export function compactRequestCommandResult<T extends RequestCommandResult>(result: T): Omit<T, "request"> {
  const { request: _request, ...rest } = result;
  return rest;
}
