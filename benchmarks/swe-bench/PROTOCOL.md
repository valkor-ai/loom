# Loom Delivery Benchmark Protocol

This protocol evaluates the outcome of an agent-run software delivery. It is designed for comparisons between a coding agent alone and the same agent using Loom.

It has two tracks. They should be reported separately.

| Track | Question | Primary outcome |
| --- | --- | --- |
| Resolution | Can the agent resolve a public software issue? | Official SWE-bench Verified resolve rate |
| Recovery | Can the agent recover from independent validation feedback and finish the delivery? | Official resolve rate after feedback and repair |

SWE-bench is a resolution benchmark. It does not, by itself, measure planning, evidence, handoff, or recovery. The Recovery track exists to measure those delivery behaviors without treating them as an unreported side effect of a bug-fix score.

## Scope

- Use SWE-bench Verified instances and their recorded base commits.
- Do not expose gold patches, test patches, or evaluation results to an agent before its initial submission.
- Run the official SWE-bench harness in its pinned Docker environment for every reported resolve result.
- Treat local test commands as supporting evidence only.
- Publish the runner revision, dataset revision, model identifier, provider, reasoning setting, date, and host platform with every result.

## Paired Agent Runs

Each instance has two isolated workspaces created from the same base commit:

- `direct`: the coding agent receives the issue and is asked to implement, test, and finish it.
- `loom`: the same coding agent receives the same issue and uses Loom from planning through review and handoff.

Keep the following fixed within a paired run:

- model and provider
- reasoning or effort setting
- tool permissions and network policy
- base commit and task prompt
- maximum wall-clock time and agent turn budget
- repository setup and evaluation environment

The direct prompt must not ask the agent to imitate Loom artifacts. The Loom prompt may require Loom's normal delivery path. Both prompts require a focused regression check and a completed implementation.

## Task Set Selection

Register the full task list before launching any agent run. The registration must include instance IDs, dataset revision, selection date, and the reason for any exclusion.

Use a mixed repository set. Do not choose or remove instances after seeing a paired outcome. An instance can be excluded before running only when its base checkout or pinned environment cannot be prepared on the benchmark host. Report every exclusion and its failure log.

For a published comparison, use at least 10 instances across multiple repositories. Run each paired instance three times with independent agent sessions. Report the median per instance and the aggregate result across all registered instances.

## Resolution Track

1. Prepare direct and Loom workspaces with `run.mjs prepare`.
2. Run each agent session independently. Do not transfer source patches, chat transcripts, or Loom state between variants.
3. Record the patch, available token count, turns, elapsed time, and terminal agent status with `run.mjs record`.
4. Run the official SWE-bench harness against the saved patch.
5. Record the official resolved or unresolved outcome for every attempt.

The primary metric is official resolved instances divided by evaluated instances. Tokens, turns, elapsed time, changed files, and repairs are secondary metrics. They are useful for explaining a result but never substitute for a resolved outcome.

## Recovery Track

The Recovery track begins only after an initial attempt has been evaluated.

1. For every unresolved initial attempt, provide both variants the same evaluator feedback: failing test identifiers and non-secret test output.
2. Start a new agent session for each variant in its own original workspace.
3. The direct variant may inspect the repository, original patch, and supplied feedback. It receives no hand-authored summary beyond that feedback.
4. The Loom variant resumes the existing project-local `.loom/` state and follows its normal repair route.
5. Run the official harness again after the repair attempt.

Report the recovery rate only over initially unresolved attempts. Also report the initial resolution rate alongside it. A Recovery result must never be combined with an initial Resolution result into a single score.

## Delivery Evidence

For every Loom attempt, preserve the project-local delivery artifacts needed to audit the outcome:

- task and architecture contracts
- task-result verification evidence
- repair request and repair result, when applicable
- review or handoff result

For each direct attempt, preserve the prompt, patch, agent terminal status, commands run, and evaluation output. This is not an attempt to make the workflows identical; it makes both outcomes inspectable.

Do not publish private repository contents, API credentials, or raw agent transcripts. Publish a redacted evidence index and hashes for retained local artifacts when needed.

## Reporting

Publish one result table per track. Each table includes every registered instance and attempt, including failures, timeouts, empty patches, setup failures, and evaluation failures.

For every aggregate metric, report:

- numerator and denominator
- median and range for numeric secondary metrics
- paired completion difference between variants
- excluded instances and reason

Use the phrase "official SWE-bench Verified result" only for results produced by the official harness. Do not call local evaluation output an official score.
