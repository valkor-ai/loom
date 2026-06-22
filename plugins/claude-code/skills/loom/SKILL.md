---
name: loom
description: Use when the user explicitly invokes /loom to route a software delivery task or knowledge-source command through the local loom CLI. The plugin uses delivery-scoped state, Brainstorm confirmation, contract/request artifacts, task execution requests, review, repair, continue routing, direct knowledge routing, and explicit deploy routing.
argument-hint: "<request> | plan <request> | continue | knowledge [subcommand] | deploy [subcommand] | status"
allowed-tools: [Read, Glob, Grep, Bash, Edit, MultiEdit, Write]
---

# loom

You are the Claude Code adapter for loom's agent-neutral delivery protocol. The user-facing command is `/loom`; the CLI command remains `loom`.

Do not emulate loom in chat. Run the CLI, parse its JSON envelope, and treat returned artifacts as the source of truth. Every workflow command must use the shared launcher with the Claude profile:

```bash
LOOM_AGENT_PROFILE=claude LOOM_COMPACT_OUTPUT=1 "$HOME/.loom/bin/loom-cli" ...
```

When this skill mentions a command such as `loom continue`, treat it as a logical subcommand name. Execute it through `$HOME/.loom/bin/loom-cli` or the returned `commandInvocation`, never through a bare `loom` executable. If the CLI returns `AGENT_PROFILE_REQUIRED`, rerun the exact same command immediately with `LOOM_AGENT_PROFILE=claude` and all original arguments.

## Non-Negotiable Closeout

After an auto-runnable command response, your next action must be a tool call or file operation that follows `instruction`; do not send a progress summary first.

Before sending any final/progress response during an auto-runnable loom route, run this guard: if `actionRequired.finalResponseGuard` exists, or `execute_task` lacks a submitted `resultFile`, keep executing. If completion is impossible, write a failed or blocked TaskResult and run `submitCommand`. If tools cannot continue, tell the user to run `/loom continue`.

For `execute_task`, a task is complete only after the TaskResult exists at `instruction.resultFile` and `instruction.submitCommand` has succeeded. Passing tests, completed source edits, internal todos, or a visible next task are not completion.

## First CLI Action

For `/loom continue`, `/loom status`, `/loom knowledge`, `/loom knowledge <subcommand>`, `/loom deploy`, or `/loom deploy <subcommand>`, your first assistant action must be the matching Bash tool call. Do not answer in prose, recap state, read files, or inspect `.loom/` before that first CLI call.

- `/loom continue`: run `LOOM_AGENT_PROFILE=claude LOOM_COMPACT_OUTPUT=1 "$HOME/.loom/bin/loom-cli" continue --project-root /abs/project`
- `/loom status`: run `LOOM_AGENT_PROFILE=claude LOOM_COMPACT_OUTPUT=1 "$HOME/.loom/bin/loom-cli" status --project-root /abs/project`
- `/loom knowledge`: run `LOOM_AGENT_PROFILE=claude LOOM_COMPACT_OUTPUT=1 "$HOME/.loom/bin/loom-cli" knowledge --project-root /abs/project`
- `/loom knowledge <subcommand>`: run `LOOM_AGENT_PROFILE=claude LOOM_COMPACT_OUTPUT=1 "$HOME/.loom/bin/loom-cli" knowledge <subcommand and args> --project-root /abs/project`
- `/loom deploy`: run `LOOM_AGENT_PROFILE=claude LOOM_COMPACT_OUTPUT=1 "$HOME/.loom/bin/loom-cli" deploy run --project-root /abs/project`
- `/loom deploy <subcommand>`: run `LOOM_AGENT_PROFILE=claude LOOM_COMPACT_OUTPUT=1 "$HOME/.loom/bin/loom-cli" deploy <subcommand> --project-root /abs/project`

Knowledge commands are direct source commands, not delivery requests. For `/loom knowledge ...`, do not run `plan`, `continue`, Brainstorm, candidate generation, task execution, or deploy routing before the knowledge command. Parse its JSON envelope; follow returned CLI instruction or report the result compactly. `knowledge build` and `knowledge resume` may return `generate_knowledge_semantics`; complete that workflow immediately.

For deploy commands, keep waiting on the first Bash session while it is active. After one short "deploy is running" update, stay quiet for the first 120 seconds unless the command returns, the user asks, or a blocker appears. Then observe no more often than once every 60 seconds; prefer `deploy status`, use logs sparingly, obey `instruction.observationPolicy`, and never send final deploy prose while `operationActive=true`.

Do not run manual `init` before `status`, `continue`, or `plan`. `status` is read-only and may report `STATE_NOT_INITIALIZED`; `plan` initializes `.loom/` when needed for new delivery requests. Do not hijack ordinary non-loom work: treat natural-language "continue" as loom only when the current project root has initialized and recoverable loom state.

## Claude Tool Boundaries
Use Claude Code's native file tools normally. If a file-read call fails because of tool arguments, retry with a valid native call or short selector. Treat that as a tool-call retry, not as a loom protocol blocker.

Avoid multi-line shell scripts for read-only inspection. Prefer loom `inspect` readCommands, short single-purpose selectors, or native file reads that do not print full artifacts into chat.

Do not use Claude Code Plan Mode, `ExitPlanMode`, or `.claude/plans/*` for any `/loom` workflow. Loom has already produced the executable request or user gate; replacing it with a Claude-internal plan approval breaks the delivery protocol. If Claude Code itself is already forced into Plan Mode and blocks Bash/Edit/Write, do not write a plan file and do not call `ExitPlanMode`; report that Claude Code must leave Plan Mode before `/loom` can execute.

Claude Code's internal task/todo tools and subagents may be used as implementation aids for source inspection, coding, verification, and local reasoning. They must not replace Loom workflow state, route nodes, decide completion, or justify stopping early. Loom state under `.loom/`, the CLI JSON envelope, and returned `instruction` / `actionRequired` fields are the only task source of truth. If Claude shows stale internal task reminders, ignore them for loom routing. `TaskStop` is allowed only to stop a task-owned background Bash/runtime after readiness probing or cleanup; do not use it as Loom progress.

## New Requests

Use `$ARGUMENTS` to choose entrypoint.

- `<request>` or `plan <request>`: run `LOOM_AGENT_PROFILE=claude LOOM_COMPACT_OUTPUT=1 "$HOME/.loom/bin/loom-cli" plan --project-root /abs/project --request "<request>"`. A bare `/loom <request>` is the normal new-delivery entrypoint and must behave like Codex `@loom <request>`.
- If the new delivery request includes local requirement files such as PDF, DOCX, XLSX, TXT, MD, CSV, or TSV paths, do not pass those paths as plain request text. Run `plan` with one `--requirement-file <path>` per requirement file, plus `--request "<remaining natural-language request>"` only when there is remaining non-file text.
- `continue`, `resume`, `proceed`, `next`, empty arguments in an initialized project, or a clear request to continue a known active loom delivery: run compact `continue`.
- `status`: run compact `status`.
- `deploy` or `deploy <subcommand>`: run the matching deploy CLI command, then use `loom-deploy` for deploy-specific skill guidance.

For a new request, read the returned `BrainstormSessionRequest` and manage clarification yourself. Always present at least one understanding summary before accepting; the initial user request never counts as confirmation. Clarify progressively in this block order: `phase_scope`, `concept_grounding`, `frontend_experience`, `final_summary`. Do not merge required blocks. Confirming `phase_scope`, `concept_grounding`, or `frontend_experience` only advances the conversation to the next block; it is not permission to write `BrainstormCandidate` or run `brainstorm accept`. Do not read the candidate write contract, write `BrainstormCandidate`, or call `brainstorm accept` until the user explicitly confirms the dedicated `final_summary` block.

During `phase_scope`, follow the request's `phaseScopeOptionComparison` guidance: present 2-3 source-grounded scope options by default, recommend exactly one, and treat `nextPhaseSeed` as a non-binding seed rather than a preselected answer. Use a single scope only when the request rules' atomic-scope exception is satisfied and explain that exception to the user.

For Brainstorm `ask_user` gates, read `requestRef` and follow root `requestReadPlan.groups`. Do this before presenting phase_scope, concept_grounding, frontend_experience, or final_summary. Do not stop at a request-ready/path-only recap; stop only after presenting the next required Brainstorm block. Do not infer scope, sources, concepts, frontend target, paths, schema, or submit command from guessed legacy root fields such as `.objective`, `.scope`, or `.outputContract`.

Follow Brainstorm `knowledgeQueryPlan`; do not merge its steps into one query.

## Instruction Priority

Every loom JSON response may include top-level `actionRequired` and `instruction`. These fields are the highest-priority routing signal.

- If `actionRequired.autoContinue` or `actionRequired.mustRunImmediately` is `true`, do not summarize progress, ask whether to continue, or stop after the command.
- Immediately execute top-level `instruction` according to its `mode`.
- If top-level `instruction` and `data.instruction` both exist, use the top-level copy first.
- If `instruction.continuationContract.kind = "auto_runnable_transition"`, the turn is not complete. Do not stop with recap/todo prose. Read `continuationContract.agentObligation`, then follow `instruction`: use `inputRefs`, produce `outputRefs`, run the listed command/submit command, obey `requiredSteps`, and stop only under `stopOnlyWhen`.
- Stop only for `ask_user`, `manual_review`, `needs_user_decision`, `report_blocked`, `report_done`, or a non-repairable command failure.

Supported instruction modes:

- `run_cli`: run `instruction.commandInvocation` when present. Otherwise run `instruction.command.argv` with `LOOM_AGENT_PROFILE=claude LOOM_COMPACT_OUTPUT=1 "$HOME/.loom/bin/loom-cli"` and the same `--project-root`. Do not use bare `loom`.
- `generate_candidate`: read `requestRef`; use root `requestReadPlan.groups`; write files. ArchitectureSections `single_section`: write `targetSection` -> `targetCandidateFile`, run `completionBarrier.followUpCommand.commandInvocation`, submit only after `submit_existing_candidate`. Others run `submitCommand`.
- `generate_knowledge_semantics`: read `instruction.requestRef`; read chunk bodies via `readCommand`; copy `request.outputContract.resultTemplate`, fill `chunkResults[]` by chunkId, write `instruction.resultFile`, run `instruction.submitCommand`, and follow returned semantic instructions until published or blocked. Do not ask whether to continue after `semantic_pending`. Do not inspect Loom source files, dist files, TypeScript types, or old results to infer schema; the template is authoritative.
- `submit_existing_candidate`: read `instruction.requestRef` when needed, verify named files exist, then run `instruction.submitCommand`.
- `execute_task`: read `instruction.requestRef`, use `requestReadPlan.groups[].readCommand`, follow `executionRules.sourceEditPreparationContract` before source/artifact writes, execute only that TaskExecutionRequest, write `instruction.resultFile`, then run `instruction.submitCommand`.
- `repair_candidate`: repair the same candidate file or grouped candidate files described by `instruction.issues`, then run `instruction.submitCommand`. Do not run `loom continue` before the repaired submit succeeds.
- `repair_result_contract`: repair the same result file described by `instruction.issues`, then run `instruction.submitCommand`.
- `deploy_repair_assets`: read `instruction.errorWindow` and diagnostics, edit only `instruction.editableFiles`, do not edit application code/package scripts/tests/RuntimeDeliveryContract, then run `instruction.retryCommand`.
- `observe_active_deploy_operation`: obey `instruction.observationPolicy`; prefer waiting on the original deploy command session, observe no more often than its interval, keep user-visible updates quiet, and do not send a final done/stuck/failed deploy response while `operationActive=true`.
- `ask_user`, `manual_review`, `report_done`, `report_blocked`: handle only the returned gate or report.

If a command response contains `data.instruction`, follow it like top-level `instruction`. If `data.instruction.mode` is auto-runnable, run it now. If `data.nextAction.type` is `continue_execution`, first follow `data.instruction`; when it is already `execute_task`, do not run `next-task`. `next-task` returns an execution request, not a stopping point.

If an accept or record command returns `accepted:false`, `recorded:false`, top-level `instruction.mode=repair_candidate`, top-level `instruction.mode=repair_result_contract`, or a `repairInstruction`, follow that repair instruction first and resubmit the same accept/record command only when the issues are agent-repairable. Do not run `loom continue` until the repaired submit succeeds. After the repaired submit succeeds, immediately follow the successful response's `data.instruction`; do not stop to summarize if the next action is auto-runnable.

If `data.nextAction.type` is `needs_user_decision`, top-level `instruction.mode` is `ask_user` or `needs_user_decision`, or any returned issue has `repairability=requires_user_decision`, do not treat the response as auto-runnable repair. Ask the user, then rewrite and submit the same candidate/result only after the user answers. For TechnicalBaseline greenfield or additive-stack confirmation, never fabricate `approval.type=user_confirmed`, `approval.confirmedAt`, or `requiresUserConfirmation=false`.

## Request Protocol

When a command returns `requestRef`, read it first. Root `requestReadPlan` is the read authority: run each required group's `readCommand`, use `data.fields[<field>].value`, and do not open full sidecar refs to discover the plan. If inspect fails, use that group's fields with requestManifest refs and targeted selectors; full listed refs are last-resort correctness fallback only.

Use `requestReadPlan` as primary request-field read map and `agentAction` as execution/write/submit map: `requestReadPlan.groups` for fields, `agentAction.write` for files, `agentAction.schema` for schema/enums, and `agentAction.submit` for submit commands. Do not invent shell selectors or infer submit arguments from older artifacts.

When an `execute_task` request contains `executionRules.sourceEditPreparationContract`, follow it before `Write`, `Edit`, `MultiEdit`, or quiet programmatic writes: form targetPath, writeKind, contentBasis, writeMethod, and writePayloadReady=true. If a native file-write tool rejects missing/invalid arguments before writing, return to that contract; do not repeat the malformed tool call. If the write boundary remains unclear, write the failed/blocked TaskResult and submit.

Candidate/result JSON under `.loom/` is machine-facing protocol data. Write or repair those files silently, then report only the artifact path, submit result, validation issues, and next action.

## Execution Boundaries

Do not parallelize loom stateful commands. Every accept, record-result, review accept/resolve, and repair command must finish before running another routing command.

During `execute_task`, run verification commands serially by default. Parallelize only read-only inspection commands. If a command may write files, install dependencies, start a server, build artifacts, run tests, clean outputs, generate code, or mutate caches, do not run it in parallel with another command.

Do not modify product code except when executing a `TaskExecutionRequest` or execution repair request. Do not modify Brainstorm, TechnicalBaseline, PlanningGenerationContract, ArchitectureArtifactContract, TaskPlan, ReviewResult, or deployment state directly unless the current request explicitly asks for that candidate/result type.

If the request includes `taskConceptGrounding`, satisfy the listed concept responsibilities and record concrete `conceptEvidence` in the TaskResult. If it includes `frontendExperienceRequirement`, implement the required usable product surface, navigation/workflow coverage, interaction states, and explicit exclusions; include UIX evidence in the TaskResult when applicable. If it includes `runtimeDeliveryRequirement`, make the project's build/start/preview chain consistent with that requirement before reporting completion.

If the request is a deploy-sourced synthetic execution repair, treat it as an `execute_task` request that may edit application code/scripts but must not mutate the original TaskPlan, AAC, RuntimeDeliveryContract, generated Dockerfile/Compose/dockerignore, ReviewResult, or deploy state. Submit it with the returned repair submit command and immediately follow the returned deploy retry instruction.

Keep chat output compact. Do not paste generated JSON candidates, result files, source diffs, full patches, full source files, full `.loom` JSON artifacts, historical TaskResult files, full TaskPlan files, full request files, full `SKILL.md`, or large command outputs unless the user explicitly asks to inspect them.

## Engineering Discipline

Load only the delivery reference matching the current instruction: [repair](references/delivery/repair.md), [testing](references/delivery/testing.md), [domain](references/delivery/domain.md), [planning](references/delivery/planning.md), [design](references/delivery/design.md), [review](references/delivery/review.md), or [handoff](references/delivery/handoff.md).

## Frontend UIX Delivery

For `frontend_experience`, `frontendExperienceRequirement`, frontend review signals, or user-visible UI work, load only relevant UIX references: [core](references/uix/core.md), [interaction](references/uix/interaction.md), [system](references/uix/system.md), [mobile](references/uix/mobile.md), [frameworks](references/uix/frameworks.md), [content](references/uix/content.md), [data](references/uix/data.md), or [verification](references/uix/verification.md).

## Deploy

For `/loom deploy`, deployment is an independent user-triggered workflow. Use `loom deploy` commands as the source of truth and use the `loom-deploy` skill for deploy-specific skill guidance. Do not manually create Dockerfiles, rewrite Compose files, start alternate local servers, edit application code, change package scripts, run raw Docker/Compose commands, or invent preview URLs unless the returned deploy repair/execution-repair request explicitly allows that action.
