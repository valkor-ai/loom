# JavaScript Testing Quality

Use this topic reference when `tech/code/javascript/testing.md` is listed in `sourceContext.codeQualityRequirements[].referenceLoadPlan`.

Read it together with `tech/code/common.md`. Do not read sibling topic files unless they are also listed in the same load plan.

## When To Use

- The task adds or changes JavaScript tests, fixtures, test setup, mocks, runtime smoke checks, module import checks, or behavior implemented in plain JavaScript.
- Use this when JavaScript behavior needs proof across Node, browser, async, module, or side-effect boundaries.
- Follow the repository's existing runner and style unless the task explicitly owns test infrastructure.

## Implementation Focus

- Test externally visible behavior through stable module exports, CLI commands, HTTP handlers, browser interactions, or runtime adapters. Avoid tests that only prove private implementation steps.
- Await or return every promise under test. Do not mix callback-style `done` with `async` tests unless the runner requires it for a legacy API.
- Cover async success, rejection, timeout, cancellation, and cleanup paths when those flows changed.
- Use fake timers only when timer behavior is the subject of the test. Advance timers deliberately, flush pending microtasks where needed, and restore real timers.
- Match the test environment to the code: Node for filesystem/process/server code, jsdom or browser runner for DOM/storage APIs, and bundler/framework runner for browser modules.
- Mock network, filesystem, time, random IDs, and process boundaries at stable adapters. Do not mock the function whose business logic is being verified.
- Add module import smoke tests when changing ESM/CommonJS boundaries, dynamic imports, package exports, or CLI entry modules.
- Avoid snapshot-only coverage for interactive or business behavior. Assertions should name the observable result, error, side effect, or state transition.
- Keep fixtures realistic enough to catch validation and defaulting bugs; malformed input fixtures are required when parser or boundary code changed.

## Verification Focus

- Run the configured JavaScript test command and the lint/build command that covers changed files.
- Confirm no unhandled promise rejection, leaked timer, open handle, or test-environment mismatch appears in test output.
- For browser tests, include the relevant user interaction or runtime API path; for Node tests, include configuration and filesystem/process error paths when touched.
- Record unsupported runtime or browser gaps only when they are outside the task boundary or blocked by repository infrastructure.

## Evidence Notes

- Record `javascript.testing` in `codeQualityEvidence.referenceGroupsChecked`.
- Record `tech/code/javascript/testing.md` in `codeQualityEvidence.referenceFilesChecked` when this file influenced the implementation.
- In the evidence summary, name the JavaScript behavior verified and the commands run.
