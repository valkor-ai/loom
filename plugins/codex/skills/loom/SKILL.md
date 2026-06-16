---
name: loom
description: Use when the user explicitly invokes @loom to route a software delivery task through the local loom CLI. The plugin uses delivery-scoped state, Brainstorm confirmation, contract/request artifacts, task execution requests, review, repair, continue routing, and explicit deploy routing.
---

# loom

You are the Codex adapter for loom's agent-neutral delivery protocol. The user-facing command is `@loom`; the CLI command remains `loom`.

Do not emulate loom in chat. Run the CLI, parse its JSON envelope, and treat returned artifacts as the source of truth.

Every agent-facing loom workflow command must use the shared launcher with the Codex profile:

```bash
LOOM_AGENT_PROFILE=codex LOOM_COMPACT_OUTPUT=1 "$HOME/.loom/bin/loom-cli" ...
```

When this skill mentions a command such as `loom continue`, treat it as a logical subcommand name. Execute it through `$HOME/.loom/bin/loom-cli` or the returned `commandInvocation`, never through a bare `loom` executable. If the CLI returns `AGENT_PROFILE_REQUIRED`, rerun the exact same command immediately with `LOOM_AGENT_PROFILE=codex` and all original arguments.

## Non-Negotiable Closeout

After an auto-runnable command response, your next action must be a tool call or file operation that follows `instruction`; do not send a progress summary first.

Before sending any final/progress response during an auto-runnable loom route, run this guard: if `actionRequired.finalResponseGuard` exists, or `instruction.mode = "execute_task"` and its `resultFile` is missing or `submitCommand` has not succeeded, do not respond to the user yet. Continue executing the instruction. If the task cannot be completed, write a failed or blocked TaskResult and run `submitCommand` so loom can route the failure. A recovery command is not a normal final answer; only if the host forcibly ends the turn while tools cannot continue, tell the user to run `@loom continue`.

For `execute_task`, a task is not complete when code is changed, tests pass, or the next task looks obvious. It is complete only after the TaskResult exists at `instruction.resultFile` and `instruction.submitCommand` has succeeded.

## First CLI Action

For explicit `@loom continue`, `@loom status`, `@loom deploy`, or `@loom deploy <subcommand>`, your first assistant action must be the matching CLI tool call. Do not answer in prose, recap state, read files, or inspect `.loom/` before that first CLI call.

- `@loom continue`: run `LOOM_AGENT_PROFILE=codex LOOM_COMPACT_OUTPUT=1 "$HOME/.loom/bin/loom-cli" continue --project-root /abs/project`
- `@loom status`: run `LOOM_AGENT_PROFILE=codex LOOM_COMPACT_OUTPUT=1 "$HOME/.loom/bin/loom-cli" status --project-root /abs/project`
- `@loom deploy`: run `LOOM_AGENT_PROFILE=codex LOOM_COMPACT_OUTPUT=1 "$HOME/.loom/bin/loom-cli" deploy run --project-root /abs/project`
- `@loom deploy <subcommand>`: run `LOOM_AGENT_PROFILE=codex LOOM_COMPACT_OUTPUT=1 "$HOME/.loom/bin/loom-cli" deploy <subcommand> --project-root /abs/project`

For deploy commands, if the first CLI tool call remains active, keep waiting on that same tool session. After one short "deploy is running" update, stay quiet while polling the original session. Do not run extra `deploy status`, `deploy inspect`, or `deploy logs` during the first 120 seconds unless the original command returns, the user asks for status, or a blocker appears. After 120 seconds, use read-only observation no more often than once every 60 seconds; prefer `deploy status`, and use `deploy logs` only after repeated unchanged status or explicit user request. If any envelope returns `mode: observe_active_deploy_operation`, obey `instruction.observationPolicy`. Do not send a final done/stuck/failed deploy response while `operationActive=true`.

Do not run manual `init` before `status`, `continue`, or `plan`. `status` is read-only and may report `STATE_NOT_INITIALIZED`; `plan` initializes `.loom/` when needed for new delivery requests. Do not hijack ordinary non-loom work: treat natural-language "continue" as loom only when the current project root has initialized and recoverable loom state.

## New Requests

For `@loom <request>` or `@loom plan <request>`, run `plan` through the launcher. For local requirement files such as PDF, DOCX, XLSX, TXT, MD, CSV, or TSV paths, pass one `--requirement-file <path>` per file, plus `--request "<remaining natural-language request>"` only when there is remaining non-file text. Do not pass requirement file paths as plain request text.

For a new request, read the returned `BrainstormSessionRequest` and manage the clarification conversation yourself using its guidance. Always present at least one understanding summary before accepting; the initial user request never counts as confirmation.

Clarify progressively in this block order:

1. `phase_scope`
2. `concept_grounding`
3. `frontend_experience`
4. `final_summary`

Do not merge required blocks. For `concept_grounding`, present delivery-wide and current-phase high-risk concepts with plain-language meanings and "must not be mistaken for" guards. For `frontend_experience`, ask in product language and preserve the confirmed target or skip reason. Confirming `phase_scope`, `concept_grounding`, or `frontend_experience` only advances the conversation to the next block; it is not permission to write `BrainstormCandidate` or run `brainstorm accept`. Do not read the candidate write contract, write `BrainstormCandidate`, or call `brainstorm accept` until the user explicitly confirms the dedicated `final_summary` block.

For Brainstorm `ask_user` gates, read `requestRef` and follow `agentAction.read.fieldGroups` inspect commands before presenting phase_scope, concept_grounding, frontend_experience, or final_summary. Do not stop at a request-ready/path-only recap; stop only after presenting the next required Brainstorm block as a concrete user-facing question or confirmation summary. Do not infer Brainstorm scope, sources, concepts, frontend target, candidateFile, output schema, or submit command from guessed legacy root fields such as `.objective`, `.scope`, or `.outputContract`.

## Instruction Priority

Every loom JSON response may include top-level `actionRequired` and `instruction`. These fields are the highest-priority routing signal.

- If `actionRequired.autoContinue` or `actionRequired.mustRunImmediately` is `true`, do not summarize progress, ask whether to continue, or stop after the command.
- Immediately execute top-level `instruction` according to its `mode`.
- If top-level `instruction` and `data.instruction` both exist, use the top-level copy first.
- If `instruction.continuationContract.kind = "auto_runnable_transition"`, the current turn is not complete. Do not stop with a recap, internal task update, or progress summary. Read `continuationContract.agentObligation`, then immediately follow top-level `instruction`: use `inputRefs`, produce `outputRefs`, run the listed command/submit command, obey `requiredSteps`, and stop only under `stopOnlyWhen`.
- Stop only for `ask_user`, `manual_review`, `needs_user_decision`, `report_blocked`, `report_done`, or a non-repairable command failure.

Supported instruction modes:

- `run_cli`: run `instruction.commandInvocation` when present. Otherwise run `instruction.command.argv` with `LOOM_AGENT_PROFILE=codex LOOM_COMPACT_OUTPUT=1 "$HOME/.loom/bin/loom-cli"` and the same `--project-root`. Do not use bare `loom`.
- `generate_candidate`: read `instruction.requestRef`, use `agentAction.read.fieldGroups[].readCommand` to read required request fields, generate the requested candidate/result files, write the exact output paths from the request contract, then run `instruction.submitCommand`. Do not create a new request.
- `submit_existing_candidate`: read `instruction.requestRef`, use `agentAction.read.fieldGroups[].readCommand` when request fields are needed, verify the named candidate/result files exist, then run `instruction.submitCommand`.
- `execute_task`: read `instruction.requestRef`, use `agentAction.read.fieldGroups[].readCommand` to read required request fields, follow `executionRules.sourceEditPreparationContract` before source/artifact writes, execute only that TaskExecutionRequest against project files, write `instruction.resultFile`, then run `instruction.submitCommand`. A progress-only response before `resultFile` exists and submit succeeds is invalid.
- `repair_candidate`: repair the same candidate file or grouped candidate files described by `instruction.issues`, then run `instruction.submitCommand`. Do not run `loom continue` before the repaired submit succeeds.
- `repair_result_contract`: repair the same result file described by `instruction.issues`, then run `instruction.submitCommand`. Do not run `loom continue` before the repaired submit succeeds.
- `deploy_repair_assets`: read `instruction.errorWindow` and diagnostics, edit only `instruction.editableFiles`, do not edit application code/package scripts/tests/RuntimeDeliveryContract, then run `instruction.retryCommand`. If the retry fails, run `loom deploy repair` and follow the returned instruction immediately.
- `observe_active_deploy_operation`: obey `instruction.observationPolicy`; prefer waiting on the original deploy command session, observe no more often than its interval, keep user-visible updates quiet, and do not send a final done/stuck/failed deploy response while `operationActive=true`.
- `ask_user`: ask only when the current user message does not already contain a usable answer. For Brainstorm gates, asking means presenting the next required block; a path-only "request is ready" summary is invalid.
- `manual_review`: show the short choices from `instruction.acceptedShortReplies`, then write and submit the manual review resolution after the user answers.
- `report_done`: surface `instruction.userMessage` when present, summarize completion using returned refs, and make clear that there is no active next step to continue.
- `report_blocked`: explain the blocker and exact condition needed to continue.

Command responses can also include a direct `data.instruction`. Top-level `instruction` is preferred when present; otherwise `data.instruction` is authoritative for local chaining:

- If `data.instruction.mode` is `run_cli` and `data.instruction.autoContinue`, `data.instruction.mustRunImmediately`, or `data.instruction.mustNotAskUserBeforeRunning` is `true`, immediately run the returned command through `commandInvocation` or `$HOME/.loom/bin/loom-cli`.
- If `data.instruction.mode` is `execute_task`, read `data.instruction.requestRef`, use `agentAction.read.fieldGroups[].readCommand` for required request fields, follow `executionRules.sourceEditPreparationContract` before source/artifact writes, complete the requested project-file task, write `data.instruction.resultFile`, and run `data.instruction.submitCommand`. Do not summarize or ask whether to continue after `next-task` or `record-result`; the task request has already been created and must be executed.
- If `data.instruction.mode` is `generate_candidate`, read `data.instruction.requestRef`, use `agentAction.read.fieldGroups[].readCommand` for required request fields, and complete the candidate generation workflow before running any next loom routing command.
- If `data.instruction.mode` is `submit_existing_candidate`, read `data.instruction.requestRef`, use `agentAction.read.fieldGroups[].readCommand` when request fields are needed, and submit the existing candidate/result files with `data.instruction.submitCommand` before running any next loom routing command.
- If `data.instruction.continuationContract.kind = "auto_runnable_transition"`, do not stop after the just-finished command. Follow the returned instruction until review, done, a user-gated decision, or a real blocking condition. Do not say "the next task is ready" or "if you want me to continue" for these auto-runnable next actions.
- If `data.nextAction.type` is `continue_execution`, first follow the returned `data.instruction`. When that instruction is already `execute_task`, do not run `next-task`; read its `requestRef`, submit its `TaskResult`, and repeat.
- If any command returns an `execute_task` instruction with `continuationContract`, a summary like "the next task is ready" is not an allowed stopping point. Continue into that `execute_task` unless the returned instruction is user-gated, blocked, done, or a non-repairable command failure.
- Run `next-task` only when the successful command response does not already include an `execute_task` instruction and the route decision explicitly asks for `next-task`.
- `next-task` returns a compact execution summary plus `data.instruction.mode = "execute_task"`. Always follow that instruction immediately: read `data.instruction.requestRef`, then use `agentAction.read.fieldGroups[].readCommand` to inspect complete required fields before modifying files. Treat direct sourceRefs reads as fallback only when inspect or the request read plan is insufficient.
- If `data.nextAction.type` is `review`, run `review` immediately and then submit the ReviewResult through its output contract.
- If an accept or record command returns `accepted:false`, `recorded:false`, top-level `instruction.mode=repair_candidate`, top-level `instruction.mode=repair_result_contract`, or a `repairInstruction`, follow that repair instruction first and resubmit the same accept/record command only when the issues are agent-repairable. Do not run `loom continue` until the repaired submit succeeds. After the repaired submit succeeds, immediately follow the successful response's `data.instruction`; do not stop to summarize if the next action is auto-runnable.
- If `data.nextAction.type` is `needs_user_decision`, top-level `instruction.mode` is `ask_user` or `needs_user_decision`, or any returned issue has `repairability=requires_user_decision`, do not treat the response as auto-runnable repair. Ask the user for the required decision, then rewrite and submit the same candidate/result only after the user answers. For TechnicalBaseline greenfield or additive-stack confirmation, never fabricate `approval.type=user_confirmed`, `approval.confirmedAt`, or `requiresUserConfirmation=false`.

## Request Protocol

When a command returns `requestRef`, read it first. If root `agentAction` is absent, read `requestManifest.refs.agentAction.ref` and use that sidecar agentAction as the read plan. Run each required field group's `readCommand` with the active launcher/profile, use `data.fields[<field>].value` as each complete field value, and do not print full `.loom` artifacts. If an inspect command fails, use that group's fields with requestManifest refs and targeted selectors. If inspect or fallback is unavailable, read requestRef and listed manifest refs directly as a correctness fallback and continue the task.

Use `agentAction` as the primary execution map:

- `agentAction.read`: what to inspect.
- `agentAction.read.fieldGroups`: exact request fields, purposes, read timing, inspect commands, and fallback selectors.
- `agentAction.write`: exact output files to write.
- `agentAction.schema`: schema and enum locations.
- `agentAction.submit`: exact command and required arguments.

Do not invent jq paths or infer submit arguments from older artifacts when `agentAction` exists. If a field lookup returns `null`, read `agentAction`, `referencedArtifactReadGuide`, and the named output contract fields instead of probing unrelated paths.

Keep chat output compact. Do not paste generated JSON candidates, result files, source diffs, full patches, full source files, full `.loom` JSON artifacts, historical TaskResult files, full TaskPlan files, full request files, full `SKILL.md`, or large command outputs unless the user explicitly asks to inspect them. For `.loom` request/result/contract artifacts, prefer request readPlan inspect commands over full-file output. Use short single-purpose selectors only as fallback when inspect is unavailable or missing the required field.

Candidate/result JSON under `.loom/` is machine-facing protocol data. Write or repair those files silently, then report only the artifact path, submit result, validation issues, and next action. Do not paste candidate/result JSON, unified diffs, or full file contents into chat.

## Execution Boundaries

Do not parallelize loom stateful commands. Every accept, record-result, review accept/resolve, and repair command must finish before running another routing command.

During `execute_task`, run verification commands serially by default. Parallelize only read-only inspection commands. If a command may write files, install dependencies, start a server, build artifacts, run tests, clean outputs, generate code, or mutate caches, do not run it in parallel with another command.

During `execute_task`, source and result writes are governed by `executionRules.sourceEditPreparationContract` when present. Do not start a file-write/edit operation until the contract's write plan is complete. If a write/edit operation is rejected for missing or invalid path/content/edit arguments before writing, return to that contract's write-plan sequence and do not repeat the malformed tool call. If the write boundary still cannot be determined after required request reads and source inspection, write the failed or blocked TaskResult described by that contract and run the submit command.

Do not modify product code except when executing a `TaskExecutionRequest` or execution repair request. Do not modify Brainstorm, TechnicalBaseline, PlanningGenerationContract, ArchitectureArtifactContract, TaskPlan, ReviewResult, or deployment state directly unless the current request explicitly asks for that candidate/result type.

If the request includes `taskConceptGrounding`, satisfy the listed concept responsibilities and record concrete `conceptEvidence` in the TaskResult. If it includes `frontendExperienceRequirement`, implement the required usable product surface, navigation/workflow coverage, interaction states, and explicit exclusions; include UIX evidence in the TaskResult when applicable: changed screens/components, covered states, responsive or accessibility checks, screenshot/Playwright refs, and remaining visual/manual-review risks. If it includes `runtimeDeliveryRequirement`, make the project's build/start/preview chain consistent with that requirement before reporting completion.

If the request is a deploy-sourced synthetic execution repair, treat it as an `execute_task` request that may edit application code/scripts but must not mutate the original TaskPlan, AAC, RuntimeDeliveryContract, generated Dockerfile/Compose/dockerignore, ReviewResult, or deploy state. Submit it with the returned repair submit command and immediately follow the returned deploy retry instruction.

## Frontend UIX Delivery

Keep UIX knowledge modular. When a request includes `frontend_experience`, `frontendExperienceRequirement`, frontend review signals, or user-visible UI work, load only the relevant references:

- [references/uix/core.md](references/uix/core.md) for the baseline UIX delivery contract.
- [references/uix/interaction.md](references/uix/interaction.md) for complex flows, forms, search, loading, feedback, state machines, or error recovery.
- [references/uix/system.md](references/uix/system.md) for design systems, tokens, component specs, theming, localization, icons, or motion.
- [references/uix/mobile.md](references/uix/mobile.md) for mobile, tablet, responsive, PWA, or native-app expectations.
- [references/uix/frameworks.md](references/uix/frameworks.md) when a frontend framework or component library is named.
- [references/uix/content.md](references/uix/content.md) for UX writing, empty states, error copy, CTAs, onboarding copy, or terminology.
- [references/uix/data.md](references/uix/data.md) for charts, dashboards, tables, analytics, finance, research, or visualization-heavy screens.
- [references/uix/verification.md](references/uix/verification.md) before visual, interaction, accessibility, or screenshot-based review.

## Deploy

For `@loom deploy`, deployment is an independent user-triggered workflow. Use `loom deploy` commands as the source of truth and use the `loom-deploy` skill for deploy-specific boundaries, diagnostics, and repair execution. Do not manually create Dockerfiles, rewrite Compose files, start alternate local servers, edit application code, change package scripts, run raw Docker/Compose commands, or invent preview URLs unless the returned deploy repair/execution-repair request explicitly allows that action.
