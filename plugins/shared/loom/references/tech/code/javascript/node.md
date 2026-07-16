# JavaScript Node Runtime Quality

## When To Use

- The task changes Node.js services, CLIs, scripts, filesystem work, streams, environment configuration, HTTP servers, worker threads, child processes, or package runtime behavior.
- Use this when Node runtime behavior, portability, process lifecycle, or resource cleanup affects delivery quality.
- If JavaScript only runs in the browser, use browser-specific references instead.

## Implementation Focus

- Validate required environment variables and runtime configuration at startup. Fail with a clear message before serving traffic or mutating files when configuration is invalid.
- Prefer `fs/promises` and async filesystem APIs for runtime paths. Synchronous filesystem reads are acceptable for small startup configuration only when they do not sit on a request or job hot path.
- Use `path` and URL helpers for portability. Do not hard-code path separators or rely on the current working directory unless the command contract defines it.
- For ESM modules, derive file paths with `import.meta.url` and `fileURLToPath` rather than assuming `__dirname` exists.
- Use `stream/promises.pipeline` or equivalent error-aware stream composition for file/network streams. Avoid buffering large files into memory unless the size is bounded by the task contract.
- Attach error listeners for `EventEmitter` workflows where unhandled `error` events can crash the process, and remove listeners for long-lived emitters when the owner stops.
- For HTTP servers, queues, watchers, and background workers, implement shutdown behavior that closes sockets, timers, handles, temporary files, and child processes.
- Spawn child processes with argument arrays and explicit stdio handling. Avoid shell execution unless shell features are required, and bound stdout/stderr size when collecting output.
- Use worker threads only for CPU-bound work or isolation that justifies the overhead. Terminate workers and propagate worker errors to the owner.
- Do not log secrets, tokens, full environment dumps, or sensitive filesystem paths in normal runtime output.

### Process And Child Boundaries

Validate environment and CLI input before opening files, binding ports, spawning children, or mutating state. Use argument arrays and explicit working directories for child processes; set timeout/abort behavior, bound collected output, and terminate the child and descendants when the owner stops. Do not turn a shell command into a string interpolation boundary.

### HTTP And Stream Boundaries

Define request body, header, response, and connection limits for a Node HTTP server. Return a stable error response after parsing or routing failures and avoid writing headers twice. Use backpressure-aware stream composition and propagate request aborts to downstream work. Graceful shutdown must stop admission, drain bounded work, close servers/watchers/workers, and report failures without forcing an unsafe process loop.

## Verification Focus

- Run Node tests or runtime smoke commands for changed entry points.
- Verify startup fails clearly for missing required configuration and succeeds with valid overrides.
- For servers or long-running processes, test or manually smoke shutdown behavior if lifecycle code changed.
- For filesystem, stream, child-process, or worker changes, cover error paths and cleanup of temporary resources.
- For HTTP or process lifecycle changes, verify malformed input, abort/timeout, bounded output, shutdown, and repeated-signal behavior where owned.

## Evidence Focus

- In the evidence summary, name the Node decision: config validation, async filesystem, path portability, stream pipeline, server shutdown, child process boundary, worker use, or secret-safe logging.
